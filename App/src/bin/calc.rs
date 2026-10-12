use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Terminal,
};
use std::{error::Error, io};
use tui_snippets::term;

const HISTORY_LEN: usize = 8;

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

        match evaluate(&self.input) {
            Ok(value) => {
                let res_str = format_number(value);
                self.history.push((self.input.clone(), res_str.clone()));
                self.last_result = res_str;
                self.input.clear();
            }
            Err(e) => {
                self.last_result = format!("ERROR: {e}");
            }
        }

        if self.history.len() > HISTORY_LEN {
            self.history.remove(0);
        }
    }
}

/// Evaluate with floating-point semantics. evalexpr treats `7/2` as integer
/// division, so every integer literal is rewritten as a float first.
fn evaluate(expr: &str) -> Result<f64, String> {
    let value = evalexpr::eval_float(&floatify(expr)).map_err(|e| match e {
        evalexpr::EvalexprError::DivisionError { .. } => "division by zero".to_string(),
        _ => "invalid expression".to_string(),
    })?;
    if value.is_infinite() {
        return Err("division by zero".to_string());
    }
    if value.is_nan() {
        return Err("undefined result".to_string());
    }
    Ok(value)
}

fn floatify(expr: &str) -> String {
    let mut out = String::with_capacity(expr.len() + 8);
    let mut number = String::new();
    let flush = |number: &mut String, out: &mut String| {
        if number.is_empty() {
            return;
        }
        if number.starts_with('.') {
            out.push('0');
        }
        out.push_str(number);
        if !number.contains('.') {
            out.push_str(".0");
        } else if number.ends_with('.') {
            out.push('0');
        }
        number.clear();
    };
    for c in expr.chars() {
        if c.is_ascii_digit() || c == '.' {
            number.push(c);
        } else {
            flush(&mut number, &mut out);
            out.push(c);
        }
    }
    flush(&mut number, &mut out);
    out
}

fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let abs = value.abs();
    if !(1e-9..1e15).contains(&abs) {
        return format!("{value:e}");
    }
    // 12 significant digits hides float noise such as 0.1 + 0.2 = 0.30000000000000004.
    let decimals = (11 - abs.log10().floor() as i32).max(0) as usize;
    let s = format!("{value:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // `calc "2 + 2"` prints the answer without opening the TUI.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        return match evaluate(&args.join(" ")) {
            Ok(value) => {
                println!("{}", format_number(value));
                Ok(())
            }
            Err(e) => {
                eprintln!("calc: {e}");
                std::process::exit(1);
            }
        };
    }

    let mut terminal = term::init()?;
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app);
    term::restore()?;
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
                KeyCode::Char(c) if "0123456789+-*/().^% ".contains(c) => {
                    app.input.push(c);
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
        .constraints(
            [
                Constraint::Length(3), // Top Banner
                Constraint::Min(1),    // Main workspace
                Constraint::Length(3), // Footer guide
            ]
            .as_ref(),
        )
        .split(f.size());

    // 1. Header Banner
    let header = Paragraph::new("⚡ QUANTUM MATRIX CALCULATOR ⚡")
        .style(
            Style::default()
                .fg(Color::Rgb(0, 255, 255))
                .add_modifier(Modifier::BOLD),
        )
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(255, 0, 128))),
        );
    f.render_widget(header, main_layout[0]);

    // Split workspace into Display Stack (Top) and Layout Grid Pad (Bottom)
    let workspace = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(45), // History & Current Display
                Constraint::Percentage(55), // Matrix Keypad layout
            ]
            .as_ref(),
        )
        .split(main_layout[1]);

    // 2. Calculation History & Output Screen
    let mut monitor_text = String::new();
    for (expr, res) in &app.history {
        monitor_text.push_str(&format!("  {} = {}\n", expr, res));
    }
    monitor_text.push_str(&format!(
        "\n👉 IN : {}\n== OUT: {}",
        app.input, app.last_result
    ));

    let display_screen = Paragraph::new(monitor_text)
        .style(Style::default().fg(Color::Rgb(0, 255, 128)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Matrix IO Core ")
                .title_style(Style::default().fg(Color::Rgb(255, 255, 0)))
                .border_style(Style::default().fg(Color::Rgb(138, 43, 226))),
        );
    f.render_widget(display_screen, workspace[0]);

    // 3. Decorative Cyber Matrix Button Pad Layout
    let keypad_data = [
        ["7", "8", "9", "/"],
        ["4", "5", "6", "*"],
        ["1", "2", "3", "-"],
        ["0", ".", "^", "+"],
        ["(", ")", "%", "C"],
    ];

    let keypad_rows: Vec<Row> = keypad_data
        .iter()
        .map(|row| {
            let cells: Vec<Cell> = row
                .iter()
                .map(|key| {
                    let color = match *key {
                        "+" | "-" | "*" | "/" | "^" | "%" => Color::Rgb(255, 0, 128), // Hot operators
                        "C" => Color::Rgb(255, 165, 0), // Vibrant clear command
                        _ => Color::Rgb(0, 191, 255),   // Electric blue numbers
                    };
                    Cell::from(format!("  {}  ", key))
                        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
                })
                .collect();
            Row::new(cells).height(1)
        })
        .collect();

    let keypad_matrix = Table::new(
        keypad_rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Input Matrix Reference ")
            .title_style(Style::default().fg(Color::Rgb(200, 200, 200)))
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(keypad_matrix, workspace[1]);

    // 4. Instructions Menu
    let footer_text =
        "Type: 0-9 + - * / ^ % ( ) | Enter: Evaluate | Backspace: Delete | c: Clear | q/Esc: Quit";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Rgb(150, 150, 150)))
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
    fn divides_as_floats() {
        assert_eq!(evaluate("7/2").unwrap(), 3.5);
        assert_eq!(evaluate("(1+2)*3").unwrap(), 9.0);
        assert_eq!(evaluate("2^10").unwrap(), 1024.0);
        assert_eq!(evaluate(".5 + 5.").unwrap(), 5.5);
    }

    #[test]
    fn reports_errors() {
        assert!(evaluate("1/0").is_err());
        assert!(evaluate("2+").is_err());
    }

    #[test]
    fn formats_cleanly() {
        assert_eq!(format_number(evaluate("0.1+0.2").unwrap()), "0.3");
        assert_eq!(format_number(3.5), "3.5");
        assert_eq!(format_number(-42.0), "-42");
        assert_eq!(format_number(1e20), "1e20");
    }
}
