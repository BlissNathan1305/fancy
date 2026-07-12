use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, Paragraph},
    Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Terminal Init
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 2. State Setup (5 Minute Countdown)
    let total_duration = Duration::from_secs(5 * 60);
    let start_time = Instant::now();
    let tick_rate = Duration::from_millis(200); // Slightly faster tick for snappier UI
    let mut last_tick = Instant::now();

    loop {
        let elapsed = start_time.elapsed();
        let remaining = if elapsed >= total_duration {
            Duration::ZERO
        } else {
            total_duration - elapsed
        };

        // 3. UI Render Layer
        terminal.draw(|f| {
            // Main layout wrapper with an outer container box
            let outer_block = Block::default()
                .title(" ⏱️  RUST CLI TIMER ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Indexed(242))); // Clean dark-gray border

            let inner_area = outer_block.inner(f.size());
            f.render_widget(outer_block, f.size());

            // Split the internal area
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(30), // Top spacing
                    Constraint::Length(3),      // Large Digital Clock
                    Constraint::Length(2),      // Spacing
                    Constraint::Length(2),      // Progress Bar Layout
                    Constraint::Min(0),         // Spacer
                    Constraint::Length(1),      // Instruction Bar
                ])
                .split(inner_area);

            // Compute time remaining parameters
            let secs = remaining.as_secs();
            let minutes = secs / 60;
            let seconds = secs % 60;
            let percent = if total_duration.as_secs() > 0 {
                (remaining.as_secs_f64() / total_duration.as_secs_f64()).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Dynamic Styling: Colors shift depending on time left
            let theme_color = if percent > 0.5 {
                Color::Cyan          // Plenty of time
            } else if percent > 0.2 {
                Color::Yellow        // Getting closer
            } else {
                Color::Red           // Running out!
            };

            // Render Digital Clock Display
            let clock_str = format!(" {:02}:{:02} ", minutes, seconds);
            let clock = Paragraph::new(clock_str)
                .alignment(Alignment::Center)
                .style(Style::default().fg(theme_color).add_modifier(Modifier::BOLD));
            f.render_widget(clock, chunks[1]);

            // Render High-Contrast Progress Bar
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::NONE))
                .gauge_style(Style::default().fg(theme_color).bg(Color::Indexed(236)))
                .ratio(percent)
                .label(""); // Blank track creates a minimal sleek ribbon accent
            f.render_widget(gauge, chunks[3]);

            // Render Help/Instructions Footer Bar
            let instructions = Line::from(vec![
                Span::styled(" [Q/Esc] ", Style::default().fg(Color::Red).bold()),
                Span::styled("Quit Timer ", Style::default().fg(Color::Gray)),
                Span::styled(" │ ", Style::default().fg(Color::Indexed(240))),
                Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled("Running...", Style::default().fg(Color::Green).italic()),
            ]);
            let help_paragraph = Paragraph::new(instructions).alignment(Alignment::Center);
            f.render_widget(help_paragraph, chunks[5]);
        })?;

        // 4. Input handling & intervals
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        if remaining == Duration::ZERO {
            break;
        }
    }

    // 5. Cleanup Terminal State cleanly
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
