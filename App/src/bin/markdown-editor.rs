use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Terminal,
};
use std::{env, fs, io};
use tui_textarea::TextArea;

#[derive(PartialEq)]
enum InputMode {
    Editing,
    Saving, 
}

struct App<'a> {
    textarea: TextArea<'a>, 
    file_path: Option<String>,
    input_mode: InputMode,
    save_prompt_buffer: String, 
    status_message: String,     
}

fn main() -> Result<(), io::Error> {
    // 1. Process System CLI Arguments
    let args: Vec<String> = env::args().collect();
    let (initial_content, file_path) = if args.len() > 1 {
        let path = args[1].clone();
        let content = fs::read_to_string(&path).unwrap_or_else(|_| String::new());
        (content, Some(path))
    } else {
        (
            String::from(r"# Commercial TUI Editor

Press `Ctrl + S` to save your work at any time. If this buffer is unsaved, a prompt will appear!"),
            None,
        )
    };

    // 2. Configure Textarea Component
    let mut textarea = TextArea::new(initial_content.lines().map(String::from).collect());
    textarea.set_cursor_style(Style::default().bg(Color::LightYellow).fg(Color::Black));

    let mut app = App { 
        textarea, 
        file_path,
        input_mode: InputMode::Editing,
        save_prompt_buffer: String::new(),
        status_message: String::from("Status: Ready"),
    };

    // 3. Initialize Terminal Environment
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 4. Runtime Event Processing Loop
    loop {
        terminal.draw(|f| {
            let root_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(f.size());

            let workspace_layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(root_layout[0]);

            let editor_border_color = match app.input_mode {
                InputMode::Editing => Color::Cyan,
                InputMode::Saving => Color::DarkGray,
            };

            app.textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" [ Input Editor ] ")
                    .border_style(Style::default().fg(editor_border_color)),
            );

            // Render Editor Component
            f.render_widget(app.textarea.widget(), workspace_layout[0]);

            // Parse and format Live Markdown Preview Context
            let raw_markdown = app.textarea.lines().join("\n");
            let preview_lines = parse_markdown_to_tui(&raw_markdown);

            let title_text = match &app.file_path {
                Some(name) => format!(" [ Live Preview: {} ] ", name),
                None => " [ Live Preview: Unsaved Buffer ] ".to_string(),
            };
            
            // Fixed Wrap Implementation: Formats text to stay within window boundaries
            let preview_widget = Paragraph::new(preview_lines)
                .block(
                    Block::default()
                        .title(title_text)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green)),
                )
                .wrap(Wrap { trim: false }); // Native word boundary soft-wrap
                
            f.render_widget(preview_widget, workspace_layout[1]);

            // Render bottom Status/Keybindings Guide Block
            let help_menu = Line::from(vec![
                " CTRL+S ".bold().bg(Color::White).fg(Color::Black),
                " Save   ".fg(Color::Gray),
                " CTRL+Q ".bold().bg(Color::White).fg(Color::Black),
                " Quit   ".fg(Color::Gray),
                " CTRL+Z ".bold().bg(Color::LightBlue).fg(Color::Black),
                " Undo   ".fg(Color::Gray),
                " ".into(),
                app.status_message.as_str().italic().fg(Color::LightYellow),
            ]);
            
            let help_widget = Paragraph::new(help_menu)
                .block(Block::default().borders(Borders::ALL).title(" Shortcuts Guide "));
            f.render_widget(help_widget, root_layout[1]);

            // 5. Render "Save As" Modal if state demands it
            if app.input_mode == InputMode::Saving {
                let area = centered_rect(60, 20, f.size());
                let save_prompt_para = Paragraph::new(format!(
                    "\n Enter filename: {}\n\n [ Press Enter to Confirm | Esc to Cancel ]", 
                    app.save_prompt_buffer
                ))
                .block(Block::default().title(" Save File As ").borders(Borders::ALL).border_style(Style::default().fg(Color::LightMagenta)))
                .style(Style::default().bg(Color::Black));
                
                f.render_widget(Clear, area); 
                f.render_widget(save_prompt_para, area);
            }
        })?;

        // 6. Intercept and Dispatch Keyboard Events based on state context
        if let Event::Key(key) = event::read()? {
            match app.input_mode {
                InputMode::Editing => {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), KeyModifiers::CONTROL) => break,
                        
                        (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                            if app.file_path.is_some() {
                                let path = app.file_path.as_ref().unwrap();
                                let data = app.textarea.lines().join("\n");
                                match fs::write(path, data) {
                                    Ok(_) => app.status_message = format!("Status: Written to {}", path),
                                    Err(e) => app.status_message = format!("Error: {}", e),
                                }
                            } else {
                                app.input_mode = InputMode::Saving;
                                app.status_message = String::from("Status: Entering File Save Name Mode");
                            }
                        }
                        _ => { app.textarea.input(key); }
                    }
                }
                InputMode::Saving => {
                    match key.code {
                        KeyCode::Esc => {
                            app.input_mode = InputMode::Editing;
                            app.save_prompt_buffer.clear();
                            app.status_message = String::from("Status: Save Cancelled");
                        }
                        KeyCode::Enter => {
                            if !app.save_prompt_buffer.trim().is_empty() {
                                let filename = app.save_prompt_buffer.trim().to_string();
                                let data = app.textarea.lines().join("\n");
                                match fs::write(&filename, data) {
                                    Ok(_) => {
                                        app.file_path = Some(filename.clone());
                                        app.status_message = format!("Status: File saved to {}", filename);
                                    }
                                    Err(e) => app.status_message = format!("Error saving: {}", e),
                                }
                                app.input_mode = InputMode::Editing;
                                app.save_prompt_buffer.clear();
                            }
                        }
                        KeyCode::Char(c) => {
                            app.save_prompt_buffer.push(c);
                        }
                        KeyCode::Backspace => {
                            app.save_prompt_buffer.pop();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // 7. Graceful Framework Teardown
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

/// Computes centered bounds for overlay windows
fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Parses plain text buffers into multi-color TUI Span selections
fn parse_markdown_to_tui(markdown: &str) -> Vec<Line> {
    use pulldown_cmark::{Event as MdEvent, Options, Parser, Tag, TagEnd};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(markdown, options);

    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut current_style = Style::default();

    for event in parser {
        match event {
            MdEvent::Start(Tag::Heading { .. }) => {
                current_style = current_style.fg(Color::LightMagenta).add_modifier(Modifier::BOLD);
                current_line.push(Span::styled("# ", current_style));
            }
            MdEvent::End(TagEnd::Heading(_)) => {
                current_style = current_style.fg(Color::Reset).remove_modifier(Modifier::BOLD);
                lines.push(Line::from(current_line.clone()));
                current_line.clear();
            }
            MdEvent::Start(Tag::Emphasis) => {
                current_style = current_style.add_modifier(Modifier::ITALIC).fg(Color::LightYellow);
            }
            MdEvent::End(TagEnd::Emphasis) => {
                current_style = current_style.remove_modifier(Modifier::ITALIC).fg(Color::Reset);
            }
            MdEvent::Start(Tag::Strong) => {
                current_style = current_style.add_modifier(Modifier::BOLD).fg(Color::LightGreen);
            }
            MdEvent::End(TagEnd::Strong) => {
                current_style = current_style.remove_modifier(Modifier::BOLD).fg(Color::Reset);
            }
            MdEvent::Start(Tag::CodeBlock(_)) => {
                current_style = current_style.bg(Color::Indexed(236)).fg(Color::LightCyan);
            }
            MdEvent::End(TagEnd::CodeBlock) => {
                current_style = current_style.bg(Color::Reset).fg(Color::Reset);
            }
            MdEvent::Text(text) => {
                for part in text.split('\n') {
                    if !part.is_empty() {
                        current_line.push(Span::styled(part.to_string(), current_style));
                    }
                }
            }
            MdEvent::SoftBreak | MdEvent::HardBreak => {
                lines.push(Line::from(current_line.clone()));
                current_line.clear();
            }
            _ => {}
        }
    }
    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }
    lines
}
