use anyhow::{Context, Result};
use arboard::Clipboard;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use getrandom::fill;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{convert::TryFrom, fs, io};

const VAULT_FILE: &str = "vault.enc";

// --- Data Structures ---

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    service: String,
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct Vault {
    entries: Vec<Entry>,
}

// --- Cryptography ---

fn derive_key(master_pass: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(master_pass.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

fn encrypt_vault(vault: &Vault, key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).context("Invalid key length")?;
    let plaintext = serde_json::to_vec(vault)?;
    
    let mut nonce_bytes = [0u8; 12];
        fill(&mut nonce_bytes).context("Failed to generate random nonce")?;
    let nonce = Nonce::try_from(&nonce_bytes[..]).context("Invalid nonce length")?;

    let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref())?;
    
    let mut final_data = nonce_bytes.to_vec();
    final_data.extend(ciphertext);
    Ok(final_data)
}

fn decrypt_vault(data: &[u8], key: &[u8; 32]) -> Result<Vault> {
    let cipher = Aes256Gcm::new_from_slice(key).context("Invalid key length")?;
    
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::try_from(nonce_bytes).context("Invalid nonce length")?;

    let plaintext = cipher.decrypt(&nonce, ciphertext)?;
    let vault = serde_json::from_slice(&plaintext)?;
    Ok(vault)
}

// --- App State ---

enum AppMode {
    Normal,
    AddService,
    AddUser,
    AddPass,
}

struct App {
    vault: Vault,
    state: ListState,
    mode: AppMode,
    input_buffer: String,
    temp_entry: Entry,
    status_message: String,
    status_color: Color,
}

impl App {
    fn new(vault: Vault) -> Self {
        let mut state = ListState::default();
        if !vault.entries.is_empty() {
            state.select(Some(0));
        }
        Self {
            vault,
            state,
            mode: AppMode::Normal,
            input_buffer: String::new(),
            temp_entry: Entry { service: String::new(), username: String::new(), password: String::new() },
            status_message: "Ready".to_string(),
            status_color: Color::Green,
        }
    }

    fn save(&mut self, key: &[u8; 32]) -> Result<()> {
        let data = encrypt_vault(&self.vault, key)?;
        fs::write(VAULT_FILE, data)?;
        self.status_message = "Vault saved and locked!".to_string();
        self.status_color = Color::Green;
        Ok(())
    }
}

// --- UI Rendering ---

fn ui(f: &mut Frame, app: &App) {
    let bg_color = Color::Rgb(30, 30, 46); 
    let surface_color = Color::Rgb(49, 50, 68);
    let text_color = Color::Rgb(205, 214, 244);
    let accent_color = Color::Rgb(137, 180, 250); 
    let highlight_bg = Color::Rgb(88, 91, 112);

    f.render_widget(ratatui::widgets::Clear, f.size()); 

    let chunks = Layout::vertical([
        Constraint::Length(3), 
        Constraint::Min(10),  
        Constraint::Length(6), 
        Constraint::Length(3), 
    ]).split(f.size());

    // 1. Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("🔐 ", Style::default().fg(text_color)),
        Span::styled("RUSTWARDEN", Style::default().fg(accent_color).add_modifier(Modifier::BOLD)),
    ]))
    .style(Style::default().bg(surface_color).fg(text_color))
    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(surface_color)));
    f.render_widget(header, chunks[0]);

    // 2. List
    let items: Vec<ListItem> = app.vault.entries.iter().map(|e| {
        ListItem::new(Line::from(vec![
            Span::styled(format!(" {} ", e.service), Style::default().fg(accent_color).add_modifier(Modifier::BOLD)),
            Span::styled(format!("({})", e.username), Style::default().fg(Color::DarkGray)),
        ]))
    }).collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Vault Entries ").border_style(Style::default().fg(surface_color)).title_style(Style::default().fg(text_color)))
        .bg(surface_color)
        .highlight_style(Style::default().bg(highlight_bg).fg(Color::Black))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, chunks[1], &mut app.state.clone());

    // 3. Bottom Area
    let bottom_block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(surface_color));
    
    let bottom_content = match app.mode {
        AppMode::Normal => {
            if let Some(i) = app.state.selected() {
                if let Some(entry) = app.vault.entries.get(i) {
                    Paragraph::new(vec![
                        Line::from(Span::styled(format!("Service:  {}", entry.service), Style::default().fg(text_color))),
                        Line::from(Span::styled(format!("Username: {}", entry.username), Style::default().fg(text_color))),
                        Line::from(Span::styled(format!("Password: {}", entry.password), Style::default().fg(Color::Magenta))),
                    ]).wrap(Wrap { trim: false }).block(bottom_block.title(" Details ").title_style(Style::default().fg(text_color)))
                } else { Paragraph::new("").block(bottom_block) }
            } else { Paragraph::new("No entries").block(bottom_block) }
        },
        _ => { 
            let prompt = match app.mode {
                AppMode::AddService => "Enter Service Name",
                AppMode::AddUser => "Enter Username",
                AppMode::AddPass => "Enter Password",
                _ => ""
            };
            Paragraph::new(format!("{}: {}", prompt, app.input_buffer))
                .style(Style::default().fg(Color::Yellow))
                .block(bottom_block.title(" New Entry ").title_style(Style::default().fg(Color::Yellow)))
                .wrap(Wrap { trim: false })
        }
    };
    f.render_widget(bottom_content, chunks[2]);

    // 4. Status Bar
    let controls = match app.mode {
        AppMode::Normal => "↑/↓: Navigate | a: Add | d: Delete | c: Copy Pass | q: Quit",
        _ => "Enter: Next Step | Esc: Cancel"
    };
    let status_bar = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {} ", controls), Style::default().fg(bg_color).bg(accent_color)),
        Span::styled(format!(" {}", app.status_message), Style::default().fg(app.status_color)),
    ]))
    .style(Style::default().bg(surface_color));
    f.render_widget(status_bar, chunks[3]);
}

