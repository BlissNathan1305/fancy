use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use std::{env, fs, io};
use tui_snippets::term;
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
    /// Text as last loaded or saved, to detect unsaved changes.
    saved_text: String,
    quit_armed: bool,
}

impl App<'_> {
    fn text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    fn is_dirty(&self) -> bool {
        self.text() != self.saved_text
    }

    fn write_to(&mut self, path: &str) {
        let data = self.text();
        // Keep the conventional trailing newline on disk.
        match fs::write(path, format!("{data}\n")) {
            Ok(_) => {
                self.file_path = Some(path.to_string());
                self.saved_text = data;
                self.status_message = format!("Saved {}", path);
            }
            Err(e) => self.status_message = format!("Error saving {}: {}", path, e),
        }
    }
}

fn main() -> Result<(), io::Error> {
    // 1. Process System CLI Arguments
    let args: Vec<String> = env::args().collect();
    let (initial_content, file_path) = if args.len() > 1 {
        let path = args[1].clone();
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            // Refuse to open (and later overwrite) a file we could not read.
            Err(e) => {
                eprintln!("markdown-editor: cannot read {}: {}", path, e);
                std::process::exit(1);
            }
        };
        (content, Some(path))
    } else {
        (
            String::from(
                r"# Commercial TUI Editor

Press `Ctrl + S` to save your work at any time. If this buffer is unsaved, a prompt will appear!",
            ),
            None,
        )
    };

    // 2. Configure Textarea Component
    let lines: Vec<String> = initial_content.lines().map(String::from).collect();
    let saved_text = if file_path.is_some() {
        lines.join("\n")
    } else {
        String::new()
    };
    let mut textarea = TextArea::new(lines);
    textarea.set_cursor_style(Style::default().bg(Color::LightYellow).fg(Color::Black));

    let mut app = App {
        textarea,
        file_path,
        input_mode: InputMode::Editing,
        save_prompt_buffer: String::new(),
        status_message: String::from("Status: Ready"),
        saved_text,
        quit_armed: false,
    };

    let mut terminal = term::init()?;
    let result = run(&mut terminal, &mut app);
    term::restore()?;
    result
}

fn run(terminal: &mut term::Tui, app: &mut App) -> io::Result<()> {
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

            let dirty = if app.is_dirty() { " ●" } else { "" };
            let title_text = match &app.file_path {
                Some(name) => format!(" [ Live Preview: {}{} ] ", name, dirty),
                None => format!(" [ Live Preview: Unsaved Buffer{} ] ", dirty),
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

            let help_widget = Paragraph::new(help_menu).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Shortcuts Guide "),
            );
            f.render_widget(help_widget, root_layout[1]);

            // 5. Render "Save As" Modal if state demands it
            if app.input_mode == InputMode::Saving {
                let area = centered_rect(60, 20, f.size());
                let save_prompt_para = Paragraph::new(format!(
                    "\n Enter filename: {}\n\n [ Press Enter to Confirm | Esc to Cancel ]",
                    app.save_prompt_buffer
                ))
                .block(
                    Block::default()
                        .title(" Save File As ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::LightMagenta)),
                )
                .style(Style::default().bg(Color::Black));

                f.render_widget(Clear, area);
                f.render_widget(save_prompt_para, area);
            }
        })?;

        // 6. Intercept and Dispatch Keyboard Events based on state context
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let quit_armed = std::mem::take(&mut app.quit_armed);
            match app.input_mode {
                InputMode::Editing => match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                        if !app.is_dirty() || quit_armed {
                            break;
                        }
                        app.quit_armed = true;
                        app.status_message = String::from(
                            "Unsaved changes! Ctrl+S to save, Ctrl+Q again to discard and quit",
                        );
                    }

                    (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                        if let Some(path) = app.file_path.clone() {
                            app.write_to(&path);
                        } else {
                            app.input_mode = InputMode::Saving;
                            app.status_message =
                                String::from("Status: Entering File Save Name Mode");
                        }
                    }
                    _ => {
                        app.textarea.input(key);
                    }
                },
                InputMode::Saving => match key.code {
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Editing;
                        app.save_prompt_buffer.clear();
                        app.status_message = String::from("Status: Save Cancelled");
                    }
                    KeyCode::Enter if !app.save_prompt_buffer.trim().is_empty() => {
                        let filename = app.save_prompt_buffer.trim().to_string();
                        app.write_to(&filename);
                        app.input_mode = InputMode::Editing;
                        app.save_prompt_buffer.clear();
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.save_prompt_buffer.push(c);
                    }
                    KeyCode::Backspace => {
                        app.save_prompt_buffer.pop();
                    }
                    _ => {}
                },
            }
        }
    }

    Ok(())
}

/// Computes centered bounds for overlay windows
fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
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

