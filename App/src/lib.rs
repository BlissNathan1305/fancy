//! Shared helpers for the TUI binaries in this package: terminal setup that
//! always restores the screen (even on panic), per-app data directories,
//! crash-safe file writes and a clipboard that survives the app exiting.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub mod term {
    use crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io::{self, Stdout};

    pub type Tui = Terminal<CrosstermBackend<Stdout>>;

    /// Enter raw mode and the alternate screen, and install a panic hook that
    /// puts the terminal back before the panic message is printed.
    pub fn init() -> io::Result<Tui> {
        let original = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = restore();
            original(info);
        }));
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Terminal::new(CrosstermBackend::new(io::stdout()))
    }

    pub fn restore() -> io::Result<()> {
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show)
    }

    /// Temporarily hand the terminal back to the shell, e.g. to run `$EDITOR`.
    pub fn suspend<T>(terminal: &mut Tui, f: impl FnOnce() -> T) -> io::Result<T> {
        restore()?;
        let out = f();
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        terminal.clear()?;
        Ok(out)
    }
}

/// `$XDG_DATA_HOME/<app>` (usually `~/.local/share/<app>`), created on demand.
pub fn data_dir(app: &str) -> io::Result<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home_dir().join(".local/share"));
    let dir = base.join(app);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Write to a temporary file and rename it over `path`, so a crash or full
/// disk never leaves a half-written data file behind.
pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Copy text to the system clipboard. `wl-copy`/`xclip` keep serving the
/// clipboard after this process exits, which in-process clipboards cannot.
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut tools: Vec<(&str, &[&str])> = Vec::new();
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        tools.push(("wl-copy", &[]));
    }
    if std::env::var_os("DISPLAY").is_some() {
        tools.push(("xclip", &["-selection", "clipboard"]));
    }
    for (cmd, args) in tools {
        let child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut child) = child {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
        }
    }
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text.to_owned()))
        .map_err(|e| format!("clipboard unavailable: {e}"))
}

/// Fire-and-forget desktop notification; silently does nothing without `notify-send`.
pub fn notify(title: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["--app-name", "rust-tui", title, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut c| c.wait());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_replaces_contents() {
        let dir = std::env::temp_dir().join(format!("tui-snippets-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.json");
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        assert!(!path.with_extension("tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
