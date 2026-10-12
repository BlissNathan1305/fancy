//! rustwarden: an encrypted password vault in the terminal.
//!
//! Vault format v2: `RWV2 | salt (16) | nonce (12) | AES-256-GCM ciphertext`,
//! key = Argon2id(master password, salt). Legacy v1 files (`nonce | ciphertext`,
//! key = SHA-256(password)) are still readable and are upgraded on first save.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, bail, Context, Result};
use argon2::Argon2;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use tui_snippets::{copy_to_clipboard, data_dir, home_dir, term, write_atomic};

const MAGIC: &[u8; 4] = b"RWV2";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Parser)]
#[command(
    name = "rustwarden",
    version,
    about = "Encrypted password vault for the terminal"
)]
struct Cli {
    /// Vault file to use (default: ~/.local/share/rustwarden/vault.enc)
    #[arg(long)]
    vault: Option<PathBuf>,

    /// Merge the entries of another vault file into this one, then exit
    #[arg(long, value_name = "FILE")]
    import: Option<PathBuf>,
}

// --- Data ---

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Entry {
    service: String,
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct Vault {
    entries: Vec<Entry>,
}

impl Vault {
    /// Adds entries that are not already present; returns how many were added.
    fn merge(&mut self, other: Vault) -> usize {
        let mut added = 0;
        for entry in other.entries {
            if !self.entries.contains(&entry) {
                self.entries.push(entry);
                added += 1;
            }
        }
        added
    }
}

// --- Crypto ---

/// An unlocked vault key, remembered so every save reuses the same salt.
struct Key {
    salt: [u8; SALT_LEN],
    bytes: [u8; 32],
}

impl Key {
    fn derive(password: &str, salt: [u8; SALT_LEN]) -> Result<Self> {
        let mut bytes = [0u8; 32];
        Argon2::default()
            .hash_password_into(password.as_bytes(), &salt, &mut bytes)
            .map_err(|e| anyhow!("key derivation failed: {e}"))?;
        Ok(Self { salt, bytes })
    }

    fn new_random(password: &str) -> Result<Self> {
        let mut salt = [0u8; SALT_LEN];
        getrandom::fill(&mut salt).map_err(|e| anyhow!("no randomness available: {e}"))?;
        Self::derive(password, salt)
    }
}

fn encrypt(vault: &Vault, key: &Key) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(&key.bytes).context("invalid key length")?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce_bytes).map_err(|e| anyhow!("no randomness available: {e}"))?;
    let nonce = Nonce::try_from(&nonce_bytes[..]).context("invalid nonce length")?;
    let ciphertext = cipher
        .encrypt(&nonce, serde_json::to_vec(vault)?.as_ref())
        .map_err(|_| anyhow!("encryption failed"))?;

    let mut out = Vec::with_capacity(4 + SALT_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&key.salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend(ciphertext);
    Ok(out)
}

fn open_aead(key: &[u8; 32], nonce: &[u8], ciphertext: &[u8]) -> Result<Vault> {
    let cipher = Aes256Gcm::new_from_slice(key).context("invalid key length")?;
    let nonce = Nonce::try_from(nonce).context("invalid nonce length")?;
    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| anyhow!("wrong master password (or the vault file is damaged)"))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

/// Returns the vault, its key, and whether the file was in the legacy format.
fn decrypt(data: &[u8], password: &str) -> Result<(Vault, Key, bool)> {
    if data.starts_with(MAGIC) {
        let rest = &data[MAGIC.len()..];
        if rest.len() < SALT_LEN + NONCE_LEN {
            bail!("vault file is truncated");
        }
        let (salt, rest) = rest.split_at(SALT_LEN);
        let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
        let key = Key::derive(password, salt.try_into()?)?;
        let vault = open_aead(&key.bytes, nonce, ciphertext)?;
        Ok((vault, key, false))
    } else {
        if data.len() < NONCE_LEN {
            bail!("vault file is truncated");
        }
        let legacy_key: [u8; 32] = Sha256::digest(password.as_bytes()).into();
        let (nonce, ciphertext) = data.split_at(NONCE_LEN);
        let vault = open_aead(&legacy_key, nonce, ciphertext)?;
        Ok((vault, Key::new_random(password)?, true))
    }
}