/// Renders Markdown into styled lines for the preview pane.
fn parse_markdown_to_tui(markdown: &str) -> Vec<Line<'static>> {
    use pulldown_cmark::{Event as MdEvent, HeadingLevel, Options, Parser, Tag, TagEnd};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut styles: Vec<Style> = vec![Style::default()];
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut in_code_block = false;
    let mut quote_depth = 0usize;

    fn flush(lines: &mut Vec<Line<'static>>, current: &mut Vec<Span<'static>>, quote_depth: usize) {
        let mut spans = Vec::new();
        if quote_depth > 0 {
            spans.push(Span::styled(
                "│ ".repeat(quote_depth),
                Style::default().fg(Color::Magenta),
            ));
        }
        spans.append(current);
        lines.push(Line::from(spans));
    }
    fn blank(lines: &mut Vec<Line<'static>>) {
        if lines.last().is_some_and(|l| !l.spans.is_empty()) {
            lines.push(Line::default());
        }
    }

    for event in Parser::new_ext(markdown, options) {
        let style = *styles.last().unwrap_or(&Style::default());
        match event {
            MdEvent::Start(Tag::Heading { level, .. }) => {
                let (marker, color) = match level {
                    HeadingLevel::H1 => ("█ ", Color::LightMagenta),
                    HeadingLevel::H2 => ("▓▓ ", Color::LightCyan),
                    HeadingLevel::H3 => ("▒▒▒ ", Color::LightBlue),
                    _ => ("░░░░ ", Color::LightGreen),
                };
                let heading = Style::default().fg(color).add_modifier(Modifier::BOLD);
                current.push(Span::styled(marker, heading));
                styles.push(heading);
            }
            MdEvent::End(TagEnd::Heading(_)) => {
                styles.pop();
                flush(&mut lines, &mut current, quote_depth);
                lines.push(Line::default());
            }
            MdEvent::End(TagEnd::Paragraph) => {
                flush(&mut lines, &mut current, quote_depth);
                if list_stack.is_empty() {
                    lines.push(Line::default());
                }
            }
            MdEvent::Start(Tag::Emphasis) => {
                styles.push(style.add_modifier(Modifier::ITALIC).fg(Color::LightYellow))
            }
            MdEvent::Start(Tag::Strong) => {
                styles.push(style.add_modifier(Modifier::BOLD).fg(Color::LightGreen))
            }
            MdEvent::Start(Tag::Strikethrough) => {
                styles.push(style.add_modifier(Modifier::CROSSED_OUT))
            }
            MdEvent::Start(Tag::Link { .. }) => styles.push(
                style
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            MdEvent::End(
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link,
            ) => {
                styles.pop();
            }
            MdEvent::Start(Tag::BlockQuote) => quote_depth += 1,
            MdEvent::End(TagEnd::BlockQuote) => quote_depth = quote_depth.saturating_sub(1),
            MdEvent::Start(Tag::List(start)) => list_stack.push(start),
            MdEvent::End(TagEnd::List(_)) => {
                list_stack.pop();
                if list_stack.is_empty() {
                    blank(&mut lines);
                }
            }
            MdEvent::Start(Tag::Item) => {
                let indent = "  ".repeat(list_stack.len().saturating_sub(1));
                let bullet = match list_stack.last_mut() {
                    Some(Some(n)) => {
                        let b = format!("{indent}{n}. ");
                        *n += 1;
                        b
                    }
                    _ => format!("{indent}• "),
                };
                current.push(Span::styled(bullet, Style::default().fg(Color::Magenta)));
            }
            MdEvent::End(TagEnd::Item) if !current.is_empty() => {
                flush(&mut lines, &mut current, quote_depth);
            }
            MdEvent::TaskListMarker(done) => {
                current.push(Span::styled(
                    if done { "[✔] " } else { "[ ] " },
                    Style::default().fg(Color::Green),
                ));
            }
            MdEvent::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                styles.push(
                    Style::default()
                        .bg(Color::Indexed(236))
                        .fg(Color::LightCyan),
                );
            }
            MdEvent::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                styles.pop();
                lines.push(Line::default());
            }
            MdEvent::Text(text) if in_code_block => {
                for part in text.trim_end_matches('\n').split('\n') {
                    current.push(Span::styled(format!(" {part} "), style));
                    flush(&mut lines, &mut current, quote_depth);
                }
            }
            MdEvent::Text(text) => current.push(Span::styled(text.to_string(), style)),
            MdEvent::Code(code) => {
                current.push(Span::styled(
                    format!(" {code} "),
                    Style::default().bg(Color::Indexed(236)).fg(Color::LightRed),
                ));
            }
            MdEvent::SoftBreak => current.push(Span::styled(" ", style)),
            MdEvent::HardBreak => flush(&mut lines, &mut current, quote_depth),
            MdEvent::Rule => {
                lines.push(Line::from(Span::styled(
                    "─".repeat(40),
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::default());
            }
            _ => {}
        }
    }
    if !current.is_empty() {
        flush(&mut lines, &mut current, quote_depth);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn paragraphs_lists_and_code_stay_on_separate_lines() {
        let md = "# Title\n\nFirst para.\n\nSecond para.\n\n- one\n- two\n\n```\nlet a = 1;\nlet b = 2;\n```\n";
        let out = text(&parse_markdown_to_tui(md));
        assert!(out.contains(&"█ Title".to_string()));
        assert!(out.contains(&"First para.".to_string()));
        assert!(out.contains(&"Second para.".to_string()));
        assert!(out.contains(&"• one".to_string()));
        assert!(out.contains(&"• two".to_string()));
        assert!(out.contains(&" let a = 1; ".to_string()));
        assert!(out.contains(&" let b = 2; ".to_string()));
    }

    #[test]
    fn ordered_lists_count_up() {
        let out = text(&parse_markdown_to_tui("3. c\n4. d\n"));
        assert_eq!(&out[..2], &["3. c".to_string(), "4. d".to_string()]);
    }
}
