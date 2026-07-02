use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{CrosstermBackend, Backend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap, Cell},
    Terminal,
};
use chrono::{Datelike, Local, NaiveDate};
use std::{error::Error, io};

struct App {
    current_date: NaiveDate,
    selected_day: u32,
}

impl App {
    fn new() -> App {
        let local_now = Local::now().date_naive();
        App {
            current_date: local_now,
            selected_day: local_now.day(),
        }
    }

    fn next_month(&mut self) {
        let mut year = self.current_date.year();
        let mut month = self.current_date.month() + 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
        self.current_date = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(self.current_date);
        self.selected_day = 1;
    }

    fn prev_month(&mut self) {
        let mut year = self.current_date.year();
        let mut month = self.current_date.month() as i32 - 1;
        if month < 1 {
            month = 12;
            year -= 1;
        }
        self.current_date = NaiveDate::from_ymd_opt(year, month as u32, 1).unwrap_or(self.current_date);
        self.selected_day = 1;
    }

    fn move_day_left(&mut self) {
        if self.selected_day > 1 {
            self.selected_day -= 1;
        }
    }

    fn move_day_right(&mut self) {
        let days_in_month = self.days_in_current_month();
        if self.selected_day < days_in_month {
            self.selected_day += 1;
        }
    }

    fn days_in_current_month(&self) -> u32 {
        let y = self.current_date.year();
        let m = self.current_date.month();
        if m == 12 {
            NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap().signed_duration_since(NaiveDate::from_ymd_opt(y, m, 1).unwrap()).num_days() as u32
        } else {
            NaiveDate::from_ymd_opt(y, m + 1, 1).unwrap().signed_duration_since(NaiveDate::from_ymd_opt(y, m, 1).unwrap()).num_days() as u32
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
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Right | KeyCode::Char('l') => app.move_day_right(),
                KeyCode::Left | KeyCode::Char('h') => app.move_day_left(),
                KeyCode::Up | KeyCode::Char('k') => app.prev_month(),
                KeyCode::Down | KeyCode::Char('j') => app.next_month(),
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
            Constraint::Length(3), 
            Constraint::Min(1),    
            Constraint::Length(3), 
        ].as_ref())
        .split(f.size());

    // 1. Header Display
    let header_text = format!("✨ NEON COSMOS CALENDAR — {} ✨", app.current_date.year());
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Rgb(0, 255, 255)).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Rgb(255, 0, 128))));
    f.render_widget(header, main_layout[0]);

    // Split Layout
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), 
            Constraint::Percentage(40), 
        ].as_ref())
        .split(main_layout[1]);

    // 2. Generate Calendar Matrix
    let month_name = match app.current_date.month() {
        1 => "JANUARY", 2 => "FEBRUARY", 3 => "MARCH", 4 => "APRIL",
        5 => "MAY", 6 => "JUNE", 7 => "JULY", 8 => "AUGUST",
        9 => "SEPTEMBER", 10 => "OCTOBER", 11 => "NOVEMBER", _ => "DECEMBER",
    };

    let first_of_month = NaiveDate::from_ymd_opt(app.current_date.year(), app.current_date.month(), 1).unwrap();
    let starting_spaces = first_of_month.weekday().num_days_from_monday();
    let days_in_month = app.days_in_current_month();

    let mut rows = Vec::new();
    let mut current_row = vec![String::new(); starting_spaces as usize];

    for day in 1..=days_in_month {
        current_row.push(day.to_string());
        if current_row.len() == 7 {
            rows.push(std::mem::take(&mut current_row));
        }
    }
    if !current_row.is_empty() {
        while current_row.len() < 7 {
            current_row.push(String::new());
        }
        rows.push(current_row);
    }

    // FIX: Map individual list items cleanly into styled dynamic row Cells
    let table_rows: Vec<Row> = rows.iter().map(|week| {
        let cells: Vec<Cell> = week.iter().map(|day_str| {
            if day_str.is_empty() {
                Cell::from("")
            } else if day_str == &app.selected_day.to_string() {
                Cell::from(format!(" [{}] ", day_str))
                    .style(Style::default().fg(Color::Black).bg(Color::Rgb(255, 255, 0)).add_modifier(Modifier::BOLD))
            } else {
                Cell::from(format!("  {}  ", day_str))
                    .style(Style::default().fg(Color::Rgb(200, 200, 255)))
            }
        }).collect();
        
        Row::new(cells).height(2)
    }).collect();

    let calendar_table = Table::new(
        table_rows,
        [
            Constraint::Percentage(14), Constraint::Percentage(14), Constraint::Percentage(14),
            Constraint::Percentage(14), Constraint::Percentage(14), Constraint::Percentage(14),
            Constraint::Percentage(14)
        ]
    )
    .header(
        Row::new(vec![" MON ", " TUE ", " WED ", " THU ", " FRI ", " SAT ", " SUN "])
            .style(Style::default().fg(Color::Rgb(0, 255, 128)).add_modifier(Modifier::BOLD))
            .height(1)
    )
    .block(Block::default()
        .borders(Borders::ALL)
        .title(format!(" 📆 {} ", month_name))
        .title_style(Style::default().fg(Color::Rgb(255, 165, 0)).add_modifier(Modifier::BOLD))
        .border_style(Style::default().fg(Color::Rgb(138, 43, 226))));

    f.render_widget(calendar_table, content_layout[0]);

    // 3. Right Sidebar: Agenda Content
    let agenda_title = format!(" 🚀 Schedule for {} {} ", month_name, app.selected_day);
    let agenda_text = format!(
        "• 09:00 AM -- Sync with Cyberpunk Grid\n\
         • 01:00 PM -- Code more high-performance Rust utilities\n\
         • 04:00 PM -- Refactor memory buffers\n\n\
         [Selected Timestamp]\n{}-{:02}-{:02}",
         app.current_date.year(), app.current_date.month(), app.selected_day
    );

    let agenda_pane = Paragraph::new(agenda_text)
        .style(Style::default().fg(Color::Rgb(240, 240, 240)))
        .wrap(Wrap { trim: false })
        .block(Block::default()
            .borders(Borders::ALL)
            .title(agenda_title)
            .title_style(Style::default().fg(Color::Rgb(255, 0, 255)).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(Color::Rgb(0, 191, 255))));
    f.render_widget(agenda_pane, content_layout[1]);

    // 4. Navigation Menu
    let footer_text = "h/l (Left/Right): Navigate Days | j/k (Down/Up): Cycle Months | q: Quit";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Rgb(160, 160, 160)))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(footer, main_layout[2]);
}