fn save_vault(path: &Path, vault: &Vault, key: &Key) -> Result<()> {
    write_atomic(path, &encrypt(vault, key)?)
        .with_context(|| format!("cannot write {}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn prompt(msg: &str) -> Result<String> {
    rpassword::prompt_password(msg).context("failed to read password")
}

// --- Unlocking ---

/// Vault files written by older versions, which saved into the current directory.
fn legacy_candidates() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for path in [PathBuf::from("vault.enc"), home_dir().join("vault.enc")] {
        if let Ok(canonical) = path.canonicalize() {
            if !found.contains(&canonical) {
                found.push(canonical);
            }
        }
    }
    found
}

fn unlock(path: &Path) -> Result<(Vault, Key)> {
    if path.exists() {
        let data = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
        let password = prompt("🔐 Master password: ")?;
        let (vault, key, legacy) = decrypt(&data, &password)?;
        if legacy {
            save_vault(path, &vault, &key)?;
            eprintln!(
                "Upgraded {} to the stronger Argon2id format.",
                path.display()
            );
        }
        return Ok((vault, key));
    }

    let legacy = legacy_candidates();
    if !legacy.is_empty() {
        eprintln!("Found vault(s) from an older version:");
        for p in &legacy {
            eprintln!("  {}", p.display());
        }
        let password = prompt("🔐 Master password to import them: ")?;
        let mut vault = Vault::default();
        let mut key = None;
        for p in &legacy {
            match fs::read(p)
                .map_err(anyhow::Error::from)
                .and_then(|d| decrypt(&d, &password))
            {
                Ok((v, k, _)) => {
                    let n = vault.merge(v);
                    eprintln!(
                        "  imported {n} entr{} from {}",
                        if n == 1 { "y" } else { "ies" },
                        p.display()
                    );
                    key.get_or_insert(k);
                }
                Err(e) => eprintln!(
                    "  skipped {}: {e} (import it later with --import)",
                    p.display()
                ),
            }
        }
        let key = key.ok_or_else(|| anyhow!("none of the old vaults opened with that password"))?;
        save_vault(path, &vault, &key)?;
        eprintln!(
            "New vault saved to {} (the old files were left untouched).",
            path.display()
        );
        return Ok((vault, key));
    }

    eprintln!("Creating a new vault at {}", path.display());
    let password = prompt("🔐 Choose a master password: ")?;
    if password.chars().count() < 8 {
        bail!("master password must be at least 8 characters");
    }
    if prompt("🔐 Repeat it: ")? != password {
        bail!("passwords did not match; nothing was created");
    }
    let key = Key::new_random(&password)?;
    let vault = Vault::default();
    save_vault(path, &vault, &key)?;
    Ok((vault, key))
}

fn import(path: &Path, from: &Path) -> Result<()> {
    let (mut vault, key) = unlock(path)?;
    let data = fs::read(from).with_context(|| format!("cannot read {}", from.display()))?;
    let password = prompt(&format!("🔐 Password for {}: ", from.display()))?;
    let (other, _, _) = decrypt(&data, &password)?;
    let added = vault.merge(other);
    save_vault(path, &vault, &key)?;
    println!(
        "Imported {added} new entr{} into {}",
        if added == 1 { "y" } else { "ies" },
        path.display()
    );
    Ok(())
}

// --- Password generator ---

fn generate_password(len: usize) -> Result<String> {
    const CHARSET: &[u8] =
        b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*-_=+?";
    let mut out = String::with_capacity(len);
    let mut buf = [0u8; 64];
    // Rejection sampling keeps every character equally likely.
    let limit = 256 - (256 % CHARSET.len());
    while out.len() < len {
        getrandom::fill(&mut buf).map_err(|e| anyhow!("no randomness available: {e}"))?;
        for &b in &buf {
            if (b as usize) < limit && out.len() < len {
                out.push(CHARSET[b as usize % CHARSET.len()] as char);
            }
        }
    }
    Ok(out)
}

// --- App state ---

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Service,
    User,
    Pass,
}

