use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs, io, path::PathBuf};
use tui_snippets::{home_dir, term, write_atomic};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct TodoItem {
    title: String,
    completed: bool,
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Adding,
    Editing,
    ConfirmDelete,
}

struct App {
    path: PathBuf,
    todos: Vec<TodoItem>,
    state: ListState,
    input: String,
    mode: Mode,
    status: String,
}

impl App {
    fn new(path: PathBuf) -> Result<App, Box<dyn Error>> {
        let todos = load(&path)?;
        let mut state = ListState::default();
        state.select(if todos.is_empty() { None } else { Some(0) });
        Ok(App {
            path,
            todos,
            state,
            input: String::new(),
            mode: Mode::Normal,
            status: String::new(),
        })
    }

    fn save(&mut self) {
        let result = serde_json::to_vec_pretty(&self.todos)
            .map_err(io::Error::other)
            .and_then(|data| write_atomic(&self.path, &data));
        if let Err(e) = result {
            self.status = format!("Save failed: {e}");
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.todos.is_empty() {
            return;
        }
        let len = self.todos.len() as isize;
        let current = self.state.selected().unwrap_or(0) as isize;
        self.state
            .select(Some((current + delta).rem_euclid(len) as usize));
    }

    /// Move the selected task up or down the list.
    fn reorder(&mut self, delta: isize) {
        let Some(i) = self.state.selected() else {
            return;
        };
        let j = i as isize + delta;
        if j < 0 || j as usize >= self.todos.len() {
            return;
        }
        self.todos.swap(i, j as usize);
        self.state.select(Some(j as usize));
        self.save();
    }

    fn toggle(&mut self) {
        if let Some(item) = self.state.selected().and_then(|i| self.todos.get_mut(i)) {
            item.completed = !item.completed;
            self.save();
        }
    }

    fn submit_input(&mut self) {
        let title = self.input.trim().to_string();
        if title.is_empty() {
            self.mode = Mode::Normal;
            return;
        }
        match self.mode {
            Mode::Adding => {
                self.todos.push(TodoItem {
                    title,
                    completed: false,
                });
                self.state.select(Some(self.todos.len() - 1));
            }
            Mode::Editing => {
                if let Some(item) = self.state.selected().and_then(|i| self.todos.get_mut(i)) {
                    item.title = title;
                }
            }
            _ => {}
        }
        self.input.clear();
        self.mode = Mode::Normal;
        self.save();
    }

    fn delete_selected(&mut self) {
        let Some(i) = self.state.selected() else {
            return;
        };
        if i >= self.todos.len() {
            return;
        }
        let removed = self.todos.remove(i);
        self.status = format!("Deleted \"{}\"", removed.title);
        if self.todos.is_empty() {
            self.state.select(None);
        } else if i >= self.todos.len() {
            self.state.select(Some(self.todos.len() - 1));
        }
        self.save();
    }

    fn clear_completed(&mut self) {
        let before = self.todos.len();
        self.todos.retain(|t| !t.completed);
        let removed = before - self.todos.len();
        self.status = format!("Cleared {removed} completed task(s)");
        self.state
            .select(if self.todos.is_empty() { None } else { Some(0) });
        self.save();
    }
}

fn storage_path() -> PathBuf {
    home_dir().join(".todo_list.json")
}

/// A missing file means an empty list. A file that exists but cannot be parsed
/// is an error, so the user's tasks are never silently replaced.
fn load(path: &PathBuf) -> Result<Vec<TodoItem>, Box<dyn Error>> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Vec::new()),
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            format!(
                "{} is not valid todo JSON ({e}); fix or move it and try again",
                path.display()
            )
            .into()
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new(storage_path())?;
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
        match app.mode {
            Mode::Normal => {
                app.status.clear();
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                    KeyCode::Char('J') => app.reorder(1),
                    KeyCode::Char('K') => app.reorder(-1),
                    KeyCode::Char(' ') | KeyCode::Enter => app.toggle(),
                    KeyCode::Char('a') | KeyCode::Char('i') => {
                        app.input.clear();
                        app.mode = Mode::Adding;
                    }
                    KeyCode::Char('e') => {
                        if let Some(item) = app.state.selected().and_then(|i| app.todos.get(i)) {
                            app.input = item.title.clone();
                            app.mode = Mode::Editing;
                        }
                    }
                    KeyCode::Char('d') if app.state.selected().is_some() => {
                        app.mode = Mode::ConfirmDelete
                    }
                    KeyCode::Char('c') => app.clear_completed(),
                    _ => {}
                }
            }
            Mode::Adding | Mode::Editing => match key.code {
                KeyCode::Enter => app.submit_input(),
                KeyCode::Esc => {
                    app.input.clear();
                    app.mode = Mode::Normal;
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
                if key.code == KeyCode::Char('y') {
                    app.delete_selected();
                }
                app.mode = Mode::Normal;
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(f.size());

    let header = Paragraph::new("⚡ CYBERPUNK RUST TODO ⚡")
        .style(
            Style::default()
                .fg(Color::Rgb(255, 0, 128))
                .add_modifier(Modifier::BOLD),
        )
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
    f.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = app
        .todos
        .iter()
        .map(|todo| {
            let (status, style) = if todo.completed {
                (
                    "[✔] ",
                    Style::default()
                        .fg(Color::Rgb(0, 255, 128))
                        .add_modifier(Modifier::DIM | Modifier::CROSSED_OUT),
                )
            } else {
                ("[ ] ", Style::default().fg(Color::Rgb(255, 255, 0)))
            };
            ListItem::new(format!("{}{}", status, todo.title)).style(style)
        })
        .collect();

    let done = app.todos.iter().filter(|t| t.completed).count();
    let todo_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Tasks — {}/{} done (saved automatically) ",
                    done,
                    app.todos.len()
                ))
                .title_style(Style::default().fg(Color::Rgb(255, 165, 0)))
                .border_style(Style::default().fg(Color::Rgb(138, 43, 226))),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(0, 191, 255))
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(todo_list, chunks[1], &mut app.state);

    let (input_title, input_text, input_color) = match app.mode {
        Mode::Adding => (
            " New task (Enter: add | Esc: cancel) ",
            format!("{}▏", app.input),
            Color::Rgb(0, 255, 255),
        ),
        Mode::Editing => (
            " Edit task (Enter: save | Esc: cancel) ",
            format!("{}▏", app.input),
            Color::Rgb(0, 255, 255),
        ),
        Mode::ConfirmDelete => (
            " Confirm ",
            "Delete the selected task? y: yes | any other key: no".to_string(),
            Color::Rgb(255, 80, 80),
        ),
        Mode::Normal => (" Status ", app.status.clone(), Color::Rgb(150, 150, 150)),
    };
    let input_block = Paragraph::new(input_text)
        .style(Style::default().fg(input_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(input_title)
                .border_style(Style::default().fg(Color::Rgb(255, 69, 0))),
        );
    f.render_widget(input_block, chunks[2]);

    let footer_text = "a: Add | e: Edit | Space: Done | d: Delete | J/K: Reorder | c: Clear done | j/k: Move | q: Quit";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Rgb(200, 200, 200)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    f.render_widget(footer, chunks[3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rtodo-test-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn missing_file_is_empty_list() {
        assert!(load(&temp_path("missing")).unwrap().is_empty());
    }

    #[test]
    fn corrupt_file_is_an_error_not_a_reset() {
        let path = temp_path("corrupt");
        fs::write(&path, "{not json").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{not json");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn add_toggle_reorder_round_trip() {
        let path = temp_path("roundtrip");
        let mut app = App::new(path.clone()).unwrap();
        for title in ["quick fix", "deploy"] {
            app.mode = Mode::Adding;
            app.input = title.to_string();
            app.submit_input();
        }
        app.toggle();
        app.reorder(-1);
        let saved = load(&path).unwrap();
        assert_eq!(
            saved[0],
            TodoItem {
                title: "deploy".into(),
                completed: true
            }
        );
        assert_eq!(saved[1].title, "quick fix");
        fs::remove_file(path).unwrap();
    }
}
