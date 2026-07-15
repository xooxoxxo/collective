# Collective Interactive (TUI + collect) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a ratatui TUI (bare `collective`) with live filter / table / detail / favorites / prefill-to-shell, plus a `collect '<cmd>'` command that captures commands into the user overlay via AI or manual entry.

**Architecture:** New view + capture layers over the existing `corpus::load()` / `search::search()`. TUI state is pure and unit-tested; rendering is a thin ratatui layer. `collect` writes YAML to `~/.collective/corpus/` (the overlay the app already merges at runtime). AI population picks a backend (Anthropic API → local `claude` CLI → manual fallback).

**Tech Stack:** Rust, clap 4, ratatui + crossterm, serde/serde_yaml/serde_json, ureq (blocking HTTP), directories, arboard, rand.

## Global Constraints

- Binary name `collective`, crate `collective`.
- The TUI NEVER executes a corpus command. Select = write to `$COLLECTIVE_PICK` file + clipboard; the shell prompt is the confirm gate.
- Collected entries go to `~/.collective/corpus/<id>.yaml` (overlay), never the repo `corpus/`.
- Collected entry `source` = `collect:<hostname>`.
- Entry ids: lowercase ascii letters, digits, hyphens; unique across corpus + overlay.
- AI backend order: `ANTHROPIC_API_KEY` → direct API; else `claude` on PATH → `claude -p ... --output-format json --model <m>`; else manual. Model from `COLLECTIVE_MODEL`, default `claude-haiku-4-5-20251001`. No live AI calls in tests.
- Corrupt/missing favorites or overlay → warn + continue, never crash.
- TUI restores terminal (raw mode off, leave alt-screen) on every exit path including panic.
- Existing subcommands (search/show/copy/random/drill) unchanged. Zero-warning build. Conventional commits. Commit after each task.

## File Structure

```
Cargo.toml           # add ratatui, ureq
src/entry.rs         # add Serialize derive to Entry + Danger; Danger::parse (Task 6)
src/favorites.rs     # NEW: favorites persistence (drill.rs pattern)
src/tui/mod.rs       # NEW: App state (pure) + event loop + terminal setup/teardown
src/tui/ui.rs        # NEW: ratatui rendering
src/ai.rs            # NEW: backend selection + JSON parse + populate()
src/collect.rs       # NEW: collect flow, slug/uniquify, write overlay
src/main.rs          # Option<Cmd> (bare -> tui); collect subcommand; --print-shell; $COLLECTIVE_PICK
shell/collective.zsh # NEW
shell/collective.bash# NEW
tests/cli.rs         # extend: print-shell, collect --manual
```

---

### Task 1: Dependencies, Entry Serialize, favorites persistence

**Files:**
- Modify: `Cargo.toml`, `src/entry.rs`
- Create: `src/favorites.rs`
- Modify: `src/main.rs` (add `mod favorites;`)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `entry::Entry` and `entry::Danger` additionally derive `serde::Serialize` (Danger keeps `#[serde(rename_all = "lowercase")]`, now applying to both directions).
  - `favorites::default_path() -> std::path::PathBuf` → `~/.collective/favorites.json`.
  - `favorites::load(path: &Path) -> std::collections::HashSet<String>` — missing → empty; corrupt → eprintln warning + empty; never panics.
  - `favorites::save(path: &Path, favs: &HashSet<String>) -> std::io::Result<()>` — creates parent dir; writes a JSON array sorted for stable diffs.

- [ ] **Step 1: Add dependencies**

Edit `Cargo.toml` `[dependencies]`, add:

```toml
ratatui = "0.28"
ureq = { version = "2", features = ["json"] }
```

- [ ] **Step 2: Add Serialize to Entry and Danger**

In `src/entry.rs`, change the derive lines:

```rust
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Danger {
```

- [ ] **Step 3: Write failing favorites tests**

Create `src/favorites.rs`:

```rust
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("col-fav-test-{name}.json"))
    }

    #[test]
    fn missing_file_gives_empty() {
        let p = tmp("missing");
        let _ = fs::remove_file(&p);
        assert!(load(&p).is_empty());
    }

    #[test]
    fn roundtrips() {
        let p = tmp("round");
        let mut favs = HashSet::new();
        favs.insert("pmset-disable-sleep".to_string());
        favs.insert("flush-dns-cache".to_string());
        save(&p, &favs).unwrap();
        let loaded = load(&p);
        assert_eq!(loaded, favs);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn corrupt_file_resets_without_panic() {
        let p = tmp("corrupt");
        fs::write(&p, "not json !!").unwrap();
        assert!(load(&p).is_empty());
        let _ = fs::remove_file(&p);
    }
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test favorites`
Expected: FAIL — `load`/`save` not defined.

- [ ] **Step 5: Implement favorites persistence**

Add above the tests in `src/favorites.rs`:

