use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
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
use std::{error::Error, io};

struct TodoItem {
    title: String,
    done: bool,
}

struct App {
    items: Vec<TodoItem>,
    state: ListState,
    input: String,
}

impl App {
    fn new() -> App {
        let mut state = ListState::default();
        if !state.selected().is_some() {
            state.select(Some(0));
        }
        App {
            items: vec![
                TodoItem { title: "Buy groceries".to_string(), done: false },
                TodoItem { title: "Finish Rust TUI app".to_string(), done: true },
            ],
            state,
            input: String::new(),
        }
    }

    fn next(&mut self) {
        if self.items.is_empty() { return; }
        let i = match self.state.selected() {
            Some(i) => if i >= self.items.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.items.is_empty() { return; }
        let i = match self.state.selected() {
            Some(i) => if i == 0 { self.items.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn toggle_done(&mut self) {
        if let Some(i) = self.state.selected() {
            if i < self.items.len() {
                self.items[i].done = !self.items[i].done;
            }
        }
    }

    fn delete_item(&mut self) {
        if let Some(i) = self.state.selected() {
            if !self.items.is_empty() {
                self.items.remove(i);
                if self.items.is_empty() {
                    self.state.select(None);
                } else if i >= self.items.len() {
                    self.state.select(Some(self.items.len() - 1));
                }
            }
        }
    }

    fn add_item(&mut self) {
        if !self.input.trim().is_empty() {
            self.items.push(TodoItem {
                title: self.input.trim().to_string(),
                done: false,
            });
            self.input.clear();
            if self.state.selected().is_none() {
                self.state.select(Some(0));
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal settings safely
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
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => app.next(),
                KeyCode::Up | KeyCode::Char('k') => app.previous(),
                KeyCode::Enter => app.toggle_done(),
                KeyCode::Char('d') => app.delete_item(),
                // Typing text into input box
                KeyCode::Char(c) => app.input.push(c),
                KeyCode::Backspace => { app.input.pop(); },
                // Submit new item
                KeyCode::Tab => app.add_item(),
                _ => {}
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    // Create a 3-part vertical layout: Input, List, Help Bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input box
            Constraint::Min(5),    // Todo List
            Constraint::Length(3), // Help text bar
        ])
        .split(f.area());

    // 1. Render Input Box
    let input_block = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(" New Todo (Type + Press Tab to Add) "));
    f.render_widget(input_block, chunks[0]);

    // 2. Build and Render Todo List items
    let items: Vec<ListItem> = app
        .items
        .iter()
        .map(|i| {
            let status = if i.done { "[✓] " } else { "[ ] " };
            let style = if i.done {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::CROSSED_OUT)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("{}{}", status, i.title)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Tasks "))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

    // Note: We statefully render the list to handle active highlighting row memory
    f.render_stateful_widget(list, chunks[1], &mut app.state);

    // 3. Render Help Bar
    let help_message = Paragraph::new("• j/k or Up/Down to navigate  • Enter to toggle done  • d to delete  • q to quit")
        .block(Block::default().borders(Borders::ALL).title(" Controls "));
    f.render_widget(help_message, chunks[2]);
}
