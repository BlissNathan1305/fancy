use chrono::{Datelike, Duration, Local, Months, NaiveDate};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Terminal,
};
use std::{error::Error, io};
use tui_snippets::term;

struct App {
    today: NaiveDate,
    selected: NaiveDate,
}

impl App {
    fn new(start: NaiveDate) -> App {
        App {
            today: Local::now().date_naive(),
            selected: start,
        }
    }

    fn move_days(&mut self, days: i64) {
        self.selected += Duration::days(days);
    }

    /// Moving by months keeps the day of month, clamped to the target month's length.
    fn move_months(&mut self, months: i32) {
        let moved = if months >= 0 {
            self.selected.checked_add_months(Months::new(months as u32))
        } else {
            self.selected
                .checked_sub_months(Months::new(months.unsigned_abs()))
        };
        if let Some(date) = moved {
            self.selected = date;
        }
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(30)
}

const MONTH_NAMES: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];

/// Accepts `YYYY-MM` or `YYYY-MM-DD` on the command line.
fn parse_start(arg: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(arg, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(&format!("{arg}-01"), "%Y-%m-%d"))
        .ok()
}

fn main() -> Result<(), Box<dyn Error>> {
    let start = match std::env::args().nth(1) {
        Some(arg) if arg == "-h" || arg == "--help" => {
            println!("Usage: cal [YYYY-MM | YYYY-MM-DD]");
            return Ok(());
        }
        Some(arg) => parse_start(&arg).ok_or(format!(
            "cal: cannot parse date '{arg}' (use YYYY-MM or YYYY-MM-DD)"
        ))?,
        None => Local::now().date_naive(),
    };

    let mut terminal = term::init()?;
    let mut app = App::new(start);
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
                KeyCode::Right | KeyCode::Char('l') => app.move_days(1),
                KeyCode::Left | KeyCode::Char('h') => app.move_days(-1),
                KeyCode::Char('J') => app.move_days(7),
                KeyCode::Char('K') => app.move_days(-7),
                KeyCode::Up | KeyCode::Char('k') | KeyCode::PageUp => app.move_months(-1),
                KeyCode::Down | KeyCode::Char('j') | KeyCode::PageDown => app.move_months(1),
                KeyCode::Char('[') => app.move_months(-12),
                KeyCode::Char(']') => app.move_months(12),
                KeyCode::Char('t') => app.selected = app.today,
                _ => {}
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.size());

    let year = app.selected.year();
    let month = app.selected.month();
    let month_name = MONTH_NAMES[month as usize - 1];

    let header = Paragraph::new(format!(
        "✨ NEON COSMOS CALENDAR — {} {} ✨",
        month_name, year
    ))
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

    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_layout[1]);

    // Calendar grid, Monday first.
    let first_of_month = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(app.selected);
    let leading_blanks = first_of_month.weekday().num_days_from_monday() as usize;
    let mut cells: Vec<Option<u32>> = vec![None; leading_blanks];
    cells.extend((1..=days_in_month(year, month)).map(Some));
    while !cells.len().is_multiple_of(7) {
        cells.push(None);
    }

    let table_rows: Vec<Row> = cells
        .chunks(7)
        .map(|week| {
            let row_cells: Vec<Cell> = week
                .iter()
                .enumerate()
                .map(|(col, day)| match day {
                    None => Cell::from(""),
                    Some(d) => {
                        let date = NaiveDate::from_ymd_opt(year, month, *d);
                        let weekend = col >= 5;
                        if *d == app.selected.day() {
                            Cell::from(format!(" [{:>2}] ", d)).style(
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::Rgb(255, 255, 0))
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else if date == Some(app.today) {
                            Cell::from(format!(" ({:>2}) ", d)).style(
                                Style::default()
                                    .fg(Color::Rgb(0, 255, 128))
                                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                            )
                        } else {
                            let fg = if weekend {
                                Color::Rgb(255, 120, 200)
                            } else {
                                Color::Rgb(200, 200, 255)
                            };
                            Cell::from(format!("  {:>2}  ", d)).style(Style::default().fg(fg))
                        }
                    }
                })
                .collect();
            Row::new(row_cells).height(2)
        })
        .collect();

    let calendar_table = Table::new(table_rows, [Constraint::Ratio(1, 7); 7])
        .header(
            Row::new(vec![
                " MON ", " TUE ", " WED ", " THU ", " FRI ", " SAT ", " SUN ",
            ])
            .style(
                Style::default()
                    .fg(Color::Rgb(0, 255, 128))
                    .add_modifier(Modifier::BOLD),
            )
            .height(1),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" 📆 {} {} ", month_name, year))
                .title_style(
                    Style::default()
                        .fg(Color::Rgb(255, 165, 0))
                        .add_modifier(Modifier::BOLD),
                )
                .border_style(Style::default().fg(Color::Rgb(138, 43, 226))),
        );
    f.render_widget(calendar_table, content_layout[0]);

    // Facts about the selected date.
    let sel = app.selected;
    let offset = (sel - app.today).num_days();
    let relative = match offset {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        -1 => "Yesterday".to_string(),
        n if n > 0 => format!("In {} days", n),
        n => format!("{} days ago", -n),
    };
    let year_len = if NaiveDate::from_ymd_opt(sel.year(), 2, 29).is_some() {
        366
    } else {
        365
    };
    let label = Style::default().fg(Color::Rgb(0, 191, 255));
    let value = Style::default()
        .fg(Color::Rgb(240, 240, 240))
        .add_modifier(Modifier::BOLD);
    let info = vec![
        Line::from(Span::styled(
            sel.format("%A, %-d %B %Y").to_string(),
            Style::default()
                .fg(Color::Rgb(255, 255, 0))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Relative:     ", label),
            Span::styled(relative, value),
        ]),
        Line::from(vec![
            Span::styled("ISO date:     ", label),
            Span::styled(sel.format("%Y-%m-%d").to_string(), value),
        ]),
        Line::from(vec![
            Span::styled("ISO week:     ", label),
            Span::styled(format!("W{:02}", sel.iso_week().week()), value),
        ]),
        Line::from(vec![
            Span::styled("Day of year:  ", label),
            Span::styled(format!("{} / {}", sel.ordinal(), year_len), value),
        ]),
        Line::from(vec![
            Span::styled("Days left:    ", label),
            Span::styled(
                format!("{} in {}", year_len - sel.ordinal(), sel.year()),
                value,
            ),
        ]),
    ];

    let info_pane = Paragraph::new(info).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" 🚀 Selected Date ")
            .title_style(
                Style::default()
                    .fg(Color::Rgb(255, 0, 255))
                    .add_modifier(Modifier::BOLD),
            )
            .border_style(Style::default().fg(Color::Rgb(0, 191, 255))),
    );
    f.render_widget(info_pane, content_layout[1]);

    let footer_text = "h/l: Day | J/K: Week | j/k: Month | [ ]: Year | t: Today | q: Quit";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Rgb(160, 160, 160)))
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
    fn month_lengths() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn month_moves_clamp_day() {
        let mut app = App::new(NaiveDate::from_ymd_opt(2026, 1, 31).unwrap());
        app.move_months(1);
        assert_eq!(app.selected, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
        app.move_months(-12);
        assert_eq!(app.selected, NaiveDate::from_ymd_opt(2025, 2, 28).unwrap());
    }

    #[test]
    fn day_moves_cross_months() {
        let mut app = App::new(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
        app.move_days(1);
        assert_eq!(app.selected, NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
    }

    #[test]
    fn parses_cli_dates() {
        assert_eq!(parse_start("2026-03"), NaiveDate::from_ymd_opt(2026, 3, 1));
        assert_eq!(
            parse_start("2026-03-15"),
            NaiveDate::from_ymd_opt(2026, 3, 15)
        );
        assert_eq!(parse_start("nope"), None);
    }
}