```rust
pub fn default_path() -> PathBuf {
    directories::BaseDirs::new()
        .expect("cannot locate home directory")
        .home_dir()
        .join(".collective/favorites.json")
}

pub fn load(path: &Path) -> HashSet<String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| {
            eprintln!("warning: favorites corrupt at {}, resetting", path.display());
            HashSet::new()
        }),
        Err(_) => HashSet::new(),
    }
}

pub fn save(path: &Path, favs: &HashSet<String>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut sorted: Vec<&String> = favs.iter().collect();
    sorted.sort();
    fs::write(path, serde_json::to_string_pretty(&sorted).expect("favorites serialize"))
}
```

Add `mod favorites;` to `src/main.rs` (after the other `mod` lines). Because no binary code calls it yet, add `#[allow(dead_code)]` on the `mod favorites;` line with comment `// consumed by the TUI (Task 3)`.

- [ ] **Step 6: Run tests + build**

Run: `cargo test favorites && cargo build`
Expected: 3 favorites tests pass; build succeeds, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/entry.rs src/favorites.rs src/main.rs
git commit -m "feat: favorites persistence + Entry Serialize + TUI deps"
```

---

### Task 2: TUI App state (pure, no terminal)

**Files:**
- Create: `src/tui/mod.rs`
- Modify: `src/main.rs` (add `mod tui;`)

**Interfaces:**
- Consumes: `entry::Entry`, `search::search`, `favorites` types.
- Produces:
  - `tui::App` with fields `all: Vec<Entry>`, `filtered: Vec<usize>`, `selected: usize`, `filter: String`, `favorites: HashSet<String>`, `fav_only: bool`.
  - `App::new(all: Vec<Entry>, favorites: HashSet<String>) -> App` (filter empty, all rows visible, selected 0).
  - `App::set_filter(&mut self, filter: &str)` — recompute `filtered` (empty filter → all indices in `all` order; else `search::search` order restricted to `all`), reset `selected` to 0, respecting `fav_only`.
  - `App::move_down(&mut self)` / `App::move_up(&mut self)` — clamp within `filtered`.
  - `App::toggle_fav_only(&mut self)` — flip flag, recompute `filtered` via `set_filter(&self.filter.clone())`.
  - `App::toggle_star(&mut self) -> Option<String>` — toggle selected entry's id in `favorites`, return the id toggled (None if no selection). Persistence is the caller's job.
  - `App::selected_entry(&self) -> Option<&Entry>`.
  - `App::visible(&self) -> Vec<&Entry>` — entries at `filtered` indices, in order.

- [ ] **Step 1: Write failing App tests**

Create `src/tui/mod.rs`:

```rust
use crate::entry::Entry;
use crate::search;
use std::collections::HashSet;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus;

    fn app() -> App {
        App::new(corpus::load(), HashSet::new())
    }

    #[test]
    fn new_shows_all_sorted() {
        let a = app();
        assert_eq!(a.filtered.len(), a.all.len());
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn filter_narrows_and_resets_selection() {
        let mut a = app();
        a.move_down();
        a.set_filter("disable sleep");
        assert!(a.visible().len() < a.all.len());
        assert_eq!(a.selected, 0);
        assert_eq!(a.selected_entry().unwrap().id, "pmset-disable-sleep");
    }

    #[test]
    fn move_clamps() {
        let mut a = app();
        a.set_filter("zzqqxxnothing"); // empty result
        a.move_down();
        assert_eq!(a.selected, 0);
        assert!(a.selected_entry().is_none());
    }

    #[test]
    fn toggle_star_adds_then_removes() {
        let mut a = app();
        a.set_filter("disable sleep");
        let id = a.toggle_star().unwrap();
        assert_eq!(id, "pmset-disable-sleep");
        assert!(a.favorites.contains("pmset-disable-sleep"));
        a.toggle_star();
        assert!(!a.favorites.contains("pmset-disable-sleep"));
    }

    #[test]
    fn fav_only_filters_to_favorites() {
        let mut a = app();
        a.set_filter("disable sleep");
        a.toggle_star(); // star pmset-disable-sleep
        a.set_filter("");
        a.toggle_fav_only();
        assert_eq!(a.visible().len(), 1);
        assert_eq!(a.visible()[0].id, "pmset-disable-sleep");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test tui::`
Expected: FAIL — `App` not defined.

- [ ] **Step 3: Implement App**

Add above the tests in `src/tui/mod.rs`:

```rust
pub struct App {
    pub all: Vec<Entry>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filter: String,
    pub favorites: HashSet<String>,
    pub fav_only: bool,
}

impl App {
    pub fn new(all: Vec<Entry>, favorites: HashSet<String>) -> App {
        let mut app = App {
            all,
            filtered: Vec::new(),
            selected: 0,
            filter: String::new(),
            favorites,
            fav_only: false,
        };
        app.recompute();
        app
    }

    fn recompute(&mut self) {
        let mut idx: Vec<usize> = if self.filter.trim().is_empty() {
            (0..self.all.len()).collect()
        } else {
            // map search results back to indices in `all`
            let hits = search::search(&self.all, &self.filter);
            hits.iter()
                .filter_map(|(e, _)| self.all.iter().position(|x| x.id == e.id))
                .collect()
        };
        if self.fav_only {
            idx.retain(|&i| self.favorites.contains(&self.all[i].id));
        }
        self.filtered = idx;
        self.selected = 0;
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.recompute();
    }

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() && self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn toggle_fav_only(&mut self) {
        self.fav_only = !self.fav_only;
        self.recompute();
    }

    pub fn toggle_star(&mut self) -> Option<String> {
        let id = self.selected_entry()?.id.clone();
        if !self.favorites.remove(&id) {
            self.favorites.insert(id.clone());
        }
        if self.fav_only {
            self.recompute();
        }
        Some(id)
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.filtered.get(self.selected).map(|&i| &self.all[i])
    }

    pub fn visible(&self) -> Vec<&Entry> {
        self.filtered.iter().map(|&i| &self.all[i]).collect()
    }
}
```

Add `mod tui;` to `src/main.rs`. The event loop / rendering land in Task 3; until then mark the `mod tui;` line `#[allow(dead_code)]` with comment `// event loop + rendering land in Task 3`.

- [ ] **Step 4: Run tests**

Run: `cargo test tui::`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add src/tui/mod.rs src/main.rs
git commit -m "feat: TUI app state (filter, selection, favorites) — pure, tested"
```

---

### Task 3: TUI rendering + event loop + bare-invocation entry point

**Files:**
- Create: `src/tui/ui.rs`
- Modify: `src/tui/mod.rs` (add `run()`, `mod ui;`), `src/main.rs` (`Option<Cmd>`, dispatch bare → `tui::run`)

**Interfaces:**
- Consumes: `App`, `favorites`, `arboard`.
- Produces:
  - `tui::run() -> std::io::Result<()>` — loads corpus + favorites, runs the full-screen loop, restores terminal on every exit path.
  - `ui::draw(f: &mut ratatui::Frame, app: &App)` — renders filter box, table, detail pane, help bar.
  - `main.rs`: `Cli.cmd: Option<Cmd>`; `None` → `tui::run()`.

- [ ] **Step 1: Implement the renderer**

Create `src/tui/ui.rs`:

```rust
use crate::entry::Danger;
use crate::tui::App;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // filter
            Constraint::Min(6),    // table
            Constraint::Length(9), // detail
            Constraint::Length(1), // help
        ])
        .split(f.area());

    // filter box
    let title = format!("collective — {} entries", app.all.len());
    let filter = Paragraph::new(format!("filter> {}", app.filter))
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(filter, chunks[0]);

    // table
    let rows = app.visible().iter().enumerate().map(|(i, e)| {
        let star = if app.favorites.contains(&e.id) { "★" } else { " " };
        let danger = format!("{:?}", e.danger).to_lowercase();
        let style = if i == app.selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else if e.danger == Danger::High {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };
        Row::new(vec![
            Cell::from(star),
            Cell::from(e.id.clone()),
            Cell::from(e.title.clone()),
            Cell::from(danger),
        ])
        .style(style)
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(30),
            Constraint::Min(20),
            Constraint::Length(7),
        ],
    )
    .header(Row::new(vec!["", "id", "title", "danger"]).style(Style::default().add_modifier(Modifier::BOLD)))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(table, chunks[1]);

    // detail pane
    let detail = match app.selected_entry() {
        Some(e) => {
            let mut lines = vec![
                Line::from(Span::styled(e.title.clone(), Style::default().add_modifier(Modifier::BOLD))),
                Line::from(format!("cmd:  {}", e.cmd)),
            ];
            if let Some(u) = e.undo.as_deref().filter(|u| !u.is_empty()) {
                lines.push(Line::from(format!("undo: {u}")));
            }
            lines.push(Line::from(format!(
                "domains: {}   danger: {:?}",
                e.domains.join(", "),
                e.danger
            )));
            lines.push(Line::from(e.explanation.trim().to_string()));
            lines.push(Line::from(format!("source: {}", e.source)));
            Paragraph::new(lines).wrap(Wrap { trim: true })
        }
        None => Paragraph::new("no match"),
    }
    .block(Block::default().borders(Borders::ALL).title("detail"));
    f.render_widget(detail, chunks[2]);

    // help bar
    let help = Paragraph::new("↵ prefill  y copy  f ★  F fav-only  type to filter  q quit")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help, chunks[3]);
}
```

- [ ] **Step 2: Implement the event loop with terminal safety**

In `src/tui/mod.rs` add `mod ui;`, imports, and `run()`:

```rust
mod ui;

