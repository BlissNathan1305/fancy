use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{CrosstermBackend, Backend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::{error::Error, io, fs, path::PathBuf};

#[derive(PartialEq)]
enum Mode {
    SelectNote,
    EditContent,
    CreateNew,
}

struct Note {
    title: String,
    content: String,
}

struct App {
    notes: Vec<Note>,
    list_state: ListState,
    mode: Mode,
    input_buffer: String,
    scroll_offset: u16, 
}

impl App {
    fn new() -> App {
        let mut state = ListState::default();
        state.select(Some(0));
        
        let mut app = App {
            notes: Vec::new(),
            list_state: state,
            mode: Mode::SelectNote,
            input_buffer: String::new(),
            scroll_offset: 0,
        };
        
        app.ensure_storage_dir();
        app.load_all_notes();
        
        if app.notes.is_empty() {
            app.notes.push(Note {
                title: "Welcome.md".to_string(),
                content: "Welcome to your Cyberpunk Notebook!\n\nPress 'n' to create a new note.\nPress 'e' to edit the current note.\nPress 'x' to delete a note.\nPress 'Esc' to stop editing.\nPress 'q' to safely quit.".to_string(),
            });
        }
        
        app
    }

    fn storage_dir(&self) -> PathBuf {
        let mut path = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        path.push(".rust_notes");
        path
    }

    fn ensure_storage_dir(&self) {
        let dir = self.storage_dir();
        if !dir.exists() {
            let _ = fs::create_dir_all(dir);
        }
    }

    fn load_all_notes(&mut self) {
        let dir = self.storage_dir();
        self.notes.clear();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
                    let title = path.file_name().unwrap().to_string_lossy().into_owned();
                    if let Ok(content) = fs::read_to_string(path) {
                        self.notes.push(Note { title, content });
                    }
                }
            }
        }
        self.notes.sort_by(|a, b| a.title.cmp(&b.title));
    }

    fn save_current_note(&self) {
        if let Some(i) = self.list_state.selected() {
            if i < self.notes.len() {
                let mut path = self.storage_dir();
                path.push(&self.notes[i].title);
                let _ = fs::write(path, &self.notes[i].content);
            }
        }
    }

    fn delete_current_note(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if !self.notes.is_empty() {
                let mut path = self.storage_dir();
                path.push(&self.notes[i].title);
                let _ = fs::remove_file(path);
                
                self.notes.remove(i);
                if self.notes.is_empty() {
                    self.list_state.select(None);
                } else if i >= self.notes.len() {
                    self.list_state.select(Some(self.notes.len() - 1));
                }
            }
        }
    }

    fn current_note_mut(&mut self) -> Option<&mut Note> {
        let i = self.list_state.selected()?;
        self.notes.get_mut(i)
    }

    fn next_note(&mut self) {
        if self.notes.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i >= self.notes.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
        self.scroll_offset = 0; 
    }

    fn prev_note(&mut self) {
        if self.notes.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { self.notes.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
        self.scroll_offset = 0; 
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
            match app.mode {
                Mode::SelectNote => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    // FIX: Added KeyCode::Down and KeyCode::Up support here
                    KeyCode::Down | KeyCode::Char('j') => app.next_note(),
                    KeyCode::Up | KeyCode::Char('k') => app.prev_note(),
                    
                    KeyCode::Char('u') => { if app.scroll_offset > 0 { app.scroll_offset -= 1; } }
                    KeyCode::Char('d') => { app.scroll_offset += 1; }
                    KeyCode::Char('e') => {
                        if let Some(note) = app.current_note_mut() {
                            app.input_buffer = note.content.clone();
                            app.mode = Mode::EditContent;
                        }
                    }
                    KeyCode::Char('n') => {
                        app.input_buffer.clear();
                        app.mode = Mode::CreateNew;
                    }
                    KeyCode::Char('x') => app.delete_current_note(), 
                    _ => {}
                },
                Mode::EditContent => match key.code {
                    KeyCode::Esc => {
                        let content_to_save = app.input_buffer.clone();
                        if let Some(note) = app.current_note_mut() {
                            note.content = content_to_save;
                            app.save_current_note();
                        }
                        app.mode = Mode::SelectNote;
                    }
                    KeyCode::Enter => app.input_buffer.push('\n'),
                    KeyCode::Backspace => { app.input_buffer.pop(); }
                    KeyCode::Char(c) => app.input_buffer.push(c),
                    _ => {}
                },
                Mode::CreateNew => match key.code {
                    KeyCode::Esc => app.mode = Mode::SelectNote,
                    KeyCode::Enter => {
                        if !app.input_buffer.is_empty() {
                            let mut filename = app.input_buffer.trim().to_string();
                            if !filename.ends_with(".md") {
                                filename.push_str(".md");
                            }
                            
                            let target_title = filename.clone();
                            
                            let new_note = Note {
                                title: filename,
                                content: String::new(),
                            };
                            app.notes.push(new_note);
                            app.notes.sort_by(|a, b| a.title.cmp(&b.title));
                            
                            let new_idx = app.notes.iter().position(|n| n.title == target_title).unwrap_or(0);
                            app.list_state.select(Some(new_idx));
                            app.save_current_note();
                            
                            app.input_buffer.clear();
                            app.mode = Mode::EditContent;
                        }
                    }
                    KeyCode::Backspace => { app.input_buffer.pop(); }
                    KeyCode::Char(c) => app.input_buffer.push(c),
                    _ => {}
                }
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

    let header = Paragraph::new("🧬 CYBERNETIC NEON NOTEBOOK 🧬")
        .style(Style::default().fg(Color::Rgb(255, 0, 255)).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Rgb(0, 255, 255))));
    f.render_widget(header, main_layout[0]);

    let workspace_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), 
            Constraint::Percentage(70), 
        ].as_ref())
        .split(main_layout[1]);

    let notes_list: Vec<ListItem> = app.notes.iter().map(|n| {
        ListItem::new(format!(" 📝 {}", n.title)).style(Style::default().fg(Color::Rgb(255, 255, 0)))
    }).collect();

    let sidebar_border_color = match app.mode {
        Mode::SelectNote => Color::Rgb(138, 43, 226), 
        _ => Color::DarkGray,
    };

    let list_widget = List::new(notes_list)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Document Index ")
            .title_style(Style::default().fg(Color::Rgb(0, 255, 128)))
            .border_style(Style::default().fg(sidebar_border_color)))
        .highlight_style(Style::default().bg(Color::Rgb(255, 0, 128)).fg(Color::White).add_modifier(Modifier::BOLD))
        .highlight_symbol("👁 ");
    f.render_stateful_widget(list_widget, workspace_chunks[0], &mut app.list_state);

    let current_selected_index = app.list_state.selected().unwrap_or(0);
    let pane_area = workspace_chunks[1];
    
    let (body_text, pane_title, border_color, text_color) = match app.mode {
        Mode::SelectNote => {
            let content = app.notes.get(current_selected_index).map(|n| n.content.as_str()).unwrap_or("");
            (content.to_string(), " Read Pane (u/d to Scroll) ", Color::Rgb(0, 191, 255), Color::Rgb(200, 255, 200))
        },
        Mode::EditContent => {
            (app.input_buffer.clone(), " EDITING MODE (Press ESC to save) ", Color::Rgb(255, 69, 0), Color::Rgb(0, 255, 255))
        },
        Mode::CreateNew => {
            (app.input_buffer.clone(), " Enter file name + press Enter ", Color::Rgb(255, 255, 0), Color::Rgb(255, 255, 255))
        }
    };

    if app.mode == Mode::EditContent || app.mode == Mode::CreateNew {
        let lines: Vec<&str> = body_text.split('\n').collect();
        let current_line_idx = lines.len().saturating_sub(1);
        let current_line = lines.last().unwrap_or(&"");
        
        let cursor_x = pane_area.x + 2 + (current_line.len() as u16 % (pane_area.width - 4));
        let base_cursor_y = pane_area.y + 1 + (current_line_idx as u16) + (current_line.len() as u16 / (pane_area.width - 4));
        
        if base_cursor_y >= pane_area.y + pane_area.height - 2 {
            app.scroll_offset = base_cursor_y - (pane_area.y + pane_area.height - 3);
        }
        
        let cursor_y = base_cursor_y.saturating_sub(app.scroll_offset);
        if cursor_y > pane_area.y && cursor_y < pane_area.y + pane_area.height - 1 {
            f.set_cursor(cursor_x, cursor_y);
        }
    }

    let content_pane = Paragraph::new(body_text)
        .style(Style::default().fg(text_color))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0)) 
        .block(Block::default()
            .borders(Borders::ALL)
            .title(pane_title)
            .title_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(border_color)));
    f.render_widget(content_pane, pane_area);

    // Dynamic Footer menu string update to show both control setups
    let footer_text = match app.mode {
        Mode::SelectNote => "🔀 Arrows / j/k: Navigate | u/d: Scroll Note | n: New | e: Edit | x: Delete | q: Quit",
        Mode::EditContent => "Typing Allowed... Hit 'Enter' for new line | Press 'Esc' to Save & Close",
        Mode::CreateNew => "Type file name then press 'Enter' to confirm or 'Esc' to cancel",
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Rgb(180, 180, 180)))
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(footer, main_layout[2]);
}
