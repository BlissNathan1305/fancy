use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::io::{self, Write};
use std::time::{Duration, Instant};
use tui_snippets::{notify, term};

const CYCLE_LEN: u32 = 4;
const TICK_MS: u64 = 250;

#[derive(Parser, Clone, Copy)]
#[command(name = "pomodoro", version, about = "Pomodoro timer for the terminal")]
struct Config {
    /// Work session length in minutes
    #[arg(short, long, default_value_t = 25, value_parser = clap::value_parser!(u64).range(1..=600))]
    work: u64,
    /// Short break length in minutes
    #[arg(short, long, default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..=600))]
    short: u64,
    /// Long break length in minutes (after every 4 work sessions)
    #[arg(short, long, default_value_t = 15, value_parser = clap::value_parser!(u64).range(1..=600))]
    long: u64,
    /// Start paused instead of running
    #[arg(long)]
    paused: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Work,
    ShortBreak,
    LongBreak,
}

impl Mode {
    fn duration(self, cfg: &Config) -> Duration {
        let minutes = match self {
            Mode::Work => cfg.work,
            Mode::ShortBreak => cfg.short,
            Mode::LongBreak => cfg.long,
        };
        Duration::from_secs(minutes * 60)
    }

    fn label(self) -> &'static str {
        match self {
            Mode::Work => "WORK",
            Mode::ShortBreak => "SHORT BREAK",
            Mode::LongBreak => "LONG BREAK",
        }
    }

    fn color(self) -> Color {
        match self {
            Mode::Work => Color::Red,
            Mode::ShortBreak => Color::Green,
            Mode::LongBreak => Color::Blue,
        }
    }

    fn next(self, completed: u32) -> Mode {
        match self {
            // `completed` already includes the session that just ended.
            Mode::Work if completed.is_multiple_of(CYCLE_LEN) => Mode::LongBreak,
            Mode::Work => Mode::ShortBreak,
            _ => Mode::Work,
        }
    }
}

/// The timer counts down to a fixed deadline, so it never drifts no matter how
/// often (or late) the UI loop wakes up.
enum Timer {
    Running { ends_at: Instant },
    Paused { remaining: Duration },
}

struct App {
    cfg: Config,
    mode: Mode,
    timer: Timer,
    completed: u32,
}

impl App {
    fn new(cfg: Config) -> Self {
        let mut app = Self {
            cfg,
            mode: Mode::Work,
            timer: Timer::Paused {
                remaining: Mode::Work.duration(&cfg),
            },
            completed: 0,
        };
        if !cfg.paused {
            app.toggle_pause();
        }
        app
    }

    fn remaining(&self) -> Duration {
        match self.timer {
            Timer::Running { ends_at } => ends_at.saturating_duration_since(Instant::now()),
            Timer::Paused { remaining } => remaining,
        }
    }

    fn is_paused(&self) -> bool {
        matches!(self.timer, Timer::Paused { .. })
    }

    fn tick(&mut self) {
        if matches!(self.timer, Timer::Running { .. }) && self.remaining().is_zero() {
            self.advance(true);
        }
    }

    fn advance(&mut self, finished: bool) {
        if finished {
            let _ = io::stdout().write_all(b"\x07");
            let _ = io::stdout().flush();
        }
        if self.mode == Mode::Work {
            self.completed += 1;
        }
        self.mode = self.mode.next(self.completed);
        let duration = self.mode.duration(&self.cfg);
        self.timer = Timer::Running {
            ends_at: Instant::now() + duration,
        };
        if finished {
            let body = match self.mode {
                Mode::Work => "Break over, back to work".to_string(),
                Mode::ShortBreak => format!(
                    "Session {} done, take {} minutes",
                    self.completed, self.cfg.short
                ),
                Mode::LongBreak => format!(
                    "{} sessions done, take {} minutes",
                    self.completed, self.cfg.long
                ),
            };
            notify("Pomodoro", &body);
        }
    }

    fn toggle_pause(&mut self) {
        self.timer = match self.timer {
            Timer::Running { .. } => Timer::Paused {
                remaining: self.remaining(),
            },
            Timer::Paused { remaining } => Timer::Running {
                ends_at: Instant::now() + remaining,
            },
        };
    }

    fn skip(&mut self) {
        self.advance(false);
    }