enum Mode {
    Normal,
    Form {
        field: Field,
        editing: Option<usize>,
    },
    ConfirmDelete,
}

struct App {
    path: PathBuf,
    key: Key,
    vault: Vault,
    state: ListState,
    mode: Mode,
    input: String,
    draft: Entry,
    reveal: bool,
    status: String,
    status_color: Color,
}

impl App {
    fn new(path: PathBuf, vault: Vault, key: Key) -> Self {
        let mut state = ListState::default();
        state.select(if vault.entries.is_empty() {
            None
        } else {
            Some(0)
        });
        Self {
            path,
            key,
            vault,
            state,
            mode: Mode::Normal,
            input: String::new(),
            draft: Entry {
                service: String::new(),
                username: String::new(),
                password: String::new(),
            },
            reveal: false,
            status: "Vault unlocked".to_string(),
            status_color: Color::Green,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>, color: Color) {
        self.status = msg.into();
        self.status_color = color;
    }

    fn save(&mut self) {
        match save_vault(&self.path, &self.vault, &self.key) {
            Ok(()) => {}
            Err(e) => self.set_status(format!("SAVE FAILED: {e}"), Color::Red),
        }
    }

    fn selected(&self) -> Option<&Entry> {
        self.state
            .selected()
            .and_then(|i| self.vault.entries.get(i))
    }

    fn start_form(&mut self, editing: Option<usize>) {
        self.draft = match editing.and_then(|i| self.vault.entries.get(i)) {
            Some(e) => e.clone(),
            None => Entry {
                service: String::new(),
                username: String::new(),
                password: String::new(),
            },
        };
        self.input = self.draft.service.clone();
        self.mode = Mode::Form {
            field: Field::Service,
            editing,
        };
    }

    fn form_next(&mut self) {
        let Mode::Form { field, editing } = self.mode else {
            return;
        };
        let value = std::mem::take(&mut self.input);
        match field {
            Field::Service => {
                if value.trim().is_empty() {
                    self.set_status("Service name cannot be empty", Color::Yellow);
                    return;
                }
                self.draft.service = value.trim().to_string();
                self.input = self.draft.username.clone();
                self.mode = Mode::Form {
                    field: Field::User,
                    editing,
                };
            }
            Field::User => {
                self.draft.username = value;
                self.input = self.draft.password.clone();
                self.mode = Mode::Form {
                    field: Field::Pass,
                    editing,
                };
            }
            Field::Pass => {
                if value.is_empty() {
                    self.input = value;
                    self.set_status(
                        "Password cannot be empty (Tab generates one)",
                        Color::Yellow,
                    );
                    return;
                }
                self.draft.password = value;
                let entry = self.draft.clone();
                let index = match editing {
                    Some(i) if i < self.vault.entries.len() => {
                        self.vault.entries[i] = entry;
                        i
                    }
                    _ => {
                        self.vault.entries.push(entry);
                        self.vault.entries.len() - 1
                    }
                };
                self.state.select(Some(index));
                self.mode = Mode::Normal;
                self.save();
                self.set_status(
                    if editing.is_some() {
                        "Entry updated"
                    } else {
                        "Entry added"
                    },
                    Color::Green,
                );
            }
        }
    }

    fn delete_selected(&mut self) {
        let Some(i) = self.state.selected() else {
            return;
        };
        if i >= self.vault.entries.len() {
            return;
        }
        let removed = self.vault.entries.remove(i);
        if self.vault.entries.is_empty() {
            self.state.select(None);
        } else if i >= self.vault.entries.len() {
            self.state.select(Some(self.vault.entries.len() - 1));
        }
        self.save();
        self.set_status(format!("Deleted {}", removed.service), Color::Red);
    }

    fn copy(&mut self, what: &str, text: String) {
        match copy_to_clipboard(&text) {
            Ok(()) => self.set_status(format!("{what} copied to clipboard"), Color::Green),
            Err(e) => self.set_status(e, Color::Red),
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.vault.entries.len() as isize;
        if len == 0 {
            return;
        }
        let current = self.state.selected().unwrap_or(0) as isize;
        self.state
            .select(Some((current + delta).clamp(0, len - 1) as usize));
    }
}

// --- UI ---

fn mask(s: &str) -> String {
    "•".repeat(s.chars().count().clamp(8, 24))
}

fn ui(f: &mut Frame, app: &App) {
    let surface = Color::Rgb(49, 50, 68);
    let text = Color::Rgb(205, 214, 244);
    let accent = Color::Rgb(137, 180, 250);

    f.render_widget(ratatui::widgets::Clear, f.size());
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(6),
        Constraint::Length(1),
    ])
    .split(f.size());

