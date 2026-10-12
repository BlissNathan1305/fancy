use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use std::{collections::HashMap, io, time::Duration};
use tui_snippets::term;

const WORD_LENGTH: usize = 5;
const MAX_ATTEMPTS: usize = 6;

const ANSWERS: &[&str] = &[
    "ABOUT", "ABOVE", "ACTOR", "ADOPT", "AFTER", "AGAIN", "AGENT", "ALARM", "ALBUM", "ALERT",
    "ALIEN", "ALIVE", "ALLOW", "ALONE", "AMBER", "ANGEL", "ANGRY", "APPLE", "ARENA", "ARRAY",
    "ASSET", "AUDIO", "AWARD", "BADGE", "BAKER", "BASIC", "BEACH", "BENCH", "BLADE", "BLAME",
    "BLANK", "BLAST", "BLEND", "BLOCK", "BLOOM", "BOARD", "BRAIN", "BRAVE", "BREAD", "BRICK",
    "BRIDE", "BRIEF", "BRING", "BROAD", "BRUSH", "BUILD", "CABIN", "CABLE", "CANDY", "CARGO",
    "CHAIN", "CHAIR", "CHALK", "CHARM", "CHART", "CHASE", "CHEAP", "CHECK", "CHESS", "CHIEF",
    "CHILD", "CLAIM", "CLASS", "CLEAN", "CLEAR", "CLIMB", "CLOCK", "CLOUD", "COAST", "CORAL",
    "COUNT", "COURT", "CRAFT", "CRANE", "CRASH", "CREAM", "CRISP", "CROWN", "CURVE", "CYCLE",
    "DAILY", "DANCE", "DEPTH", "DIARY", "DRAFT", "DRAMA", "DREAM", "DRESS", "DRIFT", "DRINK",
    "DRIVE", "EAGLE", "EARLY", "EARTH", "EIGHT", "ELBOW", "EMPTY", "ENJOY", "ENTRY", "EQUAL",
    "ERROR", "EVENT", "EXACT", "EXTRA", "FAITH", "FALSE", "FEAST", "FIELD", "FIFTY", "FINAL",
    "FLAME", "FLASH", "FLEET", "FLOOR", "FOCUS", "FORGE", "FRAME", "FRESH", "FRONT", "FRUIT",
    "GHOST", "GIANT", "GLASS", "GLOBE", "GLORY", "GRACE", "GRAIN", "GRAND", "GRAPE", "GRASS",
    "GREAT", "GREEN", "GUARD", "GUEST", "GUIDE", "HABIT", "HAPPY", "HEART", "HEAVY", "HONEY",
    "HORSE", "HOTEL", "HOUSE", "HUMAN", "IDEAL", "IMAGE", "INDEX", "INNER", "INPUT", "ISSUE",
    "JEWEL", "JUICE", "KNIFE", "LASER", "LAYER", "LEARN", "LEMON", "LEVEL", "LIGHT", "LIMIT",
    "LOCAL", "LOGIC", "LUCKY", "LUNCH", "MAGIC", "MAJOR", "MANGO", "MAPLE", "MARCH", "MATCH",
    "MEDAL", "METAL", "MODEL", "MONEY", "MONTH", "MOTOR", "MOUNT", "MOUSE", "MOVIE", "MUSIC",
    "NERVE", "NIGHT", "NOBLE", "NOISE", "NORTH", "NOVEL", "OCEAN", "OFFER", "OLIVE", "ORBIT",
    "ORDER", "OTHER", "PAINT", "PANEL", "PAPER", "PARTY", "PEACE", "PEARL", "PHONE", "PIANO",
    "PILOT", "PIXEL", "PLACE", "PLAIN", "PLANE", "PLANT", "PLATE", "POINT", "POWER", "PRESS",
    "PRICE", "PRIDE", "PRIME", "PRINT", "PRIZE", "PROOF", "PROUD", "QUEEN", "QUERY", "QUICK",
    "QUIET", "RADAR", "RADIO", "RAISE", "RANGE", "RAPID", "REACH", "READY", "RIVER", "ROBOT",
    "ROUND", "ROUTE", "ROYAL", "RUSTY", "SALAD", "SCALE", "SCENE", "SCORE", "SENSE", "SERVE",
    "SEVEN", "SHADE", "SHAPE", "SHARE", "SHARP", "SHELF", "SHELL", "SHIFT", "SHINE", "SHIRT",
    "SHORE", "SIGHT", "SKILL", "SLEEP", "SLICE", "SMART", "SMILE", "SMOKE", "SNAKE", "SOLAR",
    "SOLID", "SOUND", "SOUTH", "SPACE", "SPARK", "SPEED", "SPICE", "SPINE", "SPORT", "STACK",
    "STAFF", "STAGE", "STAIR", "STAMP", "STAND", "STARE", "START", "STEAM", "STEEL", "STONE",
    "STORM", "STORY", "SUGAR", "SUNNY", "SWEET", "SWIFT", "TABLE", "TASTE", "TEACH", "THEME",
    "THICK", "THINK", "THREE", "TIGER", "TITLE", "TOAST", "TODAY", "TOKEN", "TOOTH", "TORCH",
    "TOTAL", "TOUCH", "TOWER", "TRACK", "TRADE", "TRAIL", "TRAIN", "TREND", "TRIAL", "TRUST",
    "TRUTH", "UNCLE", "UNION", "UNITY", "UPPER", "URBAN", "USUAL", "VALID", "VALUE", "VIDEO",
    "VIRUS", "VISIT", "VITAL", "VIVID", "VOICE", "WAGON", "WATCH", "WATER", "WHEAT", "WHEEL",
    "WHITE", "WHOLE", "WORLD", "WORTH", "WRITE", "YOUTH", "ZEBRA",
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum LetterState {
    Incorrect,
    Misplaced,
    Correct,
}

/// Standard Wordle scoring: exact matches first, then each remaining letter of
/// the answer can mark at most one misplaced guess letter.
fn score(guess: &str, answer: &str) -> [LetterState; WORD_LENGTH] {
    let guess: Vec<char> = guess.chars().collect();
    let answer: Vec<char> = answer.chars().collect();
    let mut result = [LetterState::Incorrect; WORD_LENGTH];
    let mut unmatched: HashMap<char, usize> = HashMap::new();
    for i in 0..WORD_LENGTH {
        if guess[i] == answer[i] {
            result[i] = LetterState::Correct;
        } else {
            *unmatched.entry(answer[i]).or_default() += 1;
        }
    }
    for i in 0..WORD_LENGTH {
        if result[i] == LetterState::Correct {
            continue;
        }
        if let Some(count) = unmatched.get_mut(&guess[i]) {
            if *count > 0 {
                *count -= 1;
                result[i] = LetterState::Misplaced;
            }
        }
    }
    result
}

fn random_answer() -> &'static str {
    let mut buf = [0u8; 8];
    if getrandom::fill(&mut buf).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        buf[..4].copy_from_slice(&nanos.to_le_bytes());
    }
    ANSWERS[(u64::from_le_bytes(buf) % ANSWERS.len() as u64) as usize]
}

