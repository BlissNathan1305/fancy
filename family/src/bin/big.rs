use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Paragraph},
};
use std::io::{self, stdout};

struct Calculator {
    input: String,
    result: String,
}

fn main() -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    // App state
    let mut app = Calculator {
        input: String::new(),
        result: String::from("0"),
    };

    // Main UI Loop
    loop {
        terminal.draw(|f| ui(f, &app))?;

        // Handle key inputs
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char(c) if c.is_digit(10) || "+-*/.".contains(c) => {
                    app.input.push(c);
                }
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Enter => {
                    if !app.input.is_empty() {
                        // Note: Using a lightweight eval approach. 
                        // Real math parsing usually requires a crate like 'evalexpr'.
                        app.result = evaluate_simple_math(&app.input);
                    }
                }
                KeyCode::Char('c') => {
                    app.input.clear();
                    app.result = String::from("0");
                }
                _ => {}
            }
        }
    }

    // Restore terminal settings safely
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, app: &Calculator) {
    let size = f.area();

    // Create a centered bounding box for our calculator interface
    let calc_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Display screen
            Constraint::Length(8), // Button Pad
            Constraint::Min(0),    // Instructions space
        ])
        .margin(1)
        .split(size);

    // 1. Render the Display Widget
    let display_text = format!(" {} = {}", app.input, app.result);
    let display = Paragraph::new(display_text)
        .alignment(Alignment::Right)
        .block(Block::default().borders(Borders::ALL).title(" Calculator "));
    f.render_widget(display, calc_layout[0]);

    // 2. Render the Keypad Layout
    let keypad_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(calc_layout[1]);

    // Helper macro to draw grids easily
    let keys = [
        ["7", "8", "9", "/"],
        ["4", "5", "6", "*"],
        ["1", "2", "3", "-"],
        ["C", "0", "=", "+"],
    ];

    for (row_idx, row) in keys.iter().enumerate() {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(keypad_rows[row_idx]);

        for (col_idx, &key) in row.iter().enumerate() {
            let cell = Paragraph::new(key)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Cyan)));
            f.render_widget(cell, cols[col_idx]);
        }
    }

    // 3. Bottom instructions
    let instructions = Paragraph::new("Press keys to type | [Enter]: Calculate | [C]: Clear | [Q]: Quit")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(instructions, calc_layout[2]);
}

/// A very primitive parser just to demo calculations without adding extra dependencies
fn evaluate_simple_math(input: &str) -> String {
    let operators = ['+', '-', '*', '/'];
    if let Some(op_idx) = input.find(|c| operators.contains(&c)) {
        let op = input.chars().nth(op_idx).unwrap();
        let parts: Vec<&str> = input.split(op).collect();
        if parts.len() == 2 {
            let num1: f64 = parts[0].trim().parse().unwrap_or(0.0);
            let num2: f64 = parts[1].trim().parse().unwrap_or(0.0);
            let res = match op {
                '+' => num1 + num2,
                '-' => num1 - num2,
                '*' => num1 * num2,
                '/' => {
                    if num2 == 0.0 { return "Error: / 0".to_string(); }
                    num1 / num2
                }
                _ => 0.0,
            };
            return res.to_string();
        }
    }
    "Syntax Error".to_string()
}
