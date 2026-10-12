use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph},
};
use std::{
    io::{self, Write},
    time::{Duration, Instant},
};
use tui_snippets::{notify, term};

#[derive(Parser)]
#[command(name = "timer", version, about = "Countdown timer for the terminal")]
struct Cli {
    /// Duration such as 90s, 25m, 1h30m or 10 (minutes)
    #[arg(default_value = "5m", value_parser = parse_duration)]
    duration: Duration,

    /// Label shown on screen and in the notification
    #[arg(short, long, default_value = "Timer")]
    label: String,
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return Err("empty duration".into());
    }
    let mut total = 0u64;
    let mut number = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let n: u64 = number
            .parse()
            .map_err(|_| format!("invalid duration '{s}'"))?;
        number.clear();
        total += match c {
            'h' => n * 3600,
            'm' => n * 60,
            's' => n,
            _ => return Err(format!("unknown unit '{c}' in '{s}' (use h, m, s)")),
        };
    }
    if !number.is_empty() {
        // A bare number means minutes.
        total += number
            .parse::<u64>()
            .map_err(|_| format!("invalid duration '{s}'"))?
            * 60;
    }
    if total == 0 {
        return Err("duration must be greater than zero".into());
    }
    Ok(Duration::from_secs(total))
}

enum State {
    Running { ends_at: Instant },
    Paused { remaining: Duration },
    Finished { at: Instant },
}

struct App {
    label: String,
    total: Duration,
    state: State,
}

impl App {
    fn new(label: String, total: Duration) -> Self {
        Self {
            label,
            total,
            state: State::Running {
                ends_at: Instant::now() + total,
            },
        }
    }

    fn remaining(&self) -> Duration {
        match self.state {
            State::Running { ends_at } => ends_at.saturating_duration_since(Instant::now()),
            State::Paused { remaining } => remaining,
            State::Finished { .. } => Duration::ZERO,
        }
    }

    fn tick(&mut self) {
        if matches!(self.state, State::Running { .. }) && self.remaining().is_zero() {
            self.state = State::Finished { at: Instant::now() };
            let _ = io::stdout().write_all(b"\x07");
            let _ = io::stdout().flush();
            if !cfg!(test) {
                notify("⏱ Time's up", &self.label);
            }
        }
    }

    fn toggle_pause(&mut self) {
        self.state = match self.state {
            State::Running { .. } => State::Paused {
                remaining: self.remaining(),
            },
            State::Paused { remaining } => State::Running {
                ends_at: Instant::now() + remaining,
            },
            State::Finished { at } => State::Finished { at },
        };
    }

    /// Add (or with a negative value remove) time; works while running, paused or finished.
    fn adjust(&mut self, secs: i64) {
        let delta = Duration::from_secs(secs.unsigned_abs());
        let remaining = if secs >= 0 {
            self.remaining() + delta
        } else {
            self.remaining().saturating_sub(delta)
        };
        let remaining = remaining.max(Duration::from_secs(1));
        if secs > 0 {
            self.total += Duration::from_secs(secs as u64);
        }
        self.total = self.total.max(remaining);
        self.state = match self.state {
            State::Paused { .. } => State::Paused { remaining },
            _ => State::Running {
                ends_at: Instant::now() + remaining,
            },
        };
    }