use crate::favorites;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Write};

fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}

pub fn run() -> io::Result<()> {
    // Ensure the terminal is restored even on panic.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        prev(info);
    }));

    let fav_path = favorites::default_path();
    let mut app = App::new(crate::corpus::load(), favorites::load(&fav_path));

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut picked: Option<String> = None;
    let result = (|| -> io::Result<()> {
        loop {
            terminal.draw(|f| ui::draw(f, &app))?;
            let Event::Key(key) = event::read()? else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Char('q') if app.filter.is_empty() => break,
                KeyCode::Up => app.move_up(),
                KeyCode::Down => app.move_down(),
                KeyCode::Enter => {
                    if let Some(e) = app.selected_entry() {
                        picked = Some(e.cmd.clone());
                    }
                    break;
                }
                KeyCode::Char('\n') => {}
                KeyCode::Backspace => {
                    let mut f = app.filter.clone();
                    f.pop();
                    app.set_filter(&f);
                }
                // Ctrl-less bare keys drive both filter text AND actions:
                // reserve f/F/y for actions, everything else types into filter.
                KeyCode::Char('y') => {
                    if let Some(e) = app.selected_entry() {
                        let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text(e.cmd.clone()));
                    }
                }
                KeyCode::Char('f') => {
                    if let Some(_id) = app.toggle_star() {
                        let _ = favorites::save(&fav_path, &app.favorites);
                    }
                }
                KeyCode::Char('F') => app.toggle_fav_only(),
                KeyCode::Char(c) => {
                    let mut f = app.filter.clone();
                    f.push(c);
                    app.set_filter(&f);
                }
                _ => {}
            }
        }
        Ok(())
    })();

    restore();
    let _ = std::panic::take_hook();
    result?;

    if let Some(cmd) = picked {
        deliver(&cmd);
    }
    Ok(())
}
```

Note: `f`/`F`/`y` are reserved for actions and won't type into the filter — acceptable for v1 (documented ceiling; a future edit mode could free them). `q` quits only when the filter is empty, so it remains typeable in queries.

Add a `deliver` stub for now (Task 4 fills it in):

```rust
fn deliver(cmd: &str) {
    // Real prefill + clipboard land in Task 4. Print so bare use still works.
    let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text(cmd.to_string()));
    println!("{cmd}");
    let _ = io::stdout().flush();
}
```

- [ ] **Step 3: Rewire the entry point in `src/main.rs`**

Change the `Cli` struct and dispatch:

```rust
#[derive(Parser)]
#[command(name = "collective", about = "hacky script directory + console drills")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}
```

```rust
fn main() {
    let cli = Cli::parse();
    let entries = corpus::load();
    match cli.cmd {
        None => {
            if let Err(e) = tui::run() {
                eprintln!("tui error: {e}");
                std::process::exit(1);
            }
        }
        Some(Cmd::Search { query }) => cmd_search(&entries, &query.join(" ")),
        Some(Cmd::Show { id }) => cmd_show(&entries, &id),
        Some(Cmd::Copy { id }) => cmd_copy(&entries, &id),
        Some(Cmd::Random) => cmd_show(&entries, &random_id(&entries)),
        Some(Cmd::Drill { domain }) => drill::run(&entries, domain.as_deref()),
    }
}
```

Remove the `#[allow(dead_code)]` markers from `mod tui;` and `mod favorites;` in `src/main.rs` — both are now reached through `tui::run`.

