use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs, io,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(PartialEq)]
enum AppMode {
    Normal,
    Insert,
    Edit,
    ConfirmDelete,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct TodoItem {
    title: String,
    done: bool,
}

struct App {
    path: PathBuf,
    items: Vec<TodoItem>,
    state: ListState,
    input: String,
    mode: AppMode,
    status: String,
}

/// `$XDG_DATA_HOME/call/tasks.json`, usually `~/.local/share/call/tasks.json`.
fn storage_path() -> io::Result<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("call");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("tasks.json"))
}

/// A missing file is an empty list; an unreadable one is an error rather than a silent reset.
fn load(path: &Path) -> Result<Vec<TodoItem>, Box<dyn Error>> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Vec::new()),
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} is not valid task JSON ({e}); fix or move it and try again",
                path.display()
            )
            .into()
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

fn save(path: &Path, items: &[TodoItem]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(&serde_json::to_vec_pretty(items).map_err(io::Error::other)?)?;
    file.sync_all()?;
    fs::rename(tmp, path)
}

impl App {
    fn new(path: PathBuf) -> Result<App, Box<dyn Error>> {
        let items = load(&path)?;
        let mut state = ListState::default();
        state.select(if items.is_empty() { None } else { Some(0) });
        Ok(App {
            path,
            items,
            state,
            input: String::new(),
            mode: AppMode::Normal,
            status: String::new(),
        })
    }

    fn persist(&mut self) {
        if let Err(e) = save(&self.path, &self.items) {
            self.status = format!("Save failed: {e}");
        }
    }

    fn submit_input(&mut self) {
        let title = self.input.trim().to_string();
        if title.is_empty() {
            return;
        }
        if self.mode == AppMode::Edit {
            if let Some(item) = self.state.selected().and_then(|i| self.items.get_mut(i)) {
                item.title = title;
            }
        } else {
            self.items.push(TodoItem { title, done: false });
            self.state.select(Some(self.items.len() - 1));
        }
        self.input.clear();
        self.mode = AppMode::Normal;
        self.persist();
    }

    fn delete_selected(&mut self) {
        let Some(i) = self.state.selected() else {
            return;
        };
        if i >= self.items.len() {
            return;
        }
        let removed = self.items.remove(i);
        self.status = format!("Deleted \"{}\"", removed.title);
        if self.items.is_empty() {
            self.state.select(None);
        } else if i >= self.items.len() {
            self.state.select(Some(self.items.len() - 1));
        }
        self.persist();
    }

    fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new(storage_path()?)?;

    // Put the terminal back even if something panics.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        original_hook(info);
    }));
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let res = run_app(&mut terminal, &mut app);
    restore_terminal()?;
    res?;
    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match app.mode {
                AppMode::Normal => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('a') => {
                        app.status.clear();
                        app.mode = AppMode::Insert;
                    }
                    KeyCode::Char('e') => {
                        if let Some(item) = app.state.selected().and_then(|i| app.items.get(i)) {
                            app.input = item.title.clone();
                            app.mode = AppMode::Edit;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        if let Some(item) = app.state.selected().and_then(|i| app.items.get_mut(i))
                        {
                            item.done = !item.done;
                            app.persist();
                        }
                    }
                    KeyCode::Char('d') if app.state.selected().is_some() => {
                        app.mode = AppMode::ConfirmDelete
                    }
                    _ => {}
                },
                AppMode::ConfirmDelete => {
                    app.mode = AppMode::Normal;
                    if key.code == KeyCode::Char('y') {
                        app.delete_selected();
                    }
                }
                AppMode::Insert | AppMode::Edit => match key.code {
                    KeyCode::Enter => app.submit_input(),
                    KeyCode::Esc => {
                        app.input.clear();
                        app.mode = AppMode::Normal;
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.input.clear()
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    _ => {}
                },
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input Block
            Constraint::Min(5),    // List Block
            Constraint::Length(3), // Status Control Block
        ])
        .split(f.size());

    // Context colors depending on active UI Mode
    let (input_border_color, input_title, input_text) = match app.mode {
        AppMode::Normal => (
            Color::DarkGray,
            " New Task [Press 'a' to Add] ",
            app.status.clone(),
        ),
        AppMode::Insert => (
            Color::Green,
            " Writing New Task... [Enter: Save, Esc: Cancel] ",
            format!("{}▏", app.input),
        ),
        AppMode::Edit => (
            Color::Green,
            " Editing Task... [Enter: Save, Esc: Cancel] ",
            format!("{}▏", app.input),
        ),
        AppMode::ConfirmDelete => (
            Color::Red,
            " Confirm ",
            "Delete the selected task? y: yes | any other key: no".to_string(),
        ),
    };

    // 1. Input Field
    let input_block = Paragraph::new(input_text)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(input_title)
                .border_style(Style::default().fg(input_border_color)),
        );
    f.render_widget(input_block, chunks[0]);

    // 2. Task List
    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|i| {
            let status = if i.done { "[✓] " } else { "[ ] " };
            let style = if i.done {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::CROSSED_OUT)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("{}{}", status, i.title)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Active Tasks ({} open, {} done) ",
            app.items.iter().filter(|i| !i.done).count(),
            app.items.iter().filter(|i| i.done).count()
        )))
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(list, chunks[1], &mut app.state);

    // 3. Status Footer Controls Bar
    let help_text = match app.mode {
        AppMode::Normal => {
            "• a: Add  • e: Edit  • j/k: Move  • Space: Toggle Done  • d: Delete  • q: Quit"
        }
        AppMode::Insert | AppMode::Edit => "• Enter: Save Task  • Esc: Return to Navigation Mode",
        AppMode::ConfirmDelete => "• y: Delete  • any other key: Cancel",
    };

    let footer =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title(" Controls "));
    f.render_widget(footer, chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tasks_persist_between_runs() {
        let path = std::env::temp_dir().join(format!("call-test-{}.json", std::process::id()));
        let mut app = App::new(path.clone()).unwrap();
        app.mode = AppMode::Insert;
        app.input = "Ship it".into();
        app.submit_input();
        let reopened = App::new(path.clone()).unwrap();
        assert_eq!(
            reopened.items,
            vec![TodoItem {
                title: "Ship it".into(),
                done: false
            }]
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn corrupt_file_is_not_overwritten() {
        let path = std::env::temp_dir().join(format!("call-bad-{}.json", std::process::id()));
        fs::write(&path, "oops").unwrap();
        assert!(App::new(path.clone()).is_err());
        fs::remove_file(path).unwrap();
    }
}
