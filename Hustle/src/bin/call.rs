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

enum AppMode {
    Normal,
    Insert,
}

struct TodoItem {
    title: String,
    done: bool,
}

struct App {
    items: Vec<TodoItem>,
    state: ListState,
    input: String,
    mode: AppMode,
}

impl App {
    fn new() -> App {
        let mut state = ListState::default();
        state.select(Some(0));
        App {
            items: vec![
                TodoItem { title: "Configure development server".to_string(), done: false },
                TodoItem { title: "Test stateful modal TUI".to_string(), done: false },
            ],
            state,
            input: String::new(),
            mode: AppMode::Normal,
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
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
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
            match app.mode {
                AppMode::Normal => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('a') => {
                        app.mode = AppMode::Insert;
                    }
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Char(' ') => { // Spacebar to toggle
                        if let Some(i) = app.state.selected() {
                            if i < app.items.len() {
                                app.items[i].done = !app.items[i].done;
                            }
                        }
                    }
                    KeyCode::Char('d') => {
                        if let Some(i) = app.state.selected() {
                            if !app.items.is_empty() {
                                app.items.remove(i);
                                if app.items.is_empty() {
                                    app.state.select(None);
                                } else if i >= app.items.len() {
                                    app.state.select(Some(app.items.len() - 1));
                                }
                            }
                        }
                    }
                    _ => {}
                },
                AppMode::Insert => match key.code {
                    KeyCode::Enter => {
                        if !app.input.trim().is_empty() {
                            app.items.push(TodoItem {
                                title: app.input.trim().to_string(),
                                done: false,
                            });
                            app.input.clear();
                            app.mode = AppMode::Normal;
                            if app.state.selected().is_none() {
                                app.state.select(Some(0));
                            }
                        }
                    }
                    KeyCode::Esc => {
                        app.input.clear();
                        app.mode = AppMode::Normal;
                    }
                    KeyCode::Char(c) => {
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
    let (input_border_color, input_title) = match app.mode {
        AppMode::Normal => (Color::DarkGray, " New Task [Locked - Press 'a' to Add] "),
        AppMode::Insert => (Color::Green, " Writing New Task... [Press Enter to Save, Esc to Cancel] "),
    };

    // 1. Input Field
    let input_block = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(input_title).border_style(Style::default().fg(input_border_color)));
    f.render_widget(input_block, chunks[0]);

    // 2. Task List
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
        .block(Block::default().borders(Borders::ALL).title(" Active Tasks "))
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(list, chunks[1], &mut app.state);

    // 3. Status Footer Controls Bar
    let help_text = match app.mode {
        AppMode::Normal => "• a: Add Task  • j/k: Move  • Space: Toggle Done  • d: Delete  • q: Quit",
        AppMode::Insert => "• Enter: Save Task  • Esc: Return to Navigation Mode",
    };
    
    let footer = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title(" Controls "));
    f.render_widget(footer, chunks[2]);
}