    fn restart(&mut self) {
        self.state = State::Running {
            ends_at: Instant::now() + self.total,
        };
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut terminal = term::init()?;
    let result = run(&mut terminal, App::new(cli.label, cli.duration));
    term::restore()?;
    result?;
    Ok(())
}

fn run(terminal: &mut term::Tui, mut app: App) -> io::Result<()> {
    loop {
        app.tick();
        terminal.draw(|f| draw(f, &app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char(' ') | KeyCode::Char('p') => app.toggle_pause(),
                    KeyCode::Char('+') | KeyCode::Char('=') => app.adjust(60),
                    KeyCode::Char('-') => app.adjust(-60),
                    KeyCode::Char('r') => app.restart(),
                    _ => {}
                }
            }
        }
    }
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    let outer_block = Block::default()
        .title(format!(" ⏱️  {} ", app.label))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Indexed(242)));
    let inner_area = outer_block.inner(f.size());
    f.render_widget(outer_block, f.size());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner_area);

    let remaining = app.remaining();
    // Round up so a 5m timer starts at 05:00 and shows 00:01 during its last second.
    let secs = remaining.as_secs_f64().ceil() as u64;
    let ratio = (remaining.as_secs_f64() / app.total.as_secs_f64()).clamp(0.0, 1.0);

    let (theme_color, status, status_color) = match app.state {
        State::Finished { at } => {
            let flash = at.elapsed().as_millis() / 500 % 2 == 0;
            (
                if flash { Color::Red } else { Color::Yellow },
                "TIME'S UP!",
                Color::Red,
            )
        }
        State::Paused { .. } => (Color::Yellow, "Paused", Color::Yellow),
        State::Running { .. } if ratio > 0.5 => (Color::Cyan, "Running", Color::Green),
        State::Running { .. } if ratio > 0.2 => (Color::Yellow, "Running", Color::Green),
        State::Running { .. } => (Color::Red, "Running", Color::Green),
    };

    let clock = if secs >= 3600 {
        format!(" {}:{:02}:{:02} ", secs / 3600, secs % 3600 / 60, secs % 60)
    } else {
        format!(" {:02}:{:02} ", secs / 60, secs % 60)
    };
    let clock = if matches!(app.state, State::Finished { .. }) {
        " TIME'S UP ".to_string()
    } else {
        clock
    };
    f.render_widget(
        Paragraph::new(clock).alignment(Alignment::Center).style(
            Style::default()
                .fg(theme_color)
                .add_modifier(Modifier::BOLD),
        ),
        chunks[1],
    );

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(theme_color).bg(Color::Indexed(236)))
        .ratio(ratio)
        .label("");
    f.render_widget(gauge, chunks[3]);

    let instructions = Line::from(vec![
        Span::styled(" [Space] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("Pause ", Style::default().fg(Color::Gray)),
        Span::styled(" [+/-] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("±1 min ", Style::default().fg(Color::Gray)),
        Span::styled(" [R] ", Style::default().fg(Color::Cyan).bold()),
        Span::styled("Restart ", Style::default().fg(Color::Gray)),
        Span::styled(" [Q] ", Style::default().fg(Color::Red).bold()),
        Span::styled("Quit ", Style::default().fg(Color::Gray)),
        Span::styled(" │ ", Style::default().fg(Color::Indexed(240))),
        Span::styled(status, Style::default().fg(status_color).italic()),
    ]);
    f.render_widget(
        Paragraph::new(instructions).alignment(Alignment::Center),
        chunks[5],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_duration("25m").unwrap(), Duration::from_secs(1500));
        assert_eq!(parse_duration("1h30m").unwrap(), Duration::from_secs(5400));
        assert_eq!(parse_duration("10").unwrap(), Duration::from_secs(600));
        assert!(parse_duration("0").is_err());
        assert!(parse_duration("5x").is_err());
    }

    #[test]
    fn pause_and_adjust() {
        let mut app = App::new("t".into(), Duration::from_secs(120));
        app.toggle_pause();
        let before = app.remaining();
        app.adjust(60);
        assert!(matches!(app.state, State::Paused { .. }));
        assert_eq!(app.remaining(), before + Duration::from_secs(60));
        app.adjust(-600);
        assert_eq!(app.remaining(), Duration::from_secs(1));
    }

    #[test]
    fn finishes_at_zero() {
        let mut app = App::new("t".into(), Duration::from_secs(60));
        app.state = State::Running {
            ends_at: Instant::now(),
        };
        app.tick();
        assert!(matches!(app.state, State::Finished { .. }));
    }
}
