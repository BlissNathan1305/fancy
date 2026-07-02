use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{CrosstermBackend, Backend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Row, Table, Cell},
    Terminal,
};
use evalexpr::eval;
use std::{error::Error, io};

struct App {
    input: String,
    history: Vec<(String, String)>, // Stores (Expression, Result)
    last_result: String,
}

impl App {
    fn new() -> App {
        App {
            input: String::new(),
            history: Vec::new(),
            last_result: String::from("0"),
        }
    }

    fn execute_calculation(&mut self) {
        if self.input.is_empty() {
            return;
        }

        // Evaluate expression using evalexpr
        match eval(&self.input) {
            Ok(value) => {
                let res_str = value.to_string();
                self.history.push((self.input.clone(), res_str.clone()));
                self.last_result = res_str;
                self.input.clear();
            }
            Err(_) => {
                self.last_result = String::from("SYNTAX ERROR");
            }
        }
        
        // Keep history manageable
        if self.history.len() > 5 {
            self.history.remove(0);
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
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Enter => app.execute_calculation(),
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Char('c') => {
                    app.input.clear();
                    app.last_result = String::from("0");
                }
                KeyCode::Char(c) => {
                    // Filter down to valid calculator character primitives
                    if "0123456789+-*/().".contains(c) {
                        app.input.push(c);
                    }
                }
                _ => {}
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Top Banner
            Constraint::Min(1),    // Main workspace
            Constraint::Length(3), // Footer guide
        ].as_ref())
        .split(f.size());

    // 1. Header Banner
    let header = Paragraph::new("⚡ QUANTUM MATRIX CALCULATOR ⚡")
        .style(Style::default().fg(Color::Rgb(0, 255, 255)).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Rgb(255, 0, 128))));
    f.render_widget(header, main_layout[0]);

    // Split workspace into Display Stack (Top) and Layout Grid Pad (Bottom)
    let workspace = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(45), // History & Current Display
            Constraint::Percentage(55), // Matrix Keypad layout
        ].as_ref())
        .split(main_layout[1]);

    // 2. Calculation History & Output Screen
    let mut monitor_text = String::new();
    for (expr, res) in &app.history {
        monitor_text.push_str(&format!("  {} = {}\n", expr, res));
    }
    monitor_text.push_str(&format!("\n👉 IN : {}\n== OUT: {}", app.input, app.last_result));

    let display_screen = Paragraph::new(monitor_text)
        .style(Style::default().fg(Color::Rgb(0, 255, 128)))
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Matrix IO Core ")
            .title_style(Style::default().fg(Color::Rgb(255, 255, 0)))
            .border_style(Style::default().fg(Color::Rgb(138, 43, 226))));
    f.render_widget(display_screen, workspace[0]);

    // 3. Decorative Cyber Matrix Button Pad Layout
    let keypad_data = vec![
        vec!["7", "8", "9", "/"],
        vec!["4", "5", "6", "*"],
        vec!["1", "2", "3", "-"],
        vec!["0", ".", "C", "+"],
    ];

    let keypad_rows: Vec<Row> = keypad_data.iter().map(|row| {
        let cells: Vec<Cell> = row.iter().map(|key| {
            let color = match *key {
                "+" | "-" | "*" | "/" => Color::Rgb(255, 0, 128), // Hot operators
                "C" => Color::Rgb(255, 165, 0),                  // Vibrant clear command
                _ => Color::Rgb(0, 191, 255),                     // Electric blue numbers
            };
            Cell::from(format!("  {}  ", key))
                .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        }).collect();
        Row::new(cells).height(1)
    }).collect();

    let keypad_matrix = Table::new(
        keypad_rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ]
    )
    .block(Block::default()
        .borders(Borders::ALL)
        .title(" Input Matrix Reference ")
        .title_style(Style::default().fg(Color::Rgb(200, 200, 200)))
        .border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(keypad_matrix, workspace[1]);

    // 4. Instructions Menu
    let footer_text = "Type characters directly | Enter: Evaluate (=) | c: Clear Screen | q: Exit Core";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Rgb(150, 150, 150)))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(footer, main_layout[2]);
}
