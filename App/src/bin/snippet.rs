use anyhow::{bail, Context, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tui_snippets::{copy_to_clipboard, data_dir, home_dir, term, write_atomic};

#[derive(Parser)]
#[command(
    name = "snip",
    version,
    about = "Code snippet manager for the terminal"
)]
struct Cli {
    /// Merge snippets from another snippets.json into your collection, then exit
    #[arg(long, value_name = "FILE")]
    import: Option<PathBuf>,
}

// --- Data ---

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Snippet {
    title: String,
    language: String,
    code: String,
}

fn load(path: &Path) -> Result<Vec<Snippet>> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Vec::new()),
        Ok(text) => serde_json::from_str(&text).with_context(|| {
            format!(
                "{} is not valid snippet JSON; fix or move it and try again",
                path.display()
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

fn save(path: &Path, snippets: &[Snippet]) -> Result<()> {
    write_atomic(path, &serde_json::to_vec_pretty(snippets)?)?;
    Ok(())
}

/// Adds snippets that are not already present; returns how many were added.
fn merge(into: &mut Vec<Snippet>, from: Vec<Snippet>) -> usize {
    let before = into.len();
    for s in from {
        if !into.contains(&s) {
            into.push(s);
        }
    }
    into.len() - before
}

/// Older versions kept `snippets.json` in whatever directory snip was started from.
fn migrate_legacy(path: &Path) -> Result<Option<String>> {
    if path.exists() {
        return Ok(None);
    }
    let mut snippets = Vec::new();
    let mut sources = Vec::new();
    for candidate in [
        PathBuf::from("snippets.json"),
        home_dir().join("snippets.json"),
    ] {
        let Ok(canonical) = candidate.canonicalize() else {
            continue;
        };
        if sources.contains(&canonical) {
            continue;
        }
        if let Ok(found) = load(&canonical) {
            merge(&mut snippets, found);
            sources.push(canonical);
        }
    }
    if sources.is_empty() {
        return Ok(None);
    }
    save(path, &snippets)?;
    let names: Vec<String> = sources.iter().map(|p| p.display().to_string()).collect();
    Ok(Some(format!(
        "Imported {} snippets from {}",
        snippets.len(),
        names.join(", ")
    )))
}

// --- Editor integration ---

fn editor_command() -> String {
    for var in ["VISUAL", "EDITOR"] {
        if let Ok(value) = std::env::var(var) {
            if !value.trim().is_empty() {
                return value;
            }
        }
    }
    for candidate in ["nano", "nvim", "vim", "vi"] {
        if Command::new("sh")
            .args(["-c", &format!("command -v {candidate}")])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return candidate.to_string();
        }
    }
    "vi".to_string()
}

fn extension_for(language: &str) -> &'static str {
    match language.to_lowercase().as_str() {
        "rust" | "rs" => "rs",
        "python" | "py" => "py",
        "javascript" | "js" => "js",
        "typescript" | "ts" => "ts",
        "bash" | "shell" | "sh" => "sh",
        "go" => "go",
        "c" => "c",
        "c++" | "cpp" => "cpp",
        "java" => "java",
        "sql" => "sql",
        "html" => "html",
        "css" => "css",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "markdown" | "md" => "md",
        _ => "txt",
    }
}

/// Opens `code` in the user's editor and returns the edited text.
fn edit_in_editor(code: &str, language: &str) -> Result<String> {
    let path = std::env::temp_dir().join(format!(
        "snip-{}.{}",
        std::process::id(),
        extension_for(language)
    ));
    fs::write(&path, code)?;
    // Run through the shell so values like `code --wait` work.
    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{} \"$1\"", editor_command()))
        .arg("snip")
        .arg(&path)
        .status()
        .context("could not start the editor")?;
    let edited = fs::read_to_string(&path);
    let _ = fs::remove_file(&path);
    if !status.success() {
        bail!("editor exited with {status}");
    }
    Ok(edited?.trim_end_matches('\n').to_string())
}

// --- App state ---

enum Mode {
    Normal,
    Search,
    NewTitle,
    NewLanguage,
    ConfirmDelete,
}

struct App {
    path: PathBuf,
    snippets: Vec<Snippet>,
    filtered: Vec<usize>,
    state: ListState,
    mode: Mode,
    search: String,
    input: String,
    draft_title: String,
    status: String,
}