    let header = Paragraph::new(Line::from(vec![
        Span::styled("🔐 ", Style::default().fg(text)),
        Span::styled(
            "RUSTWARDEN",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {} entries", app.vault.entries.len()),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .style(Style::default().bg(surface).fg(text))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(surface)),
    );
    f.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = app
        .vault
        .entries
        .iter()
        .map(|e| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", e.service),
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({})", e.username),
                    Style::default().fg(Color::Gray),
                ),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Vault Entries ")
                .border_style(Style::default().fg(surface))
                .title_style(Style::default().fg(text)),
        )
        .bg(surface)
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(88, 91, 112))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, chunks[1], &mut app.state.clone());

    let bottom = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(surface));
    let details = match &app.mode {
        Mode::Form { field, editing } => {
            let label = match field {
                Field::Service => "Service",
                Field::User => "Username",
                Field::Pass => "Password (Tab: generate)",
            };
            let shown = if *field == Field::Pass && !app.reveal {
                mask(&app.input)
            } else {
                app.input.clone()
            };
            let shown = if *field == Field::Pass && app.input.is_empty() {
                String::new()
            } else {
                shown
            };
            let title = if editing.is_some() {
                " Edit Entry "
            } else {
                " New Entry "
            };
            Paragraph::new(format!("{label}: {shown}▏"))
                .style(Style::default().fg(Color::Yellow))
                .block(
                    bottom
                        .title(title)
                        .title_style(Style::default().fg(Color::Yellow)),
                )
                .wrap(Wrap { trim: false })
        }
        _ => match app.selected() {
            Some(entry) => {
                let password = if app.reveal {
                    entry.password.clone()
                } else {
                    mask(&entry.password)
                };
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        format!("Service:  {}", entry.service),
                        Style::default().fg(text),
                    )),
                    Line::from(Span::styled(
                        format!("Username: {}", entry.username),
                        Style::default().fg(text),
                    )),
                    Line::from(Span::styled(
                        format!("Password: {}", password),
                        Style::default().fg(Color::Magenta),
                    )),
                ])
                .wrap(Wrap { trim: false })
                .block(
                    bottom
                        .title(" Details (v: show/hide) ")
                        .title_style(Style::default().fg(text)),
                )
            }
            None => Paragraph::new("No entries yet. Press 'a' to add one.").block(bottom),
        },
    };
    f.render_widget(details, chunks[2]);

    let controls = match app.mode {
        Mode::Normal => "a add | e edit | d delete | c copy pass | u copy user | v reveal | q quit",
        Mode::Form { .. } => "Enter next | Tab generate password | Ctrl+V reveal | Esc cancel",
        Mode::ConfirmDelete => "Delete this entry? y: yes | any other key: no",
    };
    let status_bar = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", controls),
            Style::default().fg(Color::Rgb(30, 30, 46)).bg(accent),
        ),
        Span::styled(
            format!(" {}", app.status),
            Style::default().fg(app.status_color),
        ),
    ]))
    .style(Style::default().bg(surface));
    f.render_widget(status_bar, chunks[3]);
}

