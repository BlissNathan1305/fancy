use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Terminal,
};
use std::{io, time::Duration};

const WORD_LENGTH: usize = 5;
const MAX_ATTEMPTS: usize = 6;
const TARGET_WORD: &str = "RUSTY"; // The secret word to guess!

#[derive(Clone, Copy, PartialEq)]
enum LetterState {
    Empty,
    Pending,
    Correct,
    Misplaced,
    Incorrect,
}

struct GameState {
    guesses: Vec<String>,
    current_guess: String,
    game_over: bool,
    won: bool,
    message: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize sleek raw terminal canvas
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = GameState {
        guesses: Vec::new(),
        current_guess: String::new(),
        game_over: false,
        won: false,
        message: "Type a 5-letter word and hit Enter!".to_string(),
    };

    loop {
        // 2. Render UI Layout Frame
        terminal.draw(|f| {
            let area = f.size();
            
            // Outer aesthetic framing panel
            let main_block = Block::default()
                .title(" 🕹️  TERMINAL WORD PUZZLE ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(139, 92, 246))); // Lavender Border
            
            let inner_area = main_block.inner(area);
            f.render_widget(main_block, area);

            // App grid hierarchy layout
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),      // Top padding
                    Constraint::Length(2),      // Status message row
                    Constraint::Length(15),     // Word grid box
                    Constraint::Min(0),         // Vertical spacer
                    Constraint::Length(2),      // Instruction Bar
                ])
                .split(inner_area);

            // Render active game notification bar
            let msg_style = if state.won {
                Style::default().fg(Color::Green).bold()
            } else if state.game_over {
                Style::default().fg(Color::Red).bold()
            } else {
                Style::default().fg(Color::Cyan)
            };
            let msg_para = Paragraph::new(state.message.as_str())
                .alignment(Alignment::Center)
                .style(msg_style);
            f.render_widget(msg_para, chunks[1]);

            // Render the Grid Row by Row
            let grid_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Length(2); MAX_ATTEMPTS])
                .split(chunks[2]);

            for row in 0..MAX_ATTEMPTS {
                let mut spans = vec![];

                for col in 0..WORD_LENGTH {
                    let mut letter = " ".to_string();
                    let mut cell_state = LetterState::Empty;

                    if row < state.guesses.len() {
                        // Evaluated past historical guesses
                        let word = &state.guesses[row];
                        let ch = word.chars().nth(col).unwrap();
                        letter = ch.to_string();
                        
                        if TARGET_WORD.chars().nth(col).unwrap() == ch {
                            cell_state = LetterState::Correct;
                        } else if TARGET_WORD.contains(ch) {
                            cell_state = LetterState::Misplaced;
                        } else {
                            cell_state = LetterState::Incorrect;
                        }
                    } else if row == state.guesses.len() {
                        // Current active row player is typing into
                        if col < state.current_guess.len() {
                            letter = state.current_guess.chars().nth(col).unwrap().to_string();
                            cell_state = LetterState::Pending;
                        }
                    }

                    // Assign highly stylized modern background pill themes
                    let style = match cell_state {
                        LetterState::Correct => Style::default().bg(Color::Rgb(34, 197, 94)).fg(Color::White).bold(),   // Crisp Emerald Green
                        LetterState::Misplaced => Style::default().bg(Color::Rgb(234, 179, 8)).fg(Color::Black).bold(), // Amber Yellow
                        LetterState::Incorrect => Style::default().bg(Color::Rgb(75, 85, 99)).fg(Color::Rgb(156, 163, 175)), // Clean Slate Gray
                        LetterState::Pending => Style::default().bg(Color::Rgb(30, 41, 59)).fg(Color::Cyan).bold(),      // Soft Indigo highlight
                        LetterState::Empty => Style::default().bg(Color::Indexed(234)).fg(Color::DarkGray),
                    };

                    spans.push(Span::styled(format!("  {}  ", letter), style));
                    spans.push(Span::raw(" ")); // Character Spacing gaps
                }

                let row_line = Line::from(spans);
                let row_para = Paragraph::new(row_line).alignment(Alignment::Center);
                f.render_widget(row_para, grid_chunks[row]);
            }

            // Interactive game helper footer
            let footer_text = if state.game_over {
                "[R] Play Again  │  [Q/Esc] Exit Game"
            } else {
                "[A-Z] Type  │  [Backspace] Delete  │  [Enter] Submit  │  [Q/Esc] Quit"
            };
            let footer = Paragraph::new(footer_text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::Indexed(243)));
            f.render_widget(footer, chunks[4]);
        })?;

        // 3. User Input Processing Engine
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break;
                }

                if state.game_over {
                    if let KeyCode::Char('r') = key.code {
                        state.guesses.clear();
                        state.current_guess.clear();
                        state.game_over = false;
                        state.won = false;
                        state.message = "New game started! Guess the word.".to_string();
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char(c) => {
                        if c.is_ascii_alphabetic() && state.current_guess.len() < WORD_LENGTH {
                            state.current_guess.push(c.to_ascii_uppercase());
                        }
                    }
                    KeyCode::Backspace => {
                        state.current_guess.pop();
                    }
                    KeyCode::Enter => {
                        if state.current_guess.len() == WORD_LENGTH {
                            let guess = state.current_guess.clone();
                            state.guesses.push(guess.clone());
                            state.current_guess.clear();

                            if guess == TARGET_WORD {
                                state.won = true;
                                state.game_over = true;
                                state.message = "🎉 SPECTACULAR! You guessed the word! 🎉".to_string();
                            } else if state.guesses.len() >= MAX_ATTEMPTS {
                                state.game_over = true;
                                state.message = format!("💥 Game Over! The word was: {} 💥", TARGET_WORD);
                            } else {
                                state.message = "Keep going! Good attempt.".to_string();
                            }
                        } else {
                            state.message = "⚠️ Word must be exactly 5 letters long!".to_string();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // 4. Safe recovery cleanup routine
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