- [ ] **Step 4: Build, test, manual smoke**

Run: `cargo build && cargo test`
Expected: build zero-warning; all prior tests still pass.

Manual (interactive — cannot be unit-tested):
```bash
cargo run -q            # bare -> TUI opens
# type "sleep", arrow down, press f (star), F (fav-only), Esc
cargo run -q -- search sleep   # subcommand still works
```
Confirm: filter narrows live, full cmd visible in detail, star persists to `~/.collective/favorites.json`, terminal restored cleanly on quit.

- [ ] **Step 5: Commit**

```bash
git add src/tui/mod.rs src/tui/ui.rs src/main.rs
git commit -m "feat: interactive TUI (bare collective) with filter, detail, favorites"
```

---

### Task 4: Prefill delivery + shell integration

**Files:**
- Modify: `src/tui/mod.rs` (`deliver`), `src/main.rs` (`--print-shell`)
- Create: `shell/collective.zsh`, `shell/collective.bash`
- Modify: `tests/cli.rs`

**Interfaces:**
- Consumes: `App`/`run` from Task 3.
- Produces:
  - `deliver(cmd)` writes `cmd` to the path in `$COLLECTIVE_PICK` (if set) and copies to clipboard; prints `cmd` only when `$COLLECTIVE_PICK` is unset (avoids double-echo under the wrapper).
  - `main.rs` gains a top-level `--print-shell <zsh|bash>` option that prints the wrapper and exits (handled before corpus load).

- [ ] **Step 1: Implement real `deliver` in `src/tui/mod.rs`**

Replace the Task 3 stub:

```rust
fn deliver(cmd: &str) {
    let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text(cmd.to_string()));
    match std::env::var("COLLECTIVE_PICK") {
        Ok(path) if !path.is_empty() => {
            // Wrapper reads this file and places the command on the prompt.
            let _ = std::fs::write(path, cmd);
        }
        _ => {
            // No wrapper: print so the user can copy/paste.
            println!("{cmd}");
            let _ = io::stdout().flush();
        }
    }
}
```