impl App {
    fn new(path: PathBuf, status: Option<String>) -> Result<Self> {
        let snippets = load(&path)?;
        let mut app = Self {
            path,
            snippets,
            filtered: Vec::new(),
            state: ListState::default(),
            mode: Mode::Normal,
            search: String::new(),
            input: String::new(),
            draft_title: String::new(),
            status: status.unwrap_or_default(),
        };
        if app.status.is_empty() {
            app.status = if app.snippets.is_empty() {
                "No snippets yet. Press 'a' to add one.".to_string()
            } else {
                format!("Loaded {} snippets", app.snippets.len())
            };
        }
        app.update_filter();
        Ok(app)
    }

    fn persist(&mut self) {
        if let Err(e) = save(&self.path, &self.snippets) {
            self.status = format!("SAVE FAILED: {e}");
        }
    }

    fn update_filter(&mut self) {
        let query = self.search.to_lowercase();
        self.filtered = self
            .snippets
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                query.is_empty()
                    || s.title.to_lowercase().contains(&query)
                    || s.language.to_lowercase().contains(&query)
                    || s.code.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        let keep = self.state.selected().filter(|&i| i < self.filtered.len());
        self.state.select(keep.or(if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        }));
    }

    fn selected_index(&self) -> Option<usize> {
        self.state
            .selected()
            .and_then(|i| self.filtered.get(i).copied())
    }

    fn selected(&self) -> Option<&Snippet> {
        self.selected_index().and_then(|i| self.snippets.get(i))
    }

    fn select_snippet(&mut self, real_index: usize) {
        self.search.clear();
        self.update_filter();
        self.state
            .select(self.filtered.iter().position(|&i| i == real_index));
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.filtered.len() as isize;
        if len == 0 {
            return;
        }
        let current = self.state.selected().unwrap_or(0) as isize;
        self.state
            .select(Some((current + delta).clamp(0, len - 1) as usize));
    }
}

