# Collective v3 (frictionless loop) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add four small features to `collective` — placeholder filling, search/TUI filters, `collect --last`, and shell completions — each reusing existing seams.

**Architecture:** Thin additions over existing modules. One new pure module (`placeholder`). Everything else extends `main.rs` dispatch, `collect.rs`, `search.rs`, `tui/`, and the shell wrappers. No new subsystems.

**Tech Stack:** Rust, clap 4 (+ clap_complete), ratatui/crossterm, serde/serde_yaml, arboard.

## Global Constraints

- Binary `collective`, crate `collective`. Zero-warning build. Conventional commits. Commit after each task.
- No in-app execution, no plugins, no config file, no theming (anti-bloat).
- Reuse `search::is_bulk_import` for the curated predicate — do not duplicate it.
- Existing commands/TUI behavior otherwise unchanged.
- Tests never make live AI calls, drive the terminal, or touch the real clipboard.

## Current signatures (verified, build on these exactly)

- `search::search<'a>(entries: &'a [Entry], query: &str) -> Vec<(&'a Entry, u32)>`
- `fn is_bulk_import(e: &Entry) -> bool` in `src/search.rs` (currently private).
- `collect::run(cmd: &str, manual: bool)`; helpers `from_ai(cmd,&AiFields)->Entry`, `from_manual(cmd)->Entry`, `ask(prompt,default)->String`.
- `tui::App` fields include `filter, filtered, selected, favorites, fav_only`; `fn recompute(&mut self)`, `toggle_fav_only`. Event loop in `tui::run` handles keys `y`/`f`/`F`/`q`/chars; `deliver(cmd)` runs after `restore()` on the picked command.
- `main.rs`: `Cli { print_shell: Option<String>, cmd: Option<Cmd> }`; `Cmd::Search { query }`, `Cmd::Collect { command, manual }`; `cmd_search(&[Entry], &str)`, `cmd_copy(&[Entry], &str)`.

## File Structure

```
src/placeholder.rs   # NEW: tokens(), fill(), fill_interactive()
src/main.rs          # mod placeholder; search --domain/--curated; completions; collect --last dispatch; cmd_copy fill
src/search.rs        # is_bulk_import -> pub(crate)
src/tui/mod.rs       # App.curated_only + toggle + recompute + `c` key; fill on deliver
src/tui/ui.rs        # help bar adds "c curated"
src/collect.rs       # run(Option<String>, manual, last)
shell/collective.zsh # last-command capture
shell/collective.bash
Cargo.toml           # clap_complete
tests/cli.rs         # search filters, collect --last, completions
```

---

### Task 1: Placeholder filling

**Files:**
- Create: `src/placeholder.rs`
- Modify: `src/main.rs` (`mod placeholder;`, `cmd_copy`), `src/tui/mod.rs` (deliver path)

**Interfaces:**
- Produces:
  - `placeholder::tokens(cmd: &str) -> Vec<String>` — inner names of `<...>` tokens, unique, first-seen order (e.g. `"lsof -iTCP:<port> <host>"` → `["port","host"]`).
  - `placeholder::fill(cmd: &str, answers: &[(String, String)]) -> String` — replace `<name>` with its answer; empty answer leaves `<name>` in place.
  - `placeholder::fill_interactive(cmd: &str) -> String` — no tokens → unchanged; else prompt each `<name>: ` on stdin and substitute (stdin, not unit-tested).

- [ ] **Step 1: Write failing tests in `src/placeholder.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_tokens() {
        assert!(tokens("git status").is_empty());
    }

    #[test]
    fn one_token() {
        assert_eq!(tokens("lsof -iTCP:<port>"), vec!["port"]);
    }

    #[test]
    fn repeated_token_collapses() {
        assert_eq!(tokens("cp <file> <file>.bak"), vec!["file"]);
    }

    #[test]
    fn multiple_distinct_tokens_in_order() {
        assert_eq!(tokens("scp <src> <host>:<dest>"), vec!["src", "host", "dest"]);
    }

    #[test]
    fn fill_substitutes_and_leaves_empty() {
        let cmd = "lsof -iTCP:<port> <host>";
        let answers = vec![("port".to_string(), "3000".to_string()), ("host".to_string(), String::new())];
        assert_eq!(fill(cmd, &answers), "lsof -iTCP:3000 <host>");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test placeholder`