- [ ] **Step 2: Create the shell wrappers**

`shell/collective.zsh`:
```zsh
collective() {
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && print -z "$cmd"
}
```

`shell/collective.bash`:
```bash
collective() {
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && { READLINE_LINE="$cmd"; READLINE_POINT=${#cmd}; }
}
```

- [ ] **Step 3: Write failing integration test for --print-shell**

Add to `tests/cli.rs`:

```rust
#[test]
fn print_shell_zsh_emits_wrapper() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["--print-shell", "zsh"])
        .assert()
        .success()
        .stdout(predicates::str::contains("collective()"))
        .stdout(predicates::str::contains("print -z"));
}

#[test]
fn print_shell_bash_emits_wrapper() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["--print-shell", "bash"])
        .assert()
        .success()
        .stdout(predicates::str::contains("READLINE_LINE"));
}
```

- [ ] **Step 4: Run to verify failure**

Run: `cargo test --test cli print_shell`
Expected: FAIL — `--print-shell` unknown arg (clap error / non-zero).

- [ ] **Step 5: Implement `--print-shell` in `src/main.rs`**

Add the field to `Cli` and embed the wrapper files:

```rust
#[derive(Parser)]
#[command(name = "collective", about = "hacky script directory + console drills")]
struct Cli {
    /// Print the shell wrapper for zsh or bash, then exit
    #[arg(long, value_name = "SHELL")]
    print_shell: Option<String>,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}
```

At the very top of `main()`, before `corpus::load()`:

```rust
    let cli = Cli::parse();
    if let Some(shell) = cli.print_shell.as_deref() {
        match shell {
            "zsh" => print!("{}", include_str!("../shell/collective.zsh")),
            "bash" => print!("{}", include_str!("../shell/collective.bash")),
            other => {
                eprintln!("unknown shell '{other}' (use zsh or bash)");
                std::process::exit(1);
            }
        }
        return;
    }
    let entries = corpus::load();
```

(Delete the old `let cli = Cli::parse();` / `let entries = corpus::load();` lines that this replaces.)

- [ ] **Step 6: Run tests + manual prefill check**

Run: `cargo test`
Expected: all pass including the two print_shell tests.

Manual: `eval "$(cargo run -q -- --print-shell zsh)"` in a zsh, then run `collective`, select an entry with Enter — the command appears on your prompt, editable, not executed.

- [ ] **Step 7: Commit**

```bash
git add src/tui/mod.rs src/main.rs shell tests/cli.rs
git commit -m "feat: prefill-to-shell delivery and --print-shell wrappers"
```

---

### Task 5: AI backend (`src/ai.rs`)

**Files:**
- Create: `src/ai.rs`
- Modify: `src/main.rs` (add `mod ai;`)

**Interfaces:**
- Consumes: nothing project-specific.
- Produces:
  - `ai::AiFields { title: String, domains: Vec<String>, danger: String, explanation: String, tags: Vec<String>, undo: String, platform: Vec<String> }` (derives `Debug, PartialEq, serde::Deserialize`).
  - `ai::Backend { Api, Cli, Manual }` (enum).
  - `ai::select_backend(has_api_key: bool, claude_on_path: bool) -> Backend`.
  - `ai::parse_response(text: &str) -> Result<AiFields, String>` — extracts the first `{...}` JSON object from `text` (models sometimes wrap it in prose/fences) and deserializes.
  - `ai::populate(cmd: &str) -> Result<AiFields, String>` — resolves the backend and performs the call; `Backend::Manual` → `Err`. (Not unit-tested; the two helpers above are.)

- [ ] **Step 1: Write failing tests**

Create `src/ai.rs`:

