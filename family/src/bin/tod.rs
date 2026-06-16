use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, stdout};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TodoItem {
    title: String,
    completed: bool,
}

enum InputMode {
    Normal,
    Editing,
}

struct App {
    todos: Vec<TodoItem>,
    list_state: ListState,
    input_mode: InputMode,
    input_buffer: String,
}

const FILE_PATH: &str = "tui_todo.json";

fn main() -> io::Result<()> {
    // 1. Setup Terminal Settings
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // 2. Initialize App State
    let loaded_todos = load_todos(FILE_PATH).unwrap_or_else(|_| Vec::new());
    let mut app = App {
        todos: loaded_todos,
        list_state: ListState::default(),
        input_mode: InputMode::Normal,
        input_buffer: String::new(),
    };
    
    // Automatically highlight the first item if the list isn't empty
    if !app.todos.is_empty() {
        app.list_state.select(Some(0));
    }

    // 3. Main Event Loop
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('a') => {
                        app.input_mode = InputMode::Editing;
                    }
                    // Toggle Completed status
                    KeyCode::Char(' ') => {
                        if let Some(index) = app.list_state.selected() {
                            if !app.todos.is_empty() {
                                app.todos[index].completed = !app.todos[index].completed;
                            }
                        }
                    }
                    // Delete selected task
                    KeyCode::Char('d') => {
                        if let Some(index) = app.list_state.selected() {
                            if !app.todos.is_empty() {
                                app.todos.remove(index);
                                // Adjust selected index safely after deletion
                                if app.todos.is_empty() {
                                    app.list_state.select(None);
                                } else if index >= app.todos.len() {
                                    app.list_state.select(Some(app.todos.len() - 1));
                                }
                            }
                        }
                    }
                    // Navigate Up
                    KeyCode::Up | KeyCode::Char('k') => {
                        if !app.todos.is_empty() {
                            let i = match app.list_state.selected() {
                                Some(i) => {
                                    if i == 0 { app.todos.len() - 1 } else { i - 1 }
                                }
                                None => 0,
                            };
                            app.list_state.select(Some(i));
                        }
                    }
                    // Navigate Down
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !app.todos.is_empty() {
                            let i = match app.list_state.selected() {
                                Some(i) => {
                                    if i >= app.todos.len() - 1 { 0 } else { i + 1 }
                                }
                                None => 0,
                            };
                            app.list_state.select(Some(i));
                        }
                    }
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter => {
                        let trimmed = app.input_buffer.trim();
                        if !trimmed.is_empty() {
                            app.todos.push(TodoItem {
                                title: trimmed.to_string(),
                                completed: false,
                            });
                            // If it's the first element, select it
                            if app.todos.len() == 1 {
                                app.list_state.select(Some(0));
                            }
                        }
                        app.input_buffer.clear();
                        app.input_mode = InputMode::Normal;
                    }
                    KeyCode::Esc => {
                        app.input_buffer.clear();
                        app.input_mode = InputMode::Normal;
                    }
                    KeyCode::Char(c) => {
                        app.input_buffer.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input_buffer.pop();
                    }
                    _ => {}
                },
            }
        }
    }

    // 4. Save and Cleanup Terminal
    let _ = save_todos(FILE_PATH, &app.todos);
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Divide screen into layout components
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input Block
            Constraint::Min(0),    // Todo List Block
            Constraint::Length(3), // Help/Instructions Footer
        ])
        .split(size);

    // 1. Render Input Box
    let input_title = match app.input_mode {
        InputMode::Normal => " [A] Add New Task ",
        InputMode::Editing => " Type Task & Press Enter (Esc to Cancel) ",
    };
    
    let input_block = Block::default().borders(Borders::ALL).title(input_title);
    let input_paragraph = Paragraph::new(app.input_buffer.as_str())
        .block(input_block)
        .style(match app.input_mode {
            InputMode::Normal => Style::default().fg(Color::DarkGray),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        });
    f.render_widget(input_paragraph, chunks[0]);

    // 2. Map and Render Todo List Items
    let items: Vec<ListItem> = app
        .todos
        .iter()
        .map(|item| {
            let status = if item.completed { "[X]" } else { "[ ]" };
            let style = if item.completed {
                Style::default().fg(Color::Green).add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("{}  {}", status, item.title)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Tasks "))
        .highlight_style(
            Style::default()
                .bg(Color::Indexed(8)) // Highlight target background color
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    // We must pass the mutable list_state tracking what row is active
    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // 3. Render Footer Help Commands
    let footer_text = match app.input_mode {
        InputMode::Normal => " [↑/↓ or j/k]: Scroll | [Space]: Toggle | [d]: Delete | [q]: Quit ",
        InputMode::Editing => " Typing... [Enter]: Save | [Esc]: Back to List ",
    };
    let footer = Paragraph::new(footer_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::DarkGray)));
    f.render_widget(footer, chunks[2]);
}

// Storage Helpers
fn load_todos<P: AsRef<Path>>(path: P) -> Result<Vec<TodoItem>, io::Error> {
    let file = File::open(path)?;
    let list = serde_json::from_reader(file)?;
    Ok(list)
}

fn save_todos<P: AsRef<Path>>(path: P, list: &Vec<TodoItem>) -> Result<(), io::Error> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, list)?;
    Ok(())
}
