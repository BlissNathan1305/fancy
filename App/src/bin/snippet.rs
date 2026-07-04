use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::execute;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

const SNIPPETS_FILE: &str = "snippets.json";

// --- Data Model ---

#[derive(Serialize, Deserialize, Clone)]
struct Snippet {
    title: String,
    language: String,
    code: String,
}

// --- App State ---

enum AppMode {
    Normal,
    Search,
}

struct App {
    snippets: Vec<Snippet>,
    filtered_indices: Vec<usize>,
    state: ListState,
    mode: AppMode,
    search_input: String,
    status_message: String,
}

impl App {
    // Now returns a Result because it interacts with the file system
    fn new() -> Result<Self> {
        let mut app = Self {
            snippets: Vec::new(),
            filtered_indices: Vec::new(),
            state: ListState::default(),
            mode: AppMode::Normal,
            search_input: String::new(),
            status_message: "Press 'r' to reload snippets from file".to_string(),
        };
        
        app.reload_from_file()?;
        Ok(app)
    }

    fn reload_from_file(&mut self) -> Result<()> {
        let path = PathBuf::from(SNIPPETS_FILE);

        if path.exists() {
            // Load from file
            let data = fs::read_to_string(&path)?;
            self.snippets = serde_json::from_str(&data)?;
            self.status_message = format!("Loaded {} snippets from file.", self.snippets.len());
        } else {
            // Create file with dummy data if it doesn't exist
            self.snippets = vec![
                Snippet {
                    title: "Read File".to_string(),
                    language: "Rust".to_string(),
                    code: "use std::fs;\n\nlet contents = fs::read_to_string(\"file.txt\")\n    .expect(\"Should have been able to read the file\");".to_string(),
                },
                Snippet {
                    title: "HTTP GET".to_string(),
                    language: "Python".to_string(),
                    code: "import requests\n\nresponse = requests.get('https://api.example.com/data')\nprint(response.json())".to_string(),
                },
            ];
            let json = serde_json::to_string_pretty(&self.snippets)?;
            fs::write(&path, json)?;
            self.status_message = format!("Created {}. Add snippets here!", SNIPPETS_FILE);
        }

        self.update_filter();
        Ok(())
    }

    fn update_filter(&mut self) {
        if self.search_input.is_empty() {
            self.filtered_indices = (0..self.snippets.len()).collect();
        } else {
            let query = self.search_input.to_lowercase();
            self.filtered_indices = self.snippets
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.title.to_lowercase().contains(&query) || s.language.to_lowercase().contains(&query)
                })
                .map(|(i, _)| i)
                .collect();
        }

        if self.filtered_indices.is_empty() {
            self.state.select(None);
        } else {
            self.state.select(Some(0));
        }
    }

    fn get_selected_snippet(&self) -> Option<&Snippet> {
        self.state.selected().and_then(|i| {
            self.filtered_indices
                .get(i)
                .and_then(|&real_idx| self.snippets.get(real_idx))
        })
    }
}

// --- UI Rendering ---

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(10),
    ])
    .split(f.size());

    let top_block = match app.mode {
        AppMode::Normal => Block::default().borders(Borders::ALL).title(" Snippet Manager "),
        AppMode::Search => Block::default()
            .borders(Borders::ALL)
            .title(" Search (/ to cancel) ")
            .border_style(Style::default().fg(Color::Yellow)),
    };

    let top_text = match app.mode {
        AppMode::Normal => Line::from(" ↑/↓: Navigate | /: Search | Enter: Copy | r: Reload | q: Quit "),
        AppMode::Search => Line::from(Span::styled(
            format!(" {} ", app.search_input),
            Style::default().fg(Color::Black).bg(Color::White),
        )),
    };
    let top_paragraph = Paragraph::new(top_text).block(top_block);
    f.render_widget(top_paragraph, chunks[0]);

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .map(|&i| {
            let snip = &app.snippets[i];
            let line = Line::from(vec![
                Span::styled(format!(" {} ", snip.title), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({})", snip.language), Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Snippets "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, chunks[1], &mut app.state.clone());

    let preview_code = app
        .get_selected_snippet()
        .map(|s| s.code.clone())
        .unwrap_or_else(|| "No snippet selected.".to_string());

    let preview = Paragraph::new(preview_code)
        .block(Block::default().borders(Borders::ALL).title(" Preview "))
        .style(Style::default().fg(Color::Cyan))
        .wrap(Wrap { trim: false });
    f.render_widget(preview, chunks[2]);

    if !app.status_message.is_empty() {
        let status = Paragraph::new(app.status_message.clone())
            .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
        let bottom_rect = Rect {
            x: 0,
            y: f.size().height.saturating_sub(1),
            width: f.size().width,
            height: 1,
        };
        f.render_widget(status, bottom_rect);
    }
}

// --- Main Event Loop ---

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?; // Notice the ?
    let mut clipboard = Clipboard::new()?;

    let result = run_app(&mut terminal, &mut app, &mut clipboard);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    app: &mut App,
    clipboard: &mut Clipboard,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match app.mode {
                AppMode::Normal => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('r') => {
                        // RELOAD FROM FILE
                        app.reload_from_file()?;
                    }
                    KeyCode::Char('/') => {
                        app.mode = AppMode::Search;
                        app.search_input.clear();
                        app.update_filter();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !app.filtered_indices.is_empty() {
                            let current = app.state.selected().unwrap_or(0);
                            let next = current.saturating_add(1).min(app.filtered_indices.len() - 1);
                            app.state.select(Some(next));
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let Some(current) = app.state.selected() {
                            let next = current.saturating_sub(1);
                            app.state.select(Some(next));
                        } else if !app.filtered_indices.is_empty() {
                            app.state.select(Some(0));
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(snippet) = app.get_selected_snippet() {
                            match clipboard.set_text(&snippet.code) {
                                Ok(_) => app.status_message = format!("Copied '{}' to clipboard!", snippet.title),
                                Err(_) => app.status_message = "Failed to access clipboard.".to_string(),
                            }
                        }
                    }
                    _ => {}
                },
                AppMode::Search => match key.code {
                    KeyCode::Esc | KeyCode::Char('/') => {
                        app.mode = AppMode::Normal;
                        app.search_input.clear();
                        app.update_filter();
                    }
                    KeyCode::Enter => {
                        app.mode = AppMode::Normal;
                    }
                    KeyCode::Backspace => {
                        app.search_input.pop();
                        app.update_filter();
                    }
                    KeyCode::Char(c) => {
                        app.search_input.push(c);
                        app.update_filter();
                    }
                    _ => {}
                },
            }
        }
    }
}