struct GameState {
    answer: &'static str,
    guesses: Vec<String>,
    current_guess: String,
    game_over: bool,
    won: bool,
    message: String,
}

impl GameState {
    fn new() -> Self {
        Self {
            answer: random_answer(),
            guesses: Vec::new(),
            current_guess: String::new(),
            game_over: false,
            won: false,
            message: "Type a 5-letter word and hit Enter!".to_string(),
        }
    }

    fn submit(&mut self) {
        if self.current_guess.len() != WORD_LENGTH {
            self.message = "⚠️ Word must be exactly 5 letters long!".to_string();
            return;
        }
        let guess = std::mem::take(&mut self.current_guess);
        self.guesses.push(guess.clone());
        if guess == self.answer {
            self.won = true;
            self.game_over = true;
            self.message = format!(
                "🎉 Solved in {}/{}! Press R to play again.",
                self.guesses.len(),
                MAX_ATTEMPTS
            );
        } else if self.guesses.len() >= MAX_ATTEMPTS {
            self.game_over = true;
            self.message = format!(
                "💥 Game over! The word was {}. Press R to play again.",
                self.answer
            );
        } else {
            self.message = format!("{} guesses left.", MAX_ATTEMPTS - self.guesses.len());
        }
    }

    /// Best known state of every letter guessed so far, for the keyboard hint.
    fn letter_hints(&self) -> HashMap<char, LetterState> {
        let mut hints: HashMap<char, LetterState> = HashMap::new();
        for guess in &self.guesses {
            for (ch, state) in guess.chars().zip(score(guess, self.answer)) {
                let entry = hints.entry(ch).or_insert(state);
                *entry = (*entry).max(state);
            }
        }
        hints
    }
}