```rust
use serde::Deserialize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_prefers_api_key() {
        assert_eq!(select_backend(true, true), Backend::Api);
        assert_eq!(select_backend(true, false), Backend::Api);
    }

    #[test]
    fn backend_falls_back_to_cli_then_manual() {
        assert_eq!(select_backend(false, true), Backend::Cli);
        assert_eq!(select_backend(false, false), Backend::Manual);
    }

    #[test]
    fn parses_clean_json() {
        let f = parse_response(r#"{"title":"T","domains":["shell"],"danger":"low","explanation":"E","tags":["a"],"undo":"","platform":["macos"]}"#).unwrap();
        assert_eq!(f.title, "T");
        assert_eq!(f.domains, vec!["shell"]);
        assert_eq!(f.danger, "low");
    }

    #[test]
    fn parses_json_wrapped_in_prose_or_fences() {
        let text = "Here you go:\n```json\n{\"title\":\"T\",\"domains\":[\"git\"],\"danger\":\"medium\",\"explanation\":\"E\",\"tags\":[\"x\"],\"undo\":\"\",\"platform\":[\"macos\"]}\n```\n";
        let f = parse_response(text).unwrap();
        assert_eq!(f.danger, "medium");
        assert_eq!(f.domains, vec!["git"]);
    }

    #[test]
    fn malformed_json_errors() {
        assert!(parse_response("no json here").is_err());
        assert!(parse_response("{ not valid").is_err());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test ai::`
Expected: FAIL — `select_backend`/`parse_response`/`Backend`/`AiFields` not defined.

- [ ] **Step 3: Implement `src/ai.rs`**

Add above the tests:

```rust
#[derive(Debug, PartialEq, Deserialize)]
pub struct AiFields {
    pub title: String,
    pub domains: Vec<String>,
    pub danger: String,
    pub explanation: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub undo: String,
    pub platform: Vec<String>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Backend {
    Api,
    Cli,
    Manual,
}

pub fn select_backend(has_api_key: bool, claude_on_path: bool) -> Backend {
    if has_api_key {
        Backend::Api
    } else if claude_on_path {
        Backend::Cli
    } else {
        Backend::Manual
    }
}

/// Extract the first balanced {...} JSON object and deserialize it.
pub fn parse_response(text: &str) -> Result<AiFields, String> {
    let start = text.find('{').ok_or("no JSON object in response")?;
    let mut depth = 0usize;
    let mut end = None;
    for (i, c) in text[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end.ok_or("unterminated JSON object")?;
    serde_json::from_str(&text[start..end]).map_err(|e| format!("bad JSON: {e}"))
}

fn model() -> String {
    std::env::var("COLLECTIVE_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string())
}

fn prompt(cmd: &str) -> String {
    format!(
        "For the shell command below, return ONLY a JSON object with keys \
title (string, imperative), domains (array from: power, macos-admin, network, \
files, disk, debugging, security, shell, git, media), danger (\"low\"|\"medium\"|\"high\"; \
high=destructive/irreversible, medium=sudo/writes-system-state), explanation \
(2-3 sentences), tags (array of 3-5 keywords), undo (string, \"\" if none), \
platform (array, e.g. [\"macos\"] or [\"macos\",\"linux\"]). No prose.\n\nCommand: {cmd}"
    )
}

pub fn populate(cmd: &str) -> Result<AiFields, String> {
    let has_key = std::env::var("ANTHROPIC_API_KEY").map(|k| !k.is_empty()).unwrap_or(false);
    let claude = which_claude();
    match select_backend(has_key, claude.is_some()) {
        Backend::Api => populate_api(cmd),
        Backend::Cli => populate_cli(cmd, &claude.unwrap()),
        Backend::Manual => Err("no ANTHROPIC_API_KEY and no `claude` on PATH".to_string()),
    }
}

fn which_claude() -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    for dir in path.split(':') {
        let cand = std::path::Path::new(dir).join("claude");
        if cand.is_file() {
            return Some(cand.to_string_lossy().into_owned());
        }
    }
    None
}

fn populate_api(cmd: &str) -> Result<AiFields, String> {
    let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| "no API key")?;
    let body = serde_json::json!({
        "model": model(),
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": prompt(cmd)}]
    });
    let resp = ureq::post("https://api.anthropic.com/v1/messages")
        .set("x-api-key", &key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .send_json(body)
        .map_err(|e| format!("API request failed: {e}"))?;
    let v: serde_json::Value = resp.into_json().map_err(|e| format!("bad API response: {e}"))?;
    let text = v["content"][0]["text"].as_str().ok_or("no text in API response")?;
    parse_response(text)
}

fn populate_cli(cmd: &str, claude: &str) -> Result<AiFields, String> {
    let out = std::process::Command::new(claude)
        .args(["-p", &prompt(cmd), "--output-format", "json", "--model", &model()])
        .output()
        .map_err(|e| format!("claude invocation failed: {e}"))?;
    if !out.status.success() {
        return Err(format!("claude exited with {}", out.status));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    // claude --output-format json wraps the model text in {"result": "..."};
    // fall back to treating stdout as the text if that shape is absent.
    let text = match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(v) => v["result"].as_str().unwrap_or(&stdout).to_string(),
        Err(_) => stdout.to_string(),
    };
    parse_response(&text)
}
```

Add `mod ai;` to `src/main.rs`; mark `#[allow(dead_code)]` with comment `// consumed by collect (Task 6)`.

- [ ] **Step 4: Run tests + build**

Run: `cargo test ai:: && cargo build`
Expected: 5 ai tests pass; zero warnings.

- [ ] **Step 5: Commit**

```bash
git add src/ai.rs src/main.rs
git commit -m "feat: AI field population backend (API -> claude CLI -> manual)"
```

---

### Task 6: `collect` command

**Files:**
- Create: `src/collect.rs`
- Modify: `src/entry.rs` (`Danger::parse`), `src/main.rs` (`Collect` subcommand, remove ai `#[allow(dead_code)]`), `tests/cli.rs`

**Interfaces:**
- Consumes: `entry::{Entry, Danger}`, `ai`, `corpus::load`.
- Produces:
  - `entry::Danger::parse(s: &str) -> Option<Danger>` (`"low"|"medium"|"high"`).
  - `collect::slug(title: &str) -> String` — lowercase, non-alphanumeric → hyphen, collapse/trim hyphens.
  - `collect::uniquify(base: &str, existing: &std::collections::HashSet<String>) -> String` — append `-2`, `-3` … until unused.
  - `collect::write_entry(dir: &Path, e: &Entry) -> std::io::Result<PathBuf>` — writes `<dir>/<id>.yaml`.
  - `collect::run(cmd: &str, manual: bool)` — the interactive flow.
  - `main.rs`: `Cmd::Collect { command: String, #[arg(long)] manual: bool }`.

- [ ] **Step 1: Write failing tests**

Create `src/collect.rs`:

```rust
use crate::entry::{Danger, Entry};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_kebab() {
        assert_eq!(slug("Disable Sleep, Entirely!"), "disable-sleep-entirely");
        assert_eq!(slug("  git   reflog  "), "git-reflog");
    }

    #[test]
    fn uniquify_appends_suffix() {
        let mut existing = HashSet::new();
        existing.insert("git-reflog".to_string());
        existing.insert("git-reflog-2".to_string());
        assert_eq!(uniquify("git-reflog", &existing), "git-reflog-3");
        assert_eq!(uniquify("fresh", &existing), "fresh");
    }

    #[test]
    fn assembled_entry_validates_and_roundtrips() {
        let e = Entry {
            id: "test-entry".into(),
            title: "Test entry".into(),
            cmd: "echo hi".into(),
            undo: None,
            platform: vec!["macos".into()],
            domains: vec!["shell".into()],
            danger: Danger::Low,
            explanation: "Prints hi.".into(),
            source: "collect:testhost".into(),
            tags: vec!["echo".into()],
        };
        assert!(e.validate().is_ok());
        let dir = std::env::temp_dir().join("col-collect-test");
        let _ = fs::remove_dir_all(&dir);
        let path = write_entry(&dir, &e).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let back: Entry = serde_yaml::from_str(&text).unwrap();
        assert_eq!(back.id, e.id);
        assert_eq!(back.danger, Danger::Low);
        assert_eq!(back.cmd, "echo hi");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn danger_parses() {
        assert_eq!(Danger::parse("high"), Some(Danger::High));
        assert_eq!(Danger::parse("bogus"), None);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test collect:: && cargo test entry::`
Expected: FAIL — `slug`/`uniquify`/`write_entry`/`Danger::parse` not defined.

- [ ] **Step 3: Implement `Danger::parse` in `src/entry.rs`**

Add to the `impl` area of `src/entry.rs` (after `validate`), inside `impl Danger` (create the block):

```rust
impl Danger {
    pub fn parse(s: &str) -> Option<Danger> {
        match s.trim().to_lowercase().as_str() {
            "low" => Some(Danger::Low),
            "medium" => Some(Danger::Medium),
            "high" => Some(Danger::High),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Implement slug/uniquify/write_entry in `src/collect.rs`**

Add above the tests:

```rust
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut prev_hyphen = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_hyphen = false;
        } else if !prev_hyphen {
            out.push('-');
            prev_hyphen = true;
        }
    }
    out.trim_matches('-').to_string()
}

