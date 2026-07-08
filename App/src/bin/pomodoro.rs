use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io::{self, Write};
use std::time::{Duration, Instant};

const WORK_SECS: u64 = 25 * 60;
const SHORT_BREAK_SECS: u64 = 5 * 60;
const LONG_BREAK_SECS: u64 = 15 * 60;
const CYCLE_LEN: u32 = 4;
const TICK_MS: u64 = 250;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Work,
    ShortBreak,
    LongBreak,
}

impl Mode {
    fn duration(self) -> u64 {
        match self {
            Mode::Work => WORK_SECS,
            Mode::ShortBreak => SHORT_BREAK_SECS,
            Mode::LongBreak => LONG_BREAK_SECS,
        }
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
            Mode::Work if (completed + 1) % CYCLE_LEN == 0 => Mode::LongBreak,
            Mode::Work => Mode::ShortBreak,
            _ => Mode::Work,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TimerState {
    Running,
    Paused,
}

struct App {
    mode: Mode,
    state: TimerState,
    remaining: u64,
    completed: u32,
    last_tick: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            mode: Mode::Work,
            state: TimerState::Running,
            remaining: WORK_SECS,
            completed: 0,
            last_tick: Instant::now(),
        }
    }

    fn tick(&mut self) {
        if self.state != TimerState::Running {
            return;
        }
        let elapsed = self.last_tick.elapsed().as_secs();
        if elapsed < 1 {
            return;
        }
        self.last_tick = Instant::now();
        self.remaining = self.remaining.saturating_sub(elapsed);
        if self.remaining == 0 {
            self.advance();
        }
    }

    fn advance(&mut self) {
        let _ = io::stderr().write_all(b"\x07");
        let _ = io::stderr().flush();

        if self.mode == Mode::Work {
            self.completed += 1;
        }
        self.mode = self.mode.next(self.completed);
        self.remaining = self.mode.duration();
        self.last_tick = Instant::now();
    }

    fn toggle_pause(&mut self) {
        self.state = match self.state {
            TimerState::Running => TimerState::Paused,
            TimerState::Paused => {
                self.last_tick = Instant::now();
                TimerState::Running
            }
        };
    }

    fn skip(&mut self) {
        self.advance();
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn progress(&self) -> f64 {
        1.0 - self.remaining as f64 / self.mode.duration() as f64
    }

    fn fmt_time(&self) -> String {
        let m = self.remaining / 60;
        let s = self.remaining % 60;
        format!("{:02}:{:02}", m, s)
    }

    fn dots_filled(&self) -> u32 {
        match self.mode {
            Mode::LongBreak => CYCLE_LEN,
            _ => self.completed % CYCLE_LEN,
        }
    }
}

struct RestoreTerminal;

impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
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
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                match app.state {
                    TimerState::Paused => "|| ",
                    TimerState::Running => ">  ",
                },
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
            match app.state {
                TimerState::Paused => "PAUSED",
                TimerState::Running => "RUNNING",
            },
            Style::default().fg(match app.state {
                TimerState::Paused => Color::Yellow,
                TimerState::Running => Color::Green,
            }),
        )),
    ];

    f.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(
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
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Pause "),
        Span::styled(
            "[S]",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Skip "),
        Span::styled(
            "[R]",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Reset "),
        Span::styled(
            "[Q]",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Quit"),
    ]);
    f.render_widget(
        Paragraph::new(help).alignment(Alignment::Center),
        chunks[1],
    );
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let _restore = RestoreTerminal;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();

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
