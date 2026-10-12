use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::{error::Error, fs, io, path::PathBuf};
use tui_snippets::{home_dir, term, write_atomic};
use tui_textarea::TextArea;

enum Mode<'a> {
    Browse,
    Edit(Box<TextArea<'a>>),
    NewName(String),
    ConfirmDelete,
}

struct Note {
    title: String,
    content: String,
}

struct App<'a> {
    dir: PathBuf,
    notes: Vec<Note>,
    list_state: ListState,
    mode: Mode<'a>,
    scroll_offset: u16,
    status: String,
}

impl App<'_> {
    fn new() -> io::Result<Self> {
        let dir = home_dir().join(".rust_notes");
        fs::create_dir_all(&dir)?;
        let mut app = App {
            dir,
            notes: Vec::new(),
            list_state: ListState::default(),
            mode: Mode::Browse,
            scroll_offset: 0,
            status: "n: new note | e: edit | x: delete".to_string(),
        };
        app.load_all_notes();
        Ok(app)
    }

    fn load_all_notes(&mut self) {
        self.notes.clear();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                    let title = entry.file_name().to_string_lossy().into_owned();
                    if let Ok(content) = fs::read_to_string(&path) {
                        self.notes.push(Note { title, content });
                    }
                }
            }
        }
        self.sort_notes();
        self.list_state
            .select(if self.notes.is_empty() { None } else { Some(0) });
    }

    fn sort_notes(&mut self) {
        self.notes.sort_by_key(|n| n.title.to_lowercase());
    }

    fn selected(&self) -> Option<&Note> {
        self.list_state.selected().and_then(|i| self.notes.get(i))
    }

    fn select_title(&mut self, title: &str) {
        let idx = self.notes.iter().position(|n| n.title == title);
        self.list_state.select(idx);
        self.scroll_offset = 0;
    }

    fn save_selected(&mut self, content: String) {
        let Some(i) = self.list_state.selected() else {
            return;
        };
        let Some(note) = self.notes.get_mut(i) else {
            return;
        };
        let path = self.dir.join(&note.title);
        match write_atomic(&path, content.as_bytes()) {
            Ok(()) => {
                note.content = content;
                self.status = format!("Saved {}", note.title);
            }
            Err(e) => self.status = format!("Could not save {}: {e}", note.title),
        }
    }

    fn delete_selected(&mut self) {
        let Some(i) = self.list_state.selected() else {
            return;
        };
        if i >= self.notes.len() {
            return;
        }
        let path = self.dir.join(&self.notes[i].title);
        if let Err(e) = fs::remove_file(&path) {
            self.status = format!("Could not delete: {e}");
            return;
        }
        let removed = self.notes.remove(i);
        self.status = format!("Deleted {}", removed.title);
        if self.notes.is_empty() {
            self.list_state.select(None);
        } else if i >= self.notes.len() {
            self.list_state.select(Some(self.notes.len() - 1));
        }
    }

    /// Creates the note (or reopens an existing one with that name) and starts editing it.
    fn create_note(&mut self, raw_name: &str) {
        let name = match sanitize_title(raw_name) {
            Ok(name) => name,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        if !self.notes.iter().any(|n| n.title == name) {
            if let Err(e) = write_atomic(&self.dir.join(&name), b"") {
                self.status = format!("Could not create {name}: {e}");
                return;
            }
            self.notes.push(Note {
                title: name.clone(),
                content: String::new(),
            });
            self.sort_notes();
            self.status = format!("Created {name}");
        } else {
            self.status = format!("{name} already exists, opened it");
        }
        self.select_title(&name);
        self.start_editing();
    }

    fn start_editing(&mut self) {
        if let Some(note) = self.selected() {
            let mut textarea = TextArea::new(note.content.lines().map(String::from).collect());
            textarea.set_cursor_line_style(Style::default());
            self.mode = Mode::Edit(Box::new(textarea));
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.notes.is_empty() {
            return;
        }
        let len = self.notes.len() as isize;
        let current = self.list_state.selected().unwrap_or(0) as isize;
        self.list_state
            .select(Some((current + delta).rem_euclid(len) as usize));
        self.scroll_offset = 0;
    }
}

/// Note titles become file names, so keep them to a single plain path component.
fn sanitize_title(raw: &str) -> Result<String, String> {
    let mut name = raw.trim().to_string();
    if name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if name.contains('/') || name.contains('\\') || name.starts_with('.') {
        return Err("Name cannot contain '/' or start with '.'".to_string());
    }
    if !name.ends_with(".md") {
        name.push_str(".md");
    }
    Ok(name)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new()?;
    let mut terminal = term::init()?;
    let res = run_app(&mut terminal, &mut app);
    term::restore()?;
    res?;
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match &mut app.mode {
            Mode::Browse => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                KeyCode::Char('u') | KeyCode::PageUp => {
                    app.scroll_offset = app.scroll_offset.saturating_sub(5)
                }
                KeyCode::Char('d') | KeyCode::PageDown => {
                    let lines = app
                        .selected()
                        .map(|n| n.content.lines().count())
                        .unwrap_or(0) as u16;
                    app.scroll_offset = (app.scroll_offset + 5).min(lines.saturating_sub(1));
                }
                KeyCode::Char('e') | KeyCode::Enter => app.start_editing(),
                KeyCode::Char('n') => app.mode = Mode::NewName(String::new()),
                KeyCode::Char('x') if app.selected().is_some() => app.mode = Mode::ConfirmDelete,
                KeyCode::Char('r') => {
                    app.load_all_notes();
                    app.status = format!("Reloaded {} notes", app.notes.len());
                }
                _ => {}
            },
            Mode::Edit(textarea) => {
                let ctrl_s =
                    key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL);
                if key.code == KeyCode::Esc || ctrl_s {
                    let content = textarea.lines().join("\n");
                    if key.code == KeyCode::Esc {
                        app.mode = Mode::Browse;
                    }
                    app.save_selected(content);
                } else {
                    textarea.input(key);
                }
            }
            Mode::NewName(buffer) => match key.code {
                KeyCode::Esc => app.mode = Mode::Browse,
                KeyCode::Enter => {
                    let name = buffer.clone();
                    app.mode = Mode::Browse;
                    app.create_note(&name);
                }
                KeyCode::Backspace => {
                    buffer.pop();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    buffer.clear()
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    buffer.push(c)
                }
                _ => {}
            },
            Mode::ConfirmDelete => {
                if key.code == KeyCode::Char('y') {
                    app.delete_selected();
                } else {
                    app.status = "Delete cancelled".to_string();
                }
                app.mode = Mode::Browse;
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.size());

    let header = Paragraph::new("🧬 CYBERNETIC NEON NOTEBOOK 🧬")
        .style(
            Style::default()
                .fg(Color::Rgb(255, 0, 255))
                .add_modifier(Modifier::BOLD),
        )
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(0, 255, 255))),
        );
    f.render_widget(header, main_layout[0]);

    let workspace = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_layout[1]);

    let browsing = matches!(app.mode, Mode::Browse | Mode::ConfirmDelete);
    let notes_list: Vec<ListItem> = app
        .notes
        .iter()
        .map(|n| {
            ListItem::new(format!(" 📝 {}", n.title))
                .style(Style::default().fg(Color::Rgb(255, 255, 0)))
        })
        .collect();
    let list_widget = List::new(notes_list)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Document Index ({}) ", app.notes.len()))
                .title_style(Style::default().fg(Color::Rgb(0, 255, 128)))
                .border_style(Style::default().fg(if browsing {
                    Color::Rgb(138, 43, 226)
                } else {
                    Color::DarkGray
                })),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(255, 0, 128))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("👁 ");
    f.render_stateful_widget(list_widget, workspace[0], &mut app.list_state);

    let pane = workspace[1];
    match &mut app.mode {
        Mode::Edit(textarea) => {
            let title = app
                .list_state
                .selected()
                .and_then(|i| app.notes.get(i))
                .map(|n| format!(" EDITING {} (Esc: save & close | Ctrl+S: save) ", n.title))
                .unwrap_or_default();
            textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_style(
                        Style::default()
                            .fg(Color::Rgb(255, 69, 0))
                            .add_modifier(Modifier::BOLD),
                    )
                    .border_style(Style::default().fg(Color::Rgb(255, 69, 0))),
            );
            textarea.set_style(Style::default().fg(Color::Rgb(0, 255, 255)));
            f.render_widget(textarea.widget(), pane);
        }
        Mode::NewName(buffer) => {
            let prompt = Paragraph::new(format!("{buffer}▏"))
                .style(Style::default().fg(Color::White))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" New note name (Enter: create | Esc: cancel) ")
                        .border_style(Style::default().fg(Color::Rgb(255, 255, 0))),
                );
            f.render_widget(prompt, pane);
        }
        Mode::Browse | Mode::ConfirmDelete => {
            let body = match app.list_state.selected().and_then(|i| app.notes.get(i)) {
                Some(note) if note.content.is_empty() => "(empty note — press e to edit)".to_string(),
                Some(note) => note.content.clone(),
                None => "No notes yet.\n\nPress 'n' to create your first note.\nNotes are saved as Markdown files in ~/.rust_notes".to_string(),
            };
            let content = Paragraph::new(body)
                .style(Style::default().fg(Color::Rgb(200, 255, 200)))
                .wrap(Wrap { trim: false })
                .scroll((app.scroll_offset, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Read Pane (u/d to scroll) ")
                        .title_style(
                            Style::default()
                                .fg(Color::Rgb(0, 191, 255))
                                .add_modifier(Modifier::BOLD),
                        )
                        .border_style(Style::default().fg(Color::Rgb(0, 191, 255))),
                );
            f.render_widget(content, pane);
        }
    }

    let (footer_text, footer_color) = match &app.mode {
        Mode::ConfirmDelete => (
            format!(
                "Delete '{}'? This removes the file. y: yes | any other key: cancel",
                app.selected().map(|n| n.title.as_str()).unwrap_or("")
            ),
            Color::Rgb(255, 80, 80),
        ),
        Mode::Edit(_) => ("Arrows move | Ctrl+Z undo | Ctrl+S save | Esc save & close".to_string(), Color::Rgb(180, 180, 180)),
        Mode::NewName(_) => ("Type a file name, then Enter".to_string(), Color::Rgb(180, 180, 180)),
        Mode::Browse => (
            format!("j/k: Move | u/d: Scroll | n: New | e: Edit | x: Delete | r: Reload | q: Quit   — {}", app.status),
            Color::Rgb(180, 180, 180),
        ),
    };
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(footer_color))
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(footer, main_layout[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_sanitized() {
        assert_eq!(sanitize_title(" Ideas ").unwrap(), "Ideas.md");
        assert_eq!(sanitize_title("plan.md").unwrap(), "plan.md");
        assert!(sanitize_title("../etc/passwd").is_err());
        assert!(sanitize_title(".hidden").is_err());
        assert!(sanitize_title("   ").is_err());
    }
}