pub fn uniquify(base: &str, existing: &HashSet<String>) -> String {
    if !existing.contains(base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !existing.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

pub fn write_entry(dir: &Path, e: &Entry) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.yaml", e.id));
    fs::write(&path, serde_yaml::to_string(e).expect("entry serializes"))?;
    Ok(path)
}
```

- [ ] **Step 5: Implement the interactive `run` flow**

Append to `src/collect.rs`:

```rust
use crate::ai;
use std::io::{self, Write};

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn ask(prompt: &str, default: &str) -> String {
    print!("{prompt}");
    if !default.is_empty() {
        print!(" [{default}]");
    }
    print!(": ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
        return default.to_string();
    }
    let t = line.trim();
    if t.is_empty() { default.to_string() } else { t.to_string() }
}

fn csv(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// Build an Entry from AI-populated fields.
fn from_ai(cmd: &str, f: ai::AiFields) -> Entry {
    Entry {
        id: String::new(), // filled by caller after uniquify
        title: f.title,
        cmd: cmd.to_string(),
        undo: (!f.undo.is_empty()).then_some(f.undo),
        platform: if f.platform.is_empty() { vec!["macos".into()] } else { f.platform },
        domains: if f.domains.is_empty() { vec!["shell".into()] } else { f.domains },
        danger: Danger::parse(&f.danger).unwrap_or(Danger::Low),
        explanation: f.explanation,
        source: format!("collect:{}", hostname()),
        tags: f.tags,
    }
}

fn from_manual(cmd: &str) -> Entry {
    let title = ask("title", "");
    let explanation = ask("explanation", "");
    let domains = csv(&ask("domains (comma-sep)", "shell"));
    let danger = loop {
        match Danger::parse(&ask("danger (low/medium/high)", "low")) {
            Some(d) => break d,
            None => println!("  must be low, medium, or high"),
        }
    };
    let tags = csv(&ask("tags (comma-sep)", ""));
    let undo = ask("undo command", "");
    let platform = csv(&ask("platform (comma-sep)", "macos"));
    Entry {
        id: String::new(),
        title,
        cmd: cmd.to_string(),
        undo: (!undo.is_empty()).then_some(undo),
        platform,
        domains,
        danger,
        explanation,
        source: format!("collect:{}", hostname()),
        tags,
    }
}

pub fn run(cmd: &str, manual: bool) {
    let use_ai = if manual {
        false
    } else {
        matches!(ask("Populate with AI, or fill in manually? [a/m]", "a").to_lowercase().as_str(), "a" | "ai" | "")
    };

    let mut entry = if use_ai {
        match ai::populate(cmd) {
            Ok(fields) => from_ai(cmd, fields),
            Err(e) => {
                eprintln!("AI populate failed ({e}); falling back to manual.");
                from_manual(cmd)
            }
        }
    } else {
        from_manual(cmd)
    };

    // assign a unique id from existing corpus + overlay
    let existing: HashSet<String> = crate::corpus::load().into_iter().map(|e| e.id).collect();
    entry.id = uniquify(&slug(&entry.title), &existing);

    if let Err(e) = entry.validate() {
        eprintln!("cannot save: {e}");
        std::process::exit(1);
    }
    let dir = crate::favorites::default_path()
        .parent()
        .expect("home has parent")
        .join("corpus");
    match write_entry(&dir, &entry) {
        Ok(path) => println!("saved {} -> {}", entry.id, path.display()),
        Err(e) => {
            eprintln!("write failed: {e}");
            std::process::exit(1);
        }
    }
}
```

(`from_ai` defaults an unparseable AI danger to `Danger::Low` — see the `.unwrap_or(Danger::Low)` above.)

- [ ] **Step 6: Wire the subcommand in `src/main.rs`**

Add `mod collect;` (and remove the `#[allow(dead_code)]` on `mod ai;`, now reached via collect). Add the variant:

```rust
    /// Capture a command into your personal corpus (overlay)
    Collect {
        /// The command to save
        command: String,
        /// Skip the AI prompt and enter fields manually
        #[arg(long)]
        manual: bool,
    },
```

Add the match arm:

```rust
        Some(Cmd::Collect { command, manual }) => collect::run(&command, manual),
```

- [ ] **Step 7: Add integration test for collect --manual**

Add to `tests/cli.rs`:

```rust
#[test]
fn collect_manual_writes_overlay_file() {
    let home = std::env::temp_dir().join(format!("col-collect-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    // manual answers piped in field order: title, explanation, domains, danger, tags, undo, platform
    let stdin = "My Test Cmd\nDoes a thing.\nshell\nlow\nfoo,bar\n\nmacos\n";
    Command::cargo_bin("collective")
        .unwrap()
        .args(["collect", "echo hello", "--manual"])
        .env("HOME", &home)
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicates::str::contains("saved my-test-cmd"));
    let f = home.join(".collective/corpus/my-test-cmd.yaml");
    assert!(f.exists(), "overlay file not written");
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(text.contains("cmd: echo hello"));
    let _ = std::fs::remove_dir_all(&home);
}
```

Note: this relies on `directories::BaseDirs` honoring `HOME`; on macOS/Linux it does. If the test proves flaky on the runner, keep it but document the dependency.

- [ ] **Step 8: Run all tests + build**

Run: `cargo test && cargo build`
Expected: all pass (favorites, tui, ai, collect, entry units + cli integration); zero warnings.

- [ ] **Step 9: Manual AI-path smoke (optional, needs claude or key)**

```bash
cargo run -q -- collect 'pmset -a disablesleep 1'   # choose 'a'; review fields; saved to overlay
cargo run -q -- search disablesleep                  # the collected entry now appears
```

- [ ] **Step 10: Commit**

```bash
git add src/collect.rs src/entry.rs src/main.rs tests/cli.rs
git commit -m "feat: collect command — capture commands into overlay via AI or manual"
```

---

## Done Criteria

- `cargo test` green (favorites, tui App, ai, collect, entry units + cli integration); zero warnings.
- Bare `collective` opens the TUI: live filter, full-command detail pane, star favorites persisted to `~/.collective/favorites.json`, `Enter` prefills the command (via wrapper) or copies+prints (bare), terminal restored on every exit including panic.
- `collective --print-shell zsh|bash` emits a working wrapper.
- `collective collect '<cmd>'` writes a schema-valid entry to `~/.collective/corpus/`, AI-populated (API → `claude` → manual fallback) or manual, and it appears in subsequent searches.
- Existing subcommands unchanged.