// --- Main Logic ---

fn main() -> Result<()> {
    // 1. Get Master Password
    println!("🔐 Enter Master Password to unlock vault:");
    let master_pass = rpassword::prompt_password("> ").context("Failed to read password")?;
    let key = derive_key(&master_pass);

    // 2. Load or Create Vault
    let vault = if fs::metadata(VAULT_FILE).is_ok() {
        let data = fs::read(VAULT_FILE).context("Failed to read vault file")?;
        decrypt_vault(&data, &key).context("Decryption failed. Wrong password?")
    } else {
        Ok(Vault { entries: Vec::new() })
    }?;

    // 3. Setup TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(vault);
    let mut clipboard = Clipboard::new()?;

    let result = run_app(&mut terminal, &mut app, &mut clipboard, &key);

    // 4. Teardown TUI
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    app: &mut App,
    clipboard: &mut Clipboard,
    key: &[u8; 32],
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key_event) = event::read()? {
            if key_event.kind != KeyEventKind::Press { continue; }

            match app.mode {
                AppMode::Normal => match key_event.code {
                    KeyCode::Char('q') => {
                        app.save(key)?;
                        return Ok(());
                    }
                    KeyCode::Char('a') => {
                        app.mode = AppMode::AddService;
                        app.input_buffer.clear();
                        app.temp_entry = Entry { service: String::new(), username: String::new(), password: String::new() };
                    }
                    KeyCode::Char('d') => {
                        if let Some(i) = app.state.selected() {
                            app.vault.entries.remove(i);
                            app.status_message = "Entry deleted.".to_string();
                            app.status_color = Color::Red;
                            if app.vault.entries.is_empty() {
                                app.state.select(None);
                            } else if i >= app.vault.entries.len() {
                                app.state.select(Some(app.vault.entries.len() - 1));
                            }
                        }
                    }
                    KeyCode::Char('c') => {
                        if let Some(i) = app.state.selected() {
                            if let Some(entry) = app.vault.entries.get(i) {
                                clipboard.set_text(&entry.password)?;
                                app.status_message = "Password copied to clipboard!".to_string();
                                app.status_color = Color::Green;
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !app.vault.entries.is_empty() {
                            let current = app.state.selected().unwrap_or(0);
                            let next = current.saturating_add(1).min(app.vault.entries.len() - 1);
                            app.state.select(Some(next));
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let Some(current) = app.state.selected() {
                            let next = current.saturating_sub(1);
                            app.state.select(Some(next));
                        }
                    }
                    _ => {}
                },
                _ => { 
                    match key_event.code {
                        KeyCode::Esc => {
                            app.mode = AppMode::Normal;
                        }
                        KeyCode::Enter => {
                            match app.mode {
                                AppMode::AddService => {
                                    app.temp_entry.service = app.input_buffer.clone();
                                    app.input_buffer.clear();
                                    app.mode = AppMode::AddUser;
                                }
                                AppMode::AddUser => {
                                    app.temp_entry.username = app.input_buffer.clone();
                                    app.input_buffer.clear();
                                    app.mode = AppMode::AddPass;
                                }
                                AppMode::AddPass => {
                                    app.temp_entry.password = app.input_buffer.clone();
                                    app.vault.entries.push(app.temp_entry.clone());
                                    app.mode = AppMode::Normal;
                                    app.save(key)?;
                                    app.state.select(Some(app.vault.entries.len() - 1));
                                }
                                _ => {}
                            }
                        }
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                        }
                        KeyCode::Char(c) => {
                            app.input_buffer.push(c);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
