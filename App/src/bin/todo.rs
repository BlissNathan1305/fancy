use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{CrosstermBackend, Backend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, io, fs::File, path::PathBuf};

#[derive(Serialize, Deserialize, Clone)]
struct TodoItem {
    title: String,
    completed: bool,
}

struct App {
    todos: Vec<TodoItem>,
    state: ListState,
    input: String,
}

impl App {
    fn new() -> App {
        let mut state = ListState::default();
        state.select(Some(0));
        
        let todos = Self::load_from_disk().unwrap_or_else(|_| vec![
            TodoItem { title: "Buy neon spray paint".to_string(), completed: false },
            TodoItem { title: "Code a Rust TUI app".to_string(), completed: true },
            TodoItem { title: "Make everything super colorful".to_string(), completed: false },
        ]);

        App {
            todos,
            state,
            input: String::new(),
        }
    }

    fn storage_path() -> PathBuf {
        let mut path = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        path.push(".todo_list.json");
        path
    }

    fn load_from_disk() -> Result<Vec<TodoItem>, Box<dyn Error>> {
        let path = Self::storage_path();
        if !path.exists() {
            return Err("File does not exist".into());
        }
        let file = File::open(path)?;
        let todos = serde_json::from_reader(file)?;
        Ok(todos)
    }

    fn save_to_disk(&self) -> Result<(), Box<dyn Error>> {
        let path = Self::storage_path();
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &self.todos)?;
        Ok(())
    }

    fn next(&mut self) {
        if self.todos.is_empty() { return; }
        let i = match self.state.selected() {
            Some(i) => if i >= self.todos.len() - 1 { 0 } else { i + 1 }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.todos.is_empty() { return; }
        let i = match self.state.selected() {
            Some(i) => if i == 0 { self.todos.len() - 1 } else { i - 1 }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn toggle_todo(&mut self) {
        if let Some(i) = self.state.selected() {
            if i < self.todos.len() {
                self.todos[i].completed = !self.todos[i].completed;
                let _ = self.save_to_disk();
            }
        }
    }

    fn add_todo(&mut self) {
        if !self.input.is_empty() {
            self.todos.push(TodoItem {
                title: self.input.clone(),
                completed: false,
            });
            self.input.clear();
            self.state.select(Some(self.todos.len() - 1));
            let _ = self.save_to_disk();
        }
    }

    fn delete_todo(&mut self) {
        if let Some(i) = self.state.selected() {
            if !self.todos.is_empty() {
                self.todos.remove(i);
                if self.todos.is_empty() {
                    self.state.select(None);
                } else if i >= self.todos.len() {
                    self.state.select(Some(self.todos.len() - 1));
                }
                let _ = self.save_to_disk();
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => {
                    let _ = app.save_to_disk(); 
                    return Ok(());
                }
                // Arrow keys always navigate
                KeyCode::Down => app.next(),
                KeyCode::Up => app.previous(),
                
                // j and k only navigate if the input line is empty
                KeyCode::Char('j') if app.input.is_empty() => app.next(),
                KeyCode::Char('k') if app.input.is_empty() => app.previous(),
                
                KeyCode::Enter => {
                    if !app.input.is_empty() {
                        app.add_todo();
                    } else {
                        app.toggle_todo();
                    }
                }
                // Only trigger delete if not currently typing out a word ending in 'd'
                KeyCode::Char('d') if app.input.is_empty() => app.delete_todo(),
                
                // Catches everything else (including j, k, d when typing)
                KeyCode::Char(c) => {
                    app.input.push(c);
                }
                KeyCode::Backspace => {
                    app.input.pop();
                }
                _ => {}
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), 
                Constraint::Min(1),    
                Constraint::Length(3), 
                Constraint::Length(3), 
            ]
            .as_ref(),
        )
        .split(f.size());

    // Header
    let header = Paragraph::new("⚡ CYBERPUNK RUST TODO ⚡")
        .style(Style::default().fg(Color::Rgb(255, 0, 128)).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)));
    f.render_widget(header, chunks[0]);

    // List Items
    let items: Vec<ListItem> = app
        .todos
        .iter()
        .map(|todo| {
            let (status, style) = if todo.completed {
                ("[✔] ", Style::default().fg(Color::Rgb(0, 255, 128)).add_modifier(Modifier::DIM))
            } else {
                ("[ ] ", Style::default().fg(Color::Rgb(255, 255, 0)))
            };
            
            let content = format!("{}{}", status, todo.title);
            ListItem::new(content).style(style)
        })
        .collect();

    // List Container
    let todo_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Tasks (Saved Automatically) ")
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

    // Input box
    let input_block = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Rgb(0, 255, 255)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Type & press 'Enter' to add | Empty + 'Enter' to toggle ")
                .border_style(Style::default().fg(Color::Rgb(255, 69, 0))),
        );
    f.render_widget(input_block, chunks[2]);

    // Footer
    let footer_text = "▲/▼ or (j/k when empty): Move | Enter: Action | (d when empty): Delete | q: Quit";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Rgb(200, 200, 200)))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(footer, chunks[3]);
}