// --- UI ---

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Percentage(45),
        Constraint::Length(1),
    ])
    .split(f.size());

    let (top_title, top_text, top_color) = match app.mode {
        Mode::Search => (
            " Search title, language or code (Enter: keep | Esc: clear) ",
            format!("{}▏", app.search),
            Color::Yellow,
        ),
        Mode::NewTitle => (
            " New snippet: title ",
            format!("{}▏", app.input),
            Color::Yellow,
        ),
        Mode::NewLanguage => (
            " New snippet: language (e.g. Rust, Python) ",
            format!("{}▏", app.input),
            Color::Yellow,
        ),
        Mode::ConfirmDelete => (
            " Confirm ",
            "Delete this snippet? y: yes | any other key: no".to_string(),
            Color::Red,
        ),
        Mode::Normal => (
            " Snippet Manager ",
            " j/k: Move | Enter: Copy | /: Search | a: Add | e: Edit code | d: Delete | q: Quit"
                .to_string(),
            Color::White,
        ),
    };
    let top = Paragraph::new(top_text)
        .style(Style::default().fg(top_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(top_title)
                .border_style(Style::default().fg(top_color)),
        );
    f.render_widget(top, chunks[0]);

    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| {
            let snip = &app.snippets[i];
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", snip.title),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({})", snip.language),
                    Style::default().fg(Color::Gray),
                ),
            ]))
        })
        .collect();
    let title = if app.search.is_empty() {
        format!(" Snippets ({}) ", app.snippets.len())
    } else {
        format!(
            " Snippets ({} of {} match \"{}\") ",
            app.filtered.len(),
            app.snippets.len(),
            app.search
        )
    };
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, chunks[1], &mut app.state.clone());

    let preview = app
        .selected()
        .map(|s| s.code.clone())
        .unwrap_or_else(|| "No snippet selected.".to_string());
    let preview = Paragraph::new(preview)
        .block(Block::default().borders(Borders::ALL).title(" Preview "))
        .style(Style::default().fg(Color::Cyan))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, chunks[2]);

    f.render_widget(
        Paragraph::new(format!(" {}", app.status)).style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        chunks[3],
    );
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
                KeyCode::Char('r') => {
                    app.snippets = load(&app.path)?;
                    app.update_filter();
                    app.status = format!("Reloaded {} snippets", app.snippets.len());
                }
                KeyCode::Char('/') => app.mode = Mode::Search,
                KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                KeyCode::Enter | KeyCode::Char('y') => {
                    if let Some(snippet) = app.selected().cloned() {
                        app.status = match copy_to_clipboard(&snippet.code) {
                            Ok(()) => format!("Copied '{}' to clipboard", snippet.title),
                            Err(e) => e,
                        };
                    }
                }
                KeyCode::Char('a') => {
                    app.input.clear();
                    app.mode = Mode::NewTitle;
                }
                KeyCode::Char('e') => {
                    if let (Some(index), Some(snippet)) =
                        (app.selected_index(), app.selected().cloned())
                    {
                        let edited = term::suspend(terminal, || {
                            edit_in_editor(&snippet.code, &snippet.language)
                        })?;
                        match edited {
                            Ok(code) if code != snippet.code => {
                                app.snippets[index].code = code;
                                app.persist();
                                app.status = format!("Updated '{}'", snippet.title);
                            }
                            Ok(_) => app.status = "No changes".to_string(),
                            Err(e) => app.status = format!("Edit failed: {e}"),
                        }
                    }
                }
                KeyCode::Char('d') if app.selected().is_some() => app.mode = Mode::ConfirmDelete,
                _ => {}
            },
            Mode::Search => match key.code {
                KeyCode::Esc => {
                    app.search.clear();
                    app.update_filter();
                    app.mode = Mode::Normal;
                }
                KeyCode::Enter => app.mode = Mode::Normal,
                KeyCode::Down => app.move_selection(1),
                KeyCode::Up => app.move_selection(-1),
                KeyCode::Backspace => {
                    app.search.pop();
                    app.update_filter();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.search.push(c);
                    app.update_filter();
                }
                _ => {}
            },
            Mode::NewTitle | Mode::NewLanguage => match key.code {
                KeyCode::Esc => {
                    app.mode = Mode::Normal;
                    app.status = "Cancelled".to_string();
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
                KeyCode::Enter if !app.input.trim().is_empty() => {
                    let value = std::mem::take(&mut app.input).trim().to_string();
                    if matches!(app.mode, Mode::NewTitle) {
                        app.draft_title = value;
                        app.mode = Mode::NewLanguage;
                    } else {
                        app.mode = Mode::Normal;
                        let title = std::mem::take(&mut app.draft_title);
                        let edited = term::suspend(terminal, || edit_in_editor("", &value))?;
                        match edited {
                            Ok(code) if !code.trim().is_empty() => {
                                app.snippets.push(Snippet {
                                    title: title.clone(),
                                    language: value,
                                    code,
                                });
                                app.persist();
                                app.select_snippet(app.snippets.len() - 1);
                                app.status = format!("Added '{title}'");
                            }
                            Ok(_) => app.status = "Empty snippet discarded".to_string(),
                            Err(e) => app.status = format!("Edit failed: {e}"),
                        }
                    }
                }
                _ => {}
            },
            Mode::ConfirmDelete => {
                app.mode = Mode::Normal;
                if key.code == KeyCode::Char('y') {
                    if let Some(index) = app.selected_index() {
                        let removed = app.snippets.remove(index);
                        app.persist();
                        app.update_filter();
                        app.status = format!("Deleted '{}'", removed.title);
                    }
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = data_dir("snip")?.join("snippets.json");
    let migrated = migrate_legacy(&path)?;

    if let Some(from) = cli.import {
        let mut snippets = load(&path)?;
        let added = merge(&mut snippets, load(&from)?);
        save(&path, &snippets)?;
        println!("Imported {added} new snippet(s) into {}", path.display());
        return Ok(());
    }

    let mut app = App::new(path, migrated)?;
    let mut terminal = term::init()?;
    let result = run_app(&mut terminal, &mut app);
    term::restore()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snip(title: &str, code: &str) -> Snippet {
        Snippet {
            title: title.into(),
            language: "Rust".into(),
            code: code.into(),
        }
    }

    #[test]
    fn merge_skips_exact_duplicates() {
        let mut a = vec![snip("one", "1")];
        assert_eq!(merge(&mut a, vec![snip("one", "1"), snip("two", "2")]), 1);
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn search_matches_code_too() {
        let dir = std::env::temp_dir().join(format!("snip-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("snippets.json");
        save(
            &path,
            &[
                snip("Read File", "fs::read_to_string"),
                snip("HTTP", "requests.get"),
            ],
        )
        .unwrap();
        let mut app = App::new(path, None).unwrap();
        app.search = "requests".into();
        app.update_filter();
        assert_eq!(app.filtered, vec![1]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_file_is_reported() {
        let path = std::env::temp_dir().join(format!("snip-bad-{}.json", std::process::id()));
        fs::write(&path, "[{oops").unwrap();
        assert!(load(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