Expected: FAIL — `tokens`/`fill` not defined.

- [ ] **Step 3: Implement `src/placeholder.rs`**

```rust
use std::io::{self, Write};

/// Inner names of `<...>` tokens, unique, first-seen order.
pub fn tokens(cmd: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = cmd.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(rel) = cmd[i + 1..].find('>') {
                let name = &cmd[i + 1..i + 1 + rel];
                if !name.is_empty() && !name.contains('<') && !out.iter().any(|t| t == name) {
                    out.push(name.to_string());
                }
                i = i + 1 + rel + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Replace `<name>` with its answer. Empty answer leaves the token in place.
pub fn fill(cmd: &str, answers: &[(String, String)]) -> String {
    let mut out = cmd.to_string();
    for (name, ans) in answers {
        if !ans.is_empty() {
            out = out.replace(&format!("<{name}>"), ans);
        }
    }
    out
}

/// Prompt for each token on stdin and substitute. No tokens -> unchanged.
pub fn fill_interactive(cmd: &str) -> String {
    let toks = tokens(cmd);
    if toks.is_empty() {
        return cmd.to_string();
    }
    let mut answers = Vec::new();
    for t in &toks {
        print!("<{t}>: ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        answers.push((t.clone(), line.trim().to_string()));
    }
    fill(cmd, &answers)
}
```

Add `mod placeholder;` to `src/main.rs` (with the other `mod` lines).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test placeholder`
Expected: 5 passed.

- [ ] **Step 5: Wire into the TUI deliver path (`src/tui/mod.rs`)**

Find the tail of `run()`:

```rust
    if let Some(cmd) = picked {
        deliver(&cmd);
    }
    Ok(())
```

Replace with (fill runs in cooked mode, after `restore()`):

```rust
    if let Some(cmd) = picked {
        let cmd = crate::placeholder::fill_interactive(&cmd);
        deliver(&cmd);
    }
    Ok(())
