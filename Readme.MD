This is a new rust project

## App (`tui-snippets`) — terminal tools

Install: `cd App && cargo install --path . --bin calc --bin cal --bin note --bin rtodo --bin rustwarden --bin pomodoro --bin snip --bin timer`

| Command | What it does | Data |
|---|---|---|
| `calc` | Calculator TUI; `calc "7/2"` prints the answer directly | — |
| `cal [YYYY-MM]` | Month calendar with date details | — |
| `note` | Markdown notebook with a full text editor | `~/.rust_notes/*.md` |
| `rtodo` | Todo list (`a` add, `e` edit, `d` delete, `J/K` reorder) | `~/.todo_list.json` |
| `rustwarden` | Encrypted password vault (Argon2id + AES-256-GCM) | `~/.local/share/rustwarden/vault.enc` |
| `pomodoro [-w 25 -s 5 -l 15]` | Pomodoro timer with notifications | — |
| `snip` | Code snippet manager; `a` add / `e` edit in `$EDITOR` | `~/.local/share/snip/snippets.json` |
| `timer [25m] [-l label]` | Countdown timer with pause and ±1 minute | — |
| `wordle` | Word game (not installed by default) | — |
| `markdown-editor [file]` | Markdown editor with live preview (not installed by default) | — |

`rustwarden --import FILE` merges another vault; `snip --import FILE` merges another snippets file.

## Hustle — `call`

Modal task list (`a` add, `e` edit, Space done, `d` delete). Tasks are saved to `~/.local/share/call/tasks.json`.

## App/simple-file-manager — `rust-fm`

Graphical file manager (iced). Build with `cargo build --release` and copy `target/release/file` to `~/.local/bin/rust-fm`.