fn tile_style(state: Option<LetterState>, pending: bool) -> Style {
    match state {
        Some(LetterState::Correct) => Style::default()
            .bg(Color::Rgb(34, 197, 94))
            .fg(Color::White)
            .bold(),
        Some(LetterState::Misplaced) => Style::default()
            .bg(Color::Rgb(234, 179, 8))
            .fg(Color::Black)
            .bold(),
        Some(LetterState::Incorrect) => Style::default()
            .bg(Color::Rgb(75, 85, 99))
            .fg(Color::Rgb(200, 200, 210)),
        None if pending => Style::default()
            .bg(Color::Rgb(30, 41, 59))
            .fg(Color::Cyan)
            .bold(),
        None => Style::default().bg(Color::Indexed(234)).fg(Color::DarkGray),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = term::init()?;
    let result = run(&mut terminal);
    term::restore()?;
    result?;
    Ok(())
}

fn run(terminal: &mut term::Tui) -> io::Result<()> {
    let mut state = GameState::new();
    loop {
        terminal.draw(|f| draw(f, &state))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Esc => return Ok(()),
            KeyCode::Char('q') | KeyCode::Char('Q') if state.game_over => return Ok(()),
            KeyCode::Char('r') | KeyCode::Char('R') if state.game_over => state = GameState::new(),
            _ if state.game_over => {}
            KeyCode::Char(c)
                if c.is_ascii_alphabetic() && state.current_guess.len() < WORD_LENGTH =>
            {
                state.current_guess.push(c.to_ascii_uppercase());
            }
            KeyCode::Backspace => {
                state.current_guess.pop();
            }
            KeyCode::Enter => state.submit(),
            _ => {}
        }
    }
}

fn draw(f: &mut ratatui::Frame, state: &GameState) {
    let main_block = Block::default()
        .title(" 🕹️  TERMINAL WORD PUZZLE ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(139, 92, 246)));
    let inner_area = main_block.inner(f.size());
    f.render_widget(main_block, f.size());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(2 * MAX_ATTEMPTS as u16),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner_area);

    let msg_style = if state.won {
        Style::default().fg(Color::Green).bold()
    } else if state.game_over {
        Style::default().fg(Color::Red).bold()
    } else {
        Style::default().fg(Color::Cyan)
    };
    f.render_widget(
        Paragraph::new(state.message.as_str())
            .alignment(Alignment::Center)
            .style(msg_style),
        chunks[1],
    );

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(2); MAX_ATTEMPTS])
        .split(chunks[2]);
    for (row, area) in rows.iter().enumerate() {
        let mut spans = Vec::new();
        let scored = state.guesses.get(row).map(|g| (g, score(g, state.answer)));
        for col in 0..WORD_LENGTH {
            let (letter, style) = match &scored {
                Some((guess, states)) => (
                    guess.chars().nth(col).unwrap_or(' '),
                    tile_style(Some(states[col]), false),
                ),
                None if row == state.guesses.len() => match state.current_guess.chars().nth(col) {
                    Some(c) => (c, tile_style(None, true)),
                    None => (' ', tile_style(None, false)),
                },
                None => (' ', tile_style(None, false)),
            };
            spans.push(Span::styled(format!("  {}  ", letter), style));
            spans.push(Span::raw(" "));
        }
        f.render_widget(
            Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
            *area,
        );
    }

    let hints = state.letter_hints();
    let keyboard: Vec<Line> = ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM"]
        .iter()
        .map(|row| {
            Line::from(
                row.chars()
                    .flat_map(|c| {
                        [
                            Span::styled(
                                format!(" {} ", c),
                                tile_style(hints.get(&c).copied(), false),
                            ),
                            Span::raw(" "),
                        ]
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    f.render_widget(
        Paragraph::new(keyboard).alignment(Alignment::Center),
        chunks[4],
    );

    let footer = if state.game_over {
        "[R] Play Again  │  [Q/Esc] Exit"
    } else {
        "[A-Z] Type  │  [Backspace] Delete  │  [Enter] Submit  │  [Esc] Quit"
    };
    f.render_widget(
        Paragraph::new(footer)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Indexed(243))),
        chunks[6],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use LetterState::*;

    #[test]
    fn duplicate_letters_score_like_wordle() {
        // Only one L in the answer: the exact match wins and the other L is grey.
        assert_eq!(
            score("LLAMA", "HELLO"),
            [Misplaced, Misplaced, Incorrect, Incorrect, Incorrect]
        );
        assert_eq!(
            score("SPEED", "ABIDE"),
            [Incorrect, Incorrect, Misplaced, Incorrect, Misplaced]
        );
        assert_eq!(score("ROBOT", "ROBOT"), [Correct; 5]);
        assert_eq!(
            score("TOOTH", "ROBOT"),
            [Misplaced, Correct, Misplaced, Incorrect, Incorrect]
        );
    }

    #[test]
    fn answers_are_valid() {
        for w in ANSWERS {
            assert_eq!(w.len(), WORD_LENGTH, "{w}");
            assert!(w.chars().all(|c| c.is_ascii_uppercase()), "{w}");
        }
    }

    #[test]
    fn game_flow() {
        let mut g = GameState::new();
        g.answer = "CRANE";
        g.current_guess = "CRAN".into();
        g.submit();
        assert!(g.guesses.is_empty());
        g.current_guess = "CRANE".into();
        g.submit();
        assert!(g.won && g.game_over);
    }
}