fn run_app(terminal: &mut term::Tui, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match app.mode {
            Mode::Normal => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('a') => app.start_form(None),
                KeyCode::Char('e') if app.selected().is_some() => {
                    app.start_form(app.state.selected())
                }
                KeyCode::Char('d') if app.selected().is_some() => app.mode = Mode::ConfirmDelete,
                KeyCode::Char('c') => {
                    if let Some(p) = app.selected().map(|e| e.password.clone()) {
                        app.copy("Password", p);
                    }
                }
                KeyCode::Char('u') => {
                    if let Some(u) = app.selected().map(|e| e.username.clone()) {
                        app.copy("Username", u);
                    }
                }
                KeyCode::Char('v') => app.reveal = !app.reveal,
                KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                _ => {}
            },
            Mode::Form { field, .. } => match key.code {
                KeyCode::Esc => {
                    app.mode = Mode::Normal;
                    app.input.clear();
                    app.set_status("Cancelled", Color::Yellow);
                }
                KeyCode::Enter => app.form_next(),
                KeyCode::Tab if field == Field::Pass => match generate_password(20) {
                    Ok(p) => {
                        app.input = p;
                        app.set_status("Generated a 20-character password", Color::Green);
                    }
                    Err(e) => app.set_status(e.to_string(), Color::Red),
                },
                KeyCode::Char('v')
                    if key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
                {
                    app.reveal = !app.reveal
                }
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.input.clear()
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.input.push(c)
                }
                _ => {}
            },
            Mode::ConfirmDelete => {
                app.mode = Mode::Normal;
                if key.code == KeyCode::Char('y') {
                    app.delete_selected();
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = match cli.vault {
        Some(p) => p,
        None => data_dir("rustwarden")?.join("vault.enc"),
    };

    if let Some(from) = cli.import {
        return import(&path, &from);
    }

    let (vault, key) = unlock(&path)?;
    let mut app = App::new(path, vault, key);
    let mut terminal = term::init()?;
    let result = run_app(&mut terminal, &mut app);
    term::restore()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vault {
        Vault {
            entries: vec![Entry {
                service: "mail".into(),
                username: "me".into(),
                password: "s3cret".into(),
            }],
        }
    }

    #[test]
    fn v2_round_trip() {
        let key = Key::new_random("correct horse").unwrap();
        let data = encrypt(&sample(), &key).unwrap();
        assert!(data.starts_with(MAGIC));
        let (vault, _, legacy) = decrypt(&data, "correct horse").unwrap();
        assert!(!legacy);
        assert_eq!(vault.entries, sample().entries);
        assert!(decrypt(&data, "wrong").is_err());
    }

    #[test]
    fn reads_legacy_v1_files() {
        // Build a v1 file exactly as the old version did.
        let legacy_key: [u8; 32] = Sha256::digest(b"old-pass").into();
        let cipher = Aes256Gcm::new_from_slice(&legacy_key).unwrap();
        let nonce_bytes = [7u8; NONCE_LEN];
        let nonce = Nonce::try_from(&nonce_bytes[..]).unwrap();
        let ct = cipher
            .encrypt(&nonce, serde_json::to_vec(&sample()).unwrap().as_ref())
            .unwrap();
        let mut data = nonce_bytes.to_vec();
        data.extend(ct);

        let (vault, _, legacy) = decrypt(&data, "old-pass").unwrap();
        assert!(legacy);
        assert_eq!(vault.entries.len(), 1);
    }

    #[test]
    fn truncated_files_error_instead_of_panicking() {
        assert!(decrypt(b"short", "x").is_err());
        assert!(decrypt(b"RWV2short", "x").is_err());
    }

    #[test]
    fn merge_skips_duplicates() {
        let mut a = sample();
        assert_eq!(a.merge(sample()), 0);
        let other = Vault {
            entries: vec![Entry {
                service: "bank".into(),
                username: "u".into(),
                password: "p".into(),
            }],
        };
        assert_eq!(a.merge(other), 1);
        assert_eq!(a.entries.len(), 2);
    }

    #[test]
    fn generated_passwords_have_requested_length() {
        let p = generate_password(20).unwrap();
        assert_eq!(p.chars().count(), 20);
        assert_ne!(p, generate_password(20).unwrap());
    }
}