```

- [ ] **Step 6: Wire into `cmd_copy` (`src/main.rs`)**

Replace the body of `cmd_copy`:

```rust
fn cmd_copy(entries: &[entry::Entry], id: &str) {
    let e = find(entries, id);
    let cmd = placeholder::fill_interactive(&e.cmd);
    match arboard::Clipboard::new().and_then(|mut c| c.set_text(cmd.clone())) {
        Ok(()) => println!("copied: {cmd}"),
        Err(err) => {
            eprintln!("clipboard failed ({err}); here it is:\n{cmd}");
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 7: Build + full test**

Run: `cargo build && cargo test`
Expected: zero warnings; all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/placeholder.rs src/main.rs src/tui/mod.rs
git commit -m "feat: fill <placeholder> tokens before prefill/copy"
```

---

### Task 2: Search + TUI filters

**Files:**
- Modify: `src/search.rs` (visibility), `src/main.rs` (`Search` flags, `cmd_search`), `src/tui/mod.rs` (`curated_only`), `src/tui/ui.rs` (help bar), `tests/cli.rs`

**Interfaces:**
- Consumes: `search::is_bulk_import`.
- Produces:
  - `search::is_bulk_import(e: &Entry) -> bool` is `pub(crate)`.
  - `cmd_search(entries: &[Entry], query: &str, domain: Option<&str>, curated: bool)`.
  - `App.curated_only: bool` + `App::toggle_curated_only(&mut self)`; recompute drops bulk imports when set.

- [ ] **Step 1: Make `is_bulk_import` crate-visible (`src/search.rs`)**

Change:

```rust
fn is_bulk_import(e: &Entry) -> bool {
```
to
```rust
pub(crate) fn is_bulk_import(e: &Entry) -> bool {
```

- [ ] **Step 2: Write failing App test in `src/tui/mod.rs` tests module**

```rust
    #[test]
    fn curated_only_hides_bulk_imports() {
        let mut a = App::new(corpus::load(), std::collections::HashSet::new());
        let before = a.visible().len();
        a.toggle_curated_only();
        let after = a.visible().len();
        assert!(after < before, "curated view should drop bulk imports");
        assert!(a.visible().iter().all(|e| !crate::search::is_bulk_import(e)));
    }
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test tui::`
Expected: FAIL — `toggle_curated_only` not defined.

- [ ] **Step 4: Add `curated_only` to `App` (`src/tui/mod.rs`)**

Add the field to the struct:

```rust
    pub curated_only: bool,
```

Initialize it in `App::new` (alongside `fav_only: false,`):

```rust
            curated_only: false,
```

In `recompute()`, after the existing `fav_only` retain block, add:

```rust
        if self.curated_only {
            idx.retain(|&i| !crate::search::is_bulk_import(&self.all[i]));
        }
```

Add the toggle method (next to `toggle_fav_only`):

```rust
    pub fn toggle_curated_only(&mut self) {
        self.curated_only = !self.curated_only;
        self.recompute();
    }
```

- [ ] **Step 5: Run App test**

Run: `cargo test tui::`
Expected: pass.

- [ ] **Step 6: Wire the `c` key + help bar**

In `src/tui/mod.rs` event loop, next to `KeyCode::Char('F') => app.toggle_fav_only(),` add:

```rust
                KeyCode::Char('c') => app.toggle_curated_only(),
```

In `src/tui/ui.rs`, update the help bar text to include curated:

```rust
    let help = Paragraph::new("↵ prefill  y copy  f ★  F fav-only  c curated  type to filter  q quit")
```

(Replace the existing help `Paragraph::new(...)` string; keep the surrounding style/render lines.)

- [ ] **Step 7: Add `--domain`/`--curated` to `Search` (`src/main.rs`)**

Change the `Search` variant:

```rust
    /// Fuzzy-search the corpus
    Search {
        query: Vec<String>,
        /// Only entries in this domain (e.g. git, network)
        #[arg(long)]
        domain: Option<String>,
        /// Exclude bulk tldr imports; curated entries only
        #[arg(long)]
        curated: bool,
    },
```

Update the dispatch arm:

```rust
        Some(Cmd::Search { query, domain, curated }) => {
            cmd_search(&entries, &query.join(" "), domain.as_deref(), curated)
        }
```

Rewrite `cmd_search` to filter before ranking:

```rust
fn cmd_search(entries: &[entry::Entry], query: &str, domain: Option<&str>, curated: bool) {
    let filtered: Vec<entry::Entry> = entries
        .iter()
        .filter(|e| domain.is_none_or(|d| e.domains.iter().any(|x| x == d)))
        .filter(|e| !curated || !search::is_bulk_import(e))
        .cloned()
        .collect();
    let hits = search::search(&filtered, query);
    if hits.is_empty() {
        eprintln!("no matches for '{query}'");
        std::process::exit(1);
    }
    for (e, _) in hits {
        let preview: String = e.cmd.chars().take(48).collect();
        println!("{:<28} {:<44} {}", e.id, truncate(&e.title, 44), preview);
    }
}
```

(`is_none_or` needs Rust 1.82+, already used elsewhere in this crate. `Entry` derives `Clone`.)

- [ ] **Step 8: Write integration tests in `tests/cli.rs`**

```rust
#[test]
fn search_curated_excludes_tldr_imports() {
    let out = Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "git", "--curated"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("tldr-"), "curated search leaked a tldr import:\n{stdout}");
}

#[test]
fn search_domain_filters_to_domain() {
    // "network" domain entries exist among curated gems (e.g. flush-dns-cache).
    Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "dns", "--domain", "network"])
        .assert()
        .success();
}
```

- [ ] **Step 9: Build + full test**

Run: `cargo build && cargo test`
Expected: zero warnings; all pass.

- [ ] **Step 10: Commit**

```bash
git add src/search.rs src/main.rs src/tui/mod.rs src/tui/ui.rs tests/cli.rs
git commit -m "feat: --domain/--curated search filters and TUI curated toggle"
```

---

### Task 3: `collect --last`

**Files:**
- Modify: `src/collect.rs` (`run` signature + resolution), `src/main.rs` (`Collect` variant + dispatch), `shell/collective.zsh`, `shell/collective.bash`, `tests/cli.rs`

**Interfaces:**
- Produces: `collect::run(command: Option<String>, manual: bool, last: bool)`.
- The shell wrappers export `COLLECTIVE_LAST_CMD` when the invocation is `collect --last`.

- [ ] **Step 1: Change `collect::run` (`src/collect.rs`)**

Replace the `run` signature and its command-resolution preamble. Current:

```rust
pub fn run(cmd: &str, manual: bool) {
    let use_ai = if manual {
```

New — resolve the command first, then keep the existing body using a `cmd` binding:

```rust
pub fn run(command: Option<String>, manual: bool, last: bool) {
    let cmd: String = if last {
        match std::env::var("COLLECTIVE_LAST_CMD") {
            Ok(c) if !c.trim().is_empty() => c.trim().to_string(),
            _ => {
                eprintln!("--last needs the shell wrapper — run 'collective --print-shell <shell>' and reload, or pass the command explicitly");
                std::process::exit(1);
            }
        }
    } else {
        match command {
            Some(c) if !c.trim().is_empty() => c,
            _ => {
                eprintln!("nothing to collect — pass a command or use --last");
                std::process::exit(1);
            }
        }
    };
    let cmd = cmd.as_str();
    let use_ai = if manual {
```

(The rest of `run` already uses `cmd`; leave it unchanged.)

- [ ] **Step 2: Update the `Collect` variant + dispatch (`src/main.rs`)**

Change the variant:

```rust
    /// Capture a command into your personal corpus (overlay)
    Collect {
        /// The command to save (optional with --last)
        command: Option<String>,
        /// Skip the AI prompt and enter fields manually
        #[arg(long)]
        manual: bool,
        /// Capture the previous shell command (needs the shell wrapper)
        #[arg(long)]
        last: bool,
    },
```

Update the dispatch arm:

```rust
        Some(Cmd::Collect { command, manual, last }) => collect::run(command, manual, last),
```

- [ ] **Step 3: Update the shell wrappers**

`shell/collective.zsh`:

```zsh
collective() {
  local last=""
  if [[ "$1" == "collect" && " ${*} " == *" --last "* ]]; then
    last="$(fc -ln -1)"
    last="${last#"${last%%[![:space:]]*}"}"
  fi
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" COLLECTIVE_LAST_CMD="$last" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && print -z "$cmd"
}
```

`shell/collective.bash`:

```bash
collective() {
  local last=""
  if [[ "$1" == "collect" && " ${*} " == *" --last "* ]]; then
    last="$(history 1 | sed 's/^ *[0-9]* *//')"
  fi
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" COLLECTIVE_LAST_CMD="$last" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && { READLINE_LINE="$cmd"; READLINE_POINT=${#cmd}; }
}
```

- [ ] **Step 4: Write integration tests in `tests/cli.rs`**

```rust
#[test]
fn collect_last_reads_env_and_writes_overlay() {
    let home = std::env::temp_dir().join(format!("col-last-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    // manual answers: title, explanation, domains, danger, tags, undo, platform
    let stdin = "Grab Last\nCaptured from history.\nshell\nlow\nhistory\n\nmacos\n";
    Command::cargo_bin("collective")
        .unwrap()
        .args(["collect", "--last", "--manual"])
        .env("HOME", &home)
        .env("COLLECTIVE_LAST_CMD", "echo captured")
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicates::str::contains("saved grab-last"));
    let f = home.join(".collective/corpus/grab-last.yaml");
    assert!(f.exists());
    assert!(std::fs::read_to_string(&f).unwrap().contains("cmd: echo captured"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn collect_last_without_env_errors() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["collect", "--last"])
        .env_remove("COLLECTIVE_LAST_CMD")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicates::str::contains("--last needs the shell wrapper"));
}
```

- [ ] **Step 5: Build + full test**

Run: `cargo build && cargo test`
Expected: zero warnings; all pass.

- [ ] **Step 6: Commit**

```bash
git add src/collect.rs src/main.rs shell tests/cli.rs
git commit -m "feat: collect --last captures the previous shell command"
```

---

### Task 4: Shell completions

**Files:**
- Modify: `Cargo.toml`, `src/main.rs`, `README.md`, `tests/cli.rs`

**Interfaces:**
- Produces: `collective completions <zsh|bash|fish>` prints a completion script and exits.

- [ ] **Step 1: Add the dependency (`Cargo.toml`)**

In `[dependencies]`:

```toml
clap_complete = "4"
```

- [ ] **Step 2: Write failing integration test (`tests/cli.rs`)**

```rust
#[test]
fn completions_zsh_emits_script() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicates::str::contains("_collective"));
}

#[test]
fn completions_unknown_shell_errors() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["completions", "tcsh"])
        .assert()
        .failure();
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --test cli completions`
Expected: FAIL — `completions` unknown subcommand.

- [ ] **Step 4: Implement the subcommand (`src/main.rs`)**

Add imports near the top:

```rust
use clap::CommandFactory;
use clap_complete::{generate, Shell};
```

Add the `Completions` variant to `enum Cmd`:

```rust
    /// Print a shell completion script (zsh, bash, fish)
    Completions {
        /// Target shell
        shell: String,
    },
```

Add the dispatch arm (it needs no corpus; handle it directly):

```rust
        Some(Cmd::Completions { shell }) => {
            let parsed: Shell = match shell.parse() {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("unknown shell '{shell}' (use zsh, bash, or fish)");
                    std::process::exit(1);
                }
            };
            generate(parsed, &mut Cli::command(), "collective", &mut std::io::stdout());
        }
```

(`Shell` implements `FromStr` for zsh/bash/fish/etc.; this loads corpus needlessly only if placed after `corpus::load()` — it's fine functionally, but prefer placing the arm so it doesn't depend on `entries`. It doesn't use `entries`, so leaving it in the match is correct; corpus load already happened but is harmless. YAGNI: no early-exit optimization needed.)

- [ ] **Step 5: Run completions tests**

Run: `cargo test --test cli completions`
Expected: both pass.

- [ ] **Step 6: Document install (`README.md`)**

Add a subsection under Install:

```markdown
### Shell completions

```sh
collective completions zsh > ~/.zfunc/_collective   # ensure ~/.zfunc is in $fpath
collective completions bash > /usr/local/etc/bash_completion.d/collective
collective completions fish > ~/.config/fish/completions/collective.fish
```
```

- [ ] **Step 7: Build + full test**

Run: `cargo build && cargo test`
Expected: zero warnings; all pass.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs README.md tests/cli.rs
git commit -m "feat: shell completions via clap_complete"
```

---

## Done Criteria

- `cargo test` green, zero warnings.
- A `<placeholder>` command prompts to fill before prefill (TUI) and `copy`; token-free commands unaffected.
- `search --domain git --curated` returns only curated git entries; TUI `c` toggles curated-only.
- `collect --last` (env/wrapper) captures the prior command; clear error without it.
- `collective completions zsh` emits a valid script; unknown shell errors.
- All existing commands and TUI behavior otherwise unchanged.