    fn reset(&mut self) {
        *self = Self::new(self.cfg);
    }

    fn progress(&self) -> f64 {
        let total = self.mode.duration(&self.cfg).as_secs_f64();
        (1.0 - self.remaining().as_secs_f64() / total).clamp(0.0, 1.0)
    }

    fn fmt_time(&self) -> String {
        // Round up so the display shows 25:00 at the start and 00:01 in the last second.
        let secs = self.remaining().as_secs_f64().ceil() as u64;
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }

    fn dots_filled(&self) -> u32 {
        match self.mode {
            Mode::LongBreak => CYCLE_LEN,
            _ => self.completed % CYCLE_LEN,
        }
    }
}

fn render(f: &mut ratatui::Frame, app: &App) {
    let color = app.mode.color();
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(f.size());

    let lines = vec![
        Line::raw(""),
        Line::raw(""),
        Line::from(Span::styled(
            "P O M O D O R O",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                if app.is_paused() { "|| " } else { ">  " },
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                app.mode.label(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(Span::styled(
            app.fmt_time(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from({
            let w = 28_usize;
            let n = (app.progress() * w as f64).round() as usize;
            Span::styled(
                format!("{}{}", "█".repeat(n), "░".repeat(w - n)),
                Style::default().fg(color),
            )
        }),
        Line::raw(""),
        Line::raw(""),
        Line::from(
            (0..CYCLE_LEN)
                .map(|i| {
                    if i < app.dots_filled() {
                        Span::styled("● ", Style::default().fg(color))
                    } else {
                        Span::styled("○ ", Style::default().fg(Color::DarkGray))
                    }
                })
                .collect::<Vec<_>>(),
        ),
        Line::raw(""),
        Line::from(Span::styled(
            format!("Completed: {}", app.completed),
            Style::default().fg(Color::DarkGray),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            if app.is_paused() { "PAUSED" } else { "RUNNING" },
            Style::default().fg(if app.is_paused() {
                Color::Yellow
            } else {
                Color::Green
            }),
        )),
    ];

    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    format!(" {} ", app.mode.label()),
                    Style::default().fg(color),
                )),
        ),
        chunks[0],
    );

    let help = Line::from(vec![
        Span::styled(
            " [Space]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Pause "),
        Span::styled(
            "[S]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Skip "),
        Span::styled(
            "[R]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Reset "),
        Span::styled(
            "[Q]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Quit"),
    ]);
    f.render_widget(Paragraph::new(help).alignment(Alignment::Center), chunks[1]);
}

fn main() -> io::Result<()> {
    let cfg = Config::parse();
    let mut terminal = term::init()?;
    let result = run(&mut terminal, App::new(cfg));
    term::restore()?;
    result
}

fn run(terminal: &mut term::Tui, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| render(f, &app))?;

        if event::poll(Duration::from_millis(TICK_MS))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char(' ') => app.toggle_pause(),
                    KeyCode::Char('s') => app.skip(),
                    KeyCode::Char('r') => app.reset(),
                    _ => {}
                }
            }
        }

        app.tick();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            work: 25,
            short: 5,
            long: 15,
            paused: true,
        }
    }

    #[test]
    fn long_break_after_four_sessions() {
        let mut app = App::new(cfg());
        let mut modes = Vec::new();
        for _ in 0..8 {
            app.skip();
            modes.push(app.mode);
        }
        assert!(
            modes[..6]
                == [
                    Mode::ShortBreak,
                    Mode::Work,
                    Mode::ShortBreak,
                    Mode::Work,
                    Mode::ShortBreak,
                    Mode::Work
                ]
        );
        assert!(modes[6] == Mode::LongBreak);
    }

    #[test]
    fn pause_freezes_remaining_time() {
        let app = App::new(cfg());
        assert!(app.is_paused());
        assert_eq!(app.fmt_time(), "25:00");
        assert_eq!(app.progress(), 0.0);
    }

    #[test]
    fn running_timer_counts_from_deadline() {
        let mut app = App::new(Config {
            paused: false,
            ..cfg()
        });
        app.timer = Timer::Running {
            ends_at: Instant::now() + Duration::from_millis(1500),
        };
        assert_eq!(app.fmt_time(), "00:02");
        app.toggle_pause();
        assert!(app.remaining() <= Duration::from_millis(1500));
    }
}
