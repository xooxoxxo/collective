# Collective CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `col` — a Rust CLI script directory (search/show/copy/random) with SM-2 flashcard drills, plus a two-track corpus harvest (tldr import + multi-agent gem mining).

**Architecture:** Single Rust binary. Corpus = YAML files in `corpus/`, validated at build time by `build.rs` (bad entry = build failure), embedded via `include_dir!`, merged with a user overlay at `~/.collective/corpus/`. No DB, no async — everything loads in memory. Drill state is one JSON file.

**Tech Stack:** Rust 2021, clap 4 (derive), serde/serde_yaml/serde_json, include_dir, nucleo-matcher, arboard, directories, rand. Dev: assert_cmd. Corpus tooling: Python 3 + PyYAML.

## Global Constraints

- Binary name is `col`, crate name `collective`.
- Spec: `docs/superpowers/specs/2026-07-13-collective-cli-design.md` — schema fields and command behavior come from there verbatim.
- `col` NEVER executes a corpus command. Show/copy only.
- Corpus entry ids: lowercase ascii letters, digits, hyphens only. Unique across corpus.
- `danger: high` entries render a red ANSI banner and print `undo` BEFORE the cmd.
- Drill state path: `~/.collective/drill.json`. Corrupt/missing state → warn + reset, never crash.
- Deviation from spec crate list (approved rationale): drill v1 uses plain stdio instead of `crossterm` — stdlib covers it; add crossterm only when a real TUI is built.
- Commit after every task. Conventional commit messages (`feat:`, `test:`, `chore:`).
- Every entry keeps `source` provenance; tldr imports carry `(CC-BY-4.0)` attribution in `source`.

## File Structure

```
Cargo.toml
build.rs                 # build-time corpus validation
corpus/*.yaml            # hand-curated entries (one file per entry)
corpus/imported/*.yaml   # Track 1 tldr imports (Task 8)
src/main.rs              # clap dispatch + search/show/copy/random commands
src/entry.rs             # Entry struct + Danger enum + validate()  (shared with build.rs)
src/corpus.rs            # embedded + overlay loading
src/search.rs            # weighted fuzzy search
src/sm2.rs               # SM-2 algorithm (pure)
src/drill.rs             # drill state persistence + session loop
tests/cli.rs             # integration test via assert_cmd
tools/import_tldr.py     # Track 1 converter
```

---

### Task 1: Scaffold, Entry schema, build-time validation, seed corpus

**Files:**
- Create: `Cargo.toml`, `build.rs`, `src/main.rs`, `src/entry.rs`, `corpus/*.yaml` (10 seed entries), `.gitignore`

**Interfaces:**
- Produces: `entry::Entry` (all fields public, see code), `entry::Danger` enum (`Low|Medium|High`), `Entry::validate(&self) -> Result<(), String>`. Later tasks consume these exactly.

- [ ] **Step 1: Scaffold cargo project**

```bash
cd /Users/oeyucel/Workspace/projects/collective
cargo init --name collective
```

Create `.gitignore`:

```
/target
```

Replace `Cargo.toml`:

```toml
[package]
name = "collective"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "col"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1"
include_dir = "0.7"
nucleo-matcher = "0.3"
arboard = "3"
directories = "5"
rand = "0.8"

[build-dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"

[dev-dependencies]
assert_cmd = "2"
```

- [ ] **Step 2: Write failing test for Entry parsing + validation**

Create `src/entry.rs` with ONLY the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
id: pmset-disable-sleep
title: Disable sleep entirely on macOS
cmd: sudo pmset -a disablesleep 1
undo: sudo pmset -a disablesleep 0
platform: [macos]
domains: [power]
danger: medium
explanation: Hard-disables sleep even with lid closed.
source: https://ss64.com/mac/pmset.html
tags: [sleep, clamshell]
"#;

    #[test]
    fn parses_valid_entry() {
        let e: Entry = serde_yaml::from_str(GOOD).unwrap();
        assert_eq!(e.id, "pmset-disable-sleep");
        assert_eq!(e.danger, Danger::Medium);
        assert_eq!(e.undo.as_deref(), Some("sudo pmset -a disablesleep 0"));
        assert!(e.validate().is_ok());
    }

    #[test]
    fn rejects_bad_id_chars() {
        let e: Entry = serde_yaml::from_str(&GOOD.replace("pmset-disable-sleep", "Bad_ID!")).unwrap();
        assert!(e.validate().is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let bad = format!("{GOOD}\nbogus_field: 1");
        assert!(serde_yaml::from_str::<Entry>(&bad).is_err());
    }

    #[test]
    fn rejects_empty_cmd() {
        let e: Entry = serde_yaml::from_str(&GOOD.replace("cmd: sudo pmset -a disablesleep 1", "cmd: \"\"")).unwrap();
        assert!(e.validate().is_err());
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test entry`
Expected: FAIL — `Entry` / `Danger` not defined (compile error).

- [ ] **Step 4: Implement Entry above the tests in `src/entry.rs`**

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub cmd: String,
    #[serde(default)]
    pub undo: Option<String>,
    pub platform: Vec<String>,
    pub domains: Vec<String>,
    pub danger: Danger,
    pub explanation: String,
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Danger {
    Low,
    Medium,
    High,
}

impl Entry {
    pub fn validate(&self) -> Result<(), String> {
        let id_ok = !self.id.is_empty()
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_ok {
            return Err(format!("bad id {:?}: use lowercase/digits/hyphens", self.id));
        }
        if self.title.trim().is_empty() {
            return Err(format!("{}: empty title", self.id));
        }
        if self.cmd.trim().is_empty() {
            return Err(format!("{}: empty cmd", self.id));
        }
        if self.explanation.trim().is_empty() {
            return Err(format!("{}: empty explanation", self.id));
        }
        if self.platform.is_empty() {
            return Err(format!("{}: platform required", self.id));
        }
        if self.domains.is_empty() {
            return Err(format!("{}: at least one domain", self.id));
        }
        Ok(())
    }
}
```

Replace `src/main.rs` (temporary stub so the crate compiles):

```rust
mod entry;

fn main() {
    println!("col v0 — commands land in later tasks");
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test entry`
Expected: 4 passed.

- [ ] **Step 6: Write 10 seed corpus entries**

Create `corpus/` with one file per entry. All 10, verbatim:

`corpus/pmset-disable-sleep.yaml`
```yaml
id: pmset-disable-sleep
title: Disable sleep entirely on macOS (even lid closed)
cmd: sudo pmset -a disablesleep 1
undo: sudo pmset -a disablesleep 0
platform: [macos]
domains: [power, macos-admin]
danger: medium
explanation: >
  Hard-disables system sleep on all power sources, including clamshell mode.
  Great for long builds on a closed laptop; remember to undo or your battery pays.
source: https://ss64.com/mac/pmset.html
tags: [sleep, clamshell, laptop, power]
```

`corpus/caffeinate-while-running.yaml`
```yaml
id: caffeinate-while-running
title: Keep Mac awake only while a command runs
cmd: caffeinate -dims make build
undo: ""
platform: [macos]
domains: [power]
danger: low
explanation: >
  Wraps any command; Mac stays awake (display, idle, disk, system) exactly until
  it exits. Safer than pmset because it cannot be forgotten.
source: https://ss64.com/mac/caffeinate.html
tags: [sleep, awake, build]
```

`corpus/screenshot-location.yaml`
```yaml
id: screenshot-location
title: Change where macOS saves screenshots
cmd: defaults write com.apple.screencapture location ~/Screenshots && killall SystemUIServer
undo: defaults delete com.apple.screencapture location && killall SystemUIServer
platform: [macos]
domains: [macos-admin]
danger: low
explanation: >
  Stops screenshots from littering the Desktop. Folder must exist first
  (mkdir -p ~/Screenshots).
source: https://macos-defaults.com/screenshots/location.html
tags: [screenshot, defaults, desktop]
```

`corpus/dock-autohide-instant.yaml`
```yaml
id: dock-autohide-instant
title: Make Dock auto-hide animation instant
cmd: defaults write com.apple.dock autohide-time-modifier -float 0 && killall Dock
undo: defaults delete com.apple.dock autohide-time-modifier && killall Dock
platform: [macos]
domains: [macos-admin]
danger: low
explanation: >
  Removes the auto-hide animation delay so the Dock snaps in and out instantly.
source: https://macos-defaults.com/dock/autohide-time-modifier.html
tags: [dock, defaults, animation, speed]
```

`corpus/flush-dns-cache.yaml`
```yaml
id: flush-dns-cache
title: Flush the macOS DNS cache
cmd: sudo dscacheutil -flushcache && sudo killall -HUP mDNSResponder
undo: ""
platform: [macos]
domains: [network]
danger: low
explanation: >
  Clears stale DNS entries after switching VPNs, editing /etc/hosts, or fighting
  a "this site moved but my Mac disagrees" situation.
source: https://support.apple.com/en-us/101694
tags: [dns, network, cache, vpn]
```

`corpus/lsof-listening-port.yaml`
```yaml
id: lsof-listening-port
title: Find which process is listening on a port
cmd: lsof -iTCP:3000 -sTCP:LISTEN -n -P
undo: ""
platform: [macos, linux]
domains: [network, debugging]
danger: low
explanation: >
  Answers "who is squatting on my port" with PID and process name. -n -P skips
  slow DNS/port-name lookups. Swap 3000 for any port.
source: https://ss64.com/mac/lsof.html
tags: [port, lsof, process, debugging]
```

`corpus/remove-quarantine.yaml`
```yaml
id: remove-quarantine
title: Remove the quarantine flag from a downloaded app
cmd: xattr -dr com.apple.quarantine /Applications/SomeApp.app
undo: ""
platform: [macos]
domains: [macos-admin, security]
danger: medium
explanation: >
  Clears Gatekeeper's "downloaded from the internet" warning. Only run on
  software you actually trust — this bypasses a real safety check.
source: https://ss64.com/mac/xattr.html
tags: [gatekeeper, quarantine, xattr, unsigned]
```

`corpus/git-fixup-autosquash.yaml`
```yaml
id: git-fixup-autosquash
title: Fix an earlier commit without manual rebase surgery
cmd: git commit --fixup <sha> && git rebase --autosquash -i <sha>^
undo: git rebase --abort
platform: [macos, linux]
domains: [git]
danger: medium
explanation: >
  Stage the fix, mark it as belonging to <sha>, and autosquash melds it in place.
  Rewrites history — only on unpushed/unshared branches.
source: https://git-scm.com/docs/git-rebase#Documentation/git-rebase.txt---autosquash
tags: [git, rebase, fixup, history]
```

`corpus/mdfind-by-name.yaml`
```yaml
id: mdfind-by-name
title: Spotlight-search files from the terminal
cmd: mdfind -name invoice.pdf
undo: ""
platform: [macos]
domains: [files]
danger: low
explanation: >
  Uses the Spotlight index, so it is instant — no find(1) filesystem crawl.
  Drop -name to full-text search file contents instead.
source: https://ss64.com/mac/mdfind.html
tags: [spotlight, search, find, files]
```

`corpus/tmutil-thin-snapshots.yaml`
```yaml
id: tmutil-thin-snapshots
title: Reclaim disk space eaten by local Time Machine snapshots
cmd: tmutil thinlocalsnapshots / 999999999999 4
undo: ""
platform: [macos]
domains: [macos-admin, disk]
danger: medium
explanation: >
  "System Data" mysteriously huge? Local APFS snapshots are the usual culprit.
  This purges them aggressively (urgency 4). List first with
  tmutil listlocalsnapshots /.
source: https://ss64.com/mac/tmutil.html
tags: [disk, time-machine, snapshots, storage]
```

- [ ] **Step 7: Write `build.rs` (build-time validation)**

```rust
// Validates every corpus/*.yaml at build time. Bad entry = no binary.
#[path = "src/entry.rs"]
mod entry;

use std::{collections::HashSet, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=corpus");
    let mut ids = HashSet::new();
    let mut stack = vec![Path::new("corpus").to_path_buf()];
    while let Some(dir) = stack.pop() {
        for f in fs::read_dir(&dir).expect("corpus/ dir missing") {
            let p = f.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "yaml") {
                let text = fs::read_to_string(&p).unwrap();
                let e: entry::Entry = serde_yaml::from_str(&text)
                    .unwrap_or_else(|err| panic!("{}: {err}", p.display()));
                e.validate()
                    .unwrap_or_else(|err| panic!("{}: {err}", p.display()));
                assert!(ids.insert(e.id.clone()), "duplicate id: {}", e.id);
            }
        }
    }
}
```

Note: `build.rs` reuses `src/entry.rs` via `#[path]` — entry.rs must only depend on serde (it does). The `#[cfg(test)]` module inside is compiled out.

- [ ] **Step 8: Verify build validation works both ways**

Run: `cargo build`
Expected: succeeds.

Break it deliberately: change `danger: medium` to `danger: extreme` in `corpus/pmset-disable-sleep.yaml`, run `cargo build`.
Expected: build FAILS mentioning the file. Revert the change, `cargo build` green again.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "feat: entry schema, build-time corpus validation, 10 seed entries"
```

---

### Task 2: Corpus loading (embedded + user overlay)

**Files:**
- Create: `src/corpus.rs`
- Modify: `src/main.rs` (add `mod corpus;`)

**Interfaces:**
- Consumes: `entry::Entry`, `Entry::validate`
- Produces: `corpus::load() -> Vec<Entry>` (sorted by id, overlay entries from `~/.collective/corpus/` override embedded ones by id).

- [ ] **Step 1: Write failing tests in `src/corpus.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_corpus_loads_and_contains_seed() {
        let entries = embedded();
        assert!(entries.len() >= 10);
        assert!(entries.iter().any(|e| e.id == "pmset-disable-sleep"));
    }

    #[test]
    fn overlay_overrides_by_id() {
        let base = embedded();
        let n = base.len();
        let mut clone = base[0].clone();
        clone.title = "OVERRIDDEN".into();
        let merged = merge(base, vec![clone.clone()]);
        assert_eq!(merged.len(), n);
        assert_eq!(
            merged.iter().find(|e| e.id == clone.id).unwrap().title,
            "OVERRIDDEN"
        );
    }

    #[test]
    fn merged_is_sorted_by_id() {
        let merged = merge(embedded(), vec![]);
        let mut sorted = merged.clone();
        sorted.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(
            merged.iter().map(|e| &e.id).collect::<Vec<_>>(),
            sorted.iter().map(|e| &e.id).collect::<Vec<_>>()
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test corpus`
Expected: FAIL — `embedded` / `merge` not defined.

- [ ] **Step 3: Implement `src/corpus.rs`**

```rust
use crate::entry::Entry;
use include_dir::{include_dir, Dir};
use std::fs;

static EMBEDDED: Dir = include_dir!("$CARGO_MANIFEST_DIR/corpus");

/// All embedded entries. Panics never: build.rs already validated them.
fn embedded() -> Vec<Entry> {
    fn walk(dir: &Dir, out: &mut Vec<Entry>) {
        for f in dir.files() {
            if f.path().extension().is_some_and(|e| e == "yaml") {
                let text = f.contents_utf8().expect("corpus yaml is utf8");
                out.push(serde_yaml::from_str(text).expect("validated at build time"));
            }
        }
        for d in dir.dirs() {
            walk(d, out);
        }
    }
    let mut out = Vec::new();
    walk(&EMBEDDED, &mut out);
    out
}

/// User overlay: ~/.collective/corpus/*.yaml. Invalid entries warn + skip.
fn overlay() -> Vec<Entry> {
    let Some(base) = directories::BaseDirs::new() else { return vec![] };
    let dir = base.home_dir().join(".collective/corpus");
    let Ok(read) = fs::read_dir(&dir) else { return vec![] };
    read.filter_map(|f| f.ok())
        .map(|f| f.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .filter_map(|p| {
            let text = fs::read_to_string(&p).ok()?;
            match serde_yaml::from_str::<Entry>(&text).map_err(|e| e.to_string())
                .and_then(|e| e.validate().map(|_| e))
            {
                Ok(e) => Some(e),
                Err(err) => {
                    eprintln!("warning: skipping {}: {err}", p.display());
                    None
                }
            }
        })
        .collect()
}

fn merge(base: Vec<Entry>, over: Vec<Entry>) -> Vec<Entry> {
    let mut entries = base;
    for e in over {
        entries.retain(|x| x.id != e.id); // overlay wins
        entries.push(e);
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    entries
}

pub fn load() -> Vec<Entry> {
    merge(embedded(), overlay())
}
```

Add `mod corpus;` to `src/main.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test corpus`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: corpus loading with embedded data and user overlay"
```

---

### Task 3: Fuzzy search + `col search` + integration test

**Files:**
- Create: `src/search.rs`, `tests/cli.rs`
- Modify: `src/main.rs` (clap CLI + search command)

**Interfaces:**
- Consumes: `corpus::load()`, `entry::Entry`
- Produces: `search::search<'a>(entries: &'a [Entry], query: &str) -> Vec<(&'a Entry, u32)>` — descending score, max 10 results. CLI: `col search <query...>`.

- [ ] **Step 1: Write failing tests in `src/search.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus;

    #[test]
    fn finds_pmset_for_sleep_query() {
        let entries = corpus::load();
        let hits = search(&entries, "disable sleep");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].0.id, "pmset-disable-sleep");
    }

    #[test]
    fn title_outranks_cmd_only_match() {
        let entries = corpus::load();
        let hits = search(&entries, "screenshot");
        assert_eq!(hits[0].0.id, "screenshot-location");
    }

    #[test]
    fn caps_at_ten_results() {
        let entries = corpus::load();
        assert!(search(&entries, "a").len() <= 10);
    }

    #[test]
    fn no_match_returns_empty() {
        let entries = corpus::load();
        assert!(search(&entries, "zzqqxxnothing").is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test search`
Expected: FAIL — `search` not defined.

- [ ] **Step 3: Implement `src/search.rs`**

```rust
use crate::entry::Entry;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Weighted fuzzy search: title 3x, best tag 2x, cmd 1x. Top 10, best first.
pub fn search<'a>(entries: &'a [Entry], query: &str) -> Vec<(&'a Entry, u32)> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buf = Vec::new();
    let mut score_of = |text: &str, matcher: &mut Matcher| -> u32 {
        buf.clear();
        let hay = Utf32Str::new(text, &mut buf);
        pattern.score(hay, matcher).unwrap_or(0)
    };
    let mut scored: Vec<(&Entry, u32)> = entries
        .iter()
        .filter_map(|e| {
            let title = score_of(&e.title, &mut matcher);
            let tag = e
                .tags
                .iter()
                .map(|t| score_of(t, &mut matcher))
                .max()
                .unwrap_or(0);
            let cmd = score_of(&e.cmd, &mut matcher);
            let s = 3 * title + 2 * tag + cmd;
            (s > 0).then_some((e, s))
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.id.cmp(&b.0.id)));
    scored.truncate(10);
    scored
}
```

If nucleo-matcher 0.3's `Pattern::score` signature differs (it returns `Option<u32>` in 0.3 — verify against docs.rs if compile fails), adapt the `score_of` closure only; keep the weighting and public signature exactly as above.

- [ ] **Step 4: Wire the CLI in `src/main.rs`**

Replace `src/main.rs`:

```rust
mod corpus;
mod entry;
mod search;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "col", about = "hacky script directory + console drills")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fuzzy-search the corpus
    Search { query: Vec<String> },
}

fn main() {
    let cli = Cli::parse();
    let entries = corpus::load();
    match cli.cmd {
        Cmd::Search { query } => cmd_search(&entries, &query.join(" ")),
    }
}

fn cmd_search(entries: &[entry::Entry], query: &str) {
    let hits = search::search(entries, query);
    if hits.is_empty() {
        eprintln!("no matches for '{query}'");
        std::process::exit(1);
    }
    for (e, _) in hits {
        let preview: String = e.cmd.chars().take(48).collect();
        println!("{:<28} {:<44} {}", e.id, truncate(&e.title, 44), preview);
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}
```

- [ ] **Step 5: Write the integration test `tests/cli.rs`**

```rust
use assert_cmd::Command;

#[test]
fn search_sleep_returns_pmset_entry() {
    Command::cargo_bin("col")
        .unwrap()
        .args(["search", "sleep"])
        .assert()
        .success()
        .stdout(predicates::str::contains("pmset-disable-sleep"));
}
```

Add to `Cargo.toml` `[dev-dependencies]`:

```toml
predicates = "3"
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: all unit + integration tests pass.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: weighted fuzzy search and col search command"
```

---

### Task 4: `col show`, `col copy`, `col random`

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `corpus::load()`, `entry::{Entry, Danger}`
- Produces: CLI subcommands `show <id>`, `copy <id>`, `random`. Helper `find<'a>(entries: &'a [Entry], id: &str) -> &'a Entry` (exits 1 with message if missing).

- [ ] **Step 1: Extend clap enum and dispatch in `src/main.rs`**

Add variants to `Cmd`:

```rust
    /// Show full entry: cmd, explanation, undo, danger, source
    Show { id: String },
    /// Copy the entry's command to the clipboard
    Copy { id: String },
    /// Print one random gem
    Random,
```

Add match arms:

```rust
        Cmd::Show { id } => cmd_show(&entries, &id),
        Cmd::Copy { id } => cmd_copy(&entries, &id),
        Cmd::Random => cmd_show(&entries, &random_id(&entries)),
```

Add the implementations:

```rust
fn find<'a>(entries: &'a [entry::Entry], id: &str) -> &'a entry::Entry {
    entries.iter().find(|e| e.id == id).unwrap_or_else(|| {
        eprintln!("no entry '{id}' — try: col search {id}");
        std::process::exit(1);
    })
}

fn cmd_show(entries: &[entry::Entry], id: &str) {
    use entry::Danger;
    let e = find(entries, id);
    println!("{}  [{}]", e.title, e.domains.join(", "));
    if e.danger == Danger::High {
        println!("\x1b[1;31m⚠ DANGER: high — know your exit before you run this.\x1b[0m");
        if let Some(u) = e.undo.as_deref().filter(|u| !u.is_empty()) {
            println!("\x1b[31m  undo: {u}\x1b[0m");
        }
    }
    println!("\n  {}\n", e.cmd);
    if e.danger != Danger::High {
        if let Some(u) = e.undo.as_deref().filter(|u| !u.is_empty()) {
            println!("undo: {u}");
        }
    }
    println!("{}", e.explanation.trim());
    println!("source: {}", e.source);
}

fn cmd_copy(entries: &[entry::Entry], id: &str) {
    let e = find(entries, id);
    match arboard::Clipboard::new().and_then(|mut c| c.set_text(e.cmd.clone())) {
        Ok(()) => println!("copied: {}", e.cmd),
        Err(err) => {
            eprintln!("clipboard failed ({err}); here it is:\n{}", e.cmd);
            std::process::exit(1);
        }
    }
}

fn random_id(entries: &[entry::Entry]) -> String {
    use rand::seq::SliceRandom;
    entries
        .choose(&mut rand::thread_rng())
        .expect("corpus is never empty")
        .id
        .clone()
}
```

- [ ] **Step 2: Extend `tests/cli.rs`**

```rust
#[test]
fn show_prints_cmd_undo_and_source() {
    Command::cargo_bin("col")
        .unwrap()
        .args(["show", "pmset-disable-sleep"])
        .assert()
        .success()
        .stdout(predicates::str::contains("sudo pmset -a disablesleep 1"))
        .stdout(predicates::str::contains("undo: sudo pmset -a disablesleep 0"))
        .stdout(predicates::str::contains("source: "));
}

#[test]
fn show_unknown_id_fails_with_hint() {
    Command::cargo_bin("col")
        .unwrap()
        .args(["show", "nope-nope"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("col search"));
}

#[test]
fn random_prints_an_entry() {
    Command::cargo_bin("col")
        .unwrap()
        .arg("random")
        .assert()
        .success()
        .stdout(predicates::str::contains("source: "));
}
```

(No automated test for `copy` — clipboard needs a GUI session; the failure path prints the cmd and exits 1, verified manually.)

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 4: Manual smoke test**

```bash
cargo run -q -- show pmset-disable-sleep
cargo run -q -- copy lsof-listening-port   # then paste somewhere
cargo run -q -- random
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: show, copy, and random commands"
```

---

### Task 5: SM-2 algorithm

**Files:**
- Create: `src/sm2.rs`
- Modify: `src/main.rs` (add `mod sm2;`)

**Interfaces:**
- Produces:
  - `sm2::Card { ease: f64, interval_days: f64, due: u64, reps: u32 }` — derives `Serialize, Deserialize, Clone, Copy, Debug, PartialEq`; `Default` = `{ease: 2.5, interval_days: 0.0, due: 0, reps: 0}`.
  - `sm2::review(card: Card, grade: u8, now: u64) -> Card` — grade 1..=4 (again/hard/good/easy), `now` = unix seconds.
  - `sm2::DAY: u64` = 86400.

- [ ] **Step 1: Write failing tests in `src/sm2.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn first_good_review_schedules_one_day() {
        let c = review(Card::default(), 3, NOW);
        assert_eq!(c.reps, 1);
        assert_eq!(c.interval_days, 1.0);
        assert_eq!(c.due, NOW + DAY);
    }

    #[test]
    fn second_good_review_schedules_six_days() {
        let c = review(review(Card::default(), 3, NOW), 3, NOW);
        assert_eq!(c.interval_days, 6.0);
        assert_eq!(c.due, NOW + 6 * DAY);
    }

    #[test]
    fn third_review_multiplies_by_ease() {
        let c = review(review(review(Card::default(), 3, NOW), 3, NOW), 3, NOW);
        assert!(c.interval_days > 6.0);
    }

    #[test]
    fn again_resets_reps_and_is_due_now() {
        let learned = review(review(Card::default(), 3, NOW), 3, NOW);
        let c = review(learned, 1, NOW);
        assert_eq!(c.reps, 0);
        assert_eq!(c.due, NOW);
    }

    #[test]
    fn ease_never_drops_below_floor() {
        let mut c = Card::default();
        for _ in 0..20 {
            c = review(c, 2, NOW); // repeated "hard"
        }
        assert!(c.ease >= 1.3);
    }

    #[test]
    fn easy_grows_ease() {
        let c = review(Card::default(), 4, NOW);
        assert!(c.ease > 2.5);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test sm2`
Expected: FAIL — `Card` / `review` not defined.

- [ ] **Step 3: Implement `src/sm2.rs`**

```rust
use serde::{Deserialize, Serialize};

pub const DAY: u64 = 86400;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub ease: f64,
    pub interval_days: f64,
    pub due: u64,
    pub reps: u32,
}

impl Default for Card {
    fn default() -> Self {
        Card { ease: 2.5, interval_days: 0.0, due: 0, reps: 0 }
    }
}

/// SM-2. grade: 1=again 2=hard 3=good 4=easy (maps to SM-2 quality 2..5).
pub fn review(card: Card, grade: u8, now: u64) -> Card {
    assert!((1..=4).contains(&grade), "grade must be 1..=4");
    let mut c = card;
    if grade == 1 {
        c.reps = 0;
        c.interval_days = 0.0;
        c.due = now;
        return c;
    }
    let q = (grade + 1) as f64; // 3, 4, 5
    c.reps += 1;
    c.interval_days = match c.reps {
        1 => 1.0,
        2 => 6.0,
        _ => c.interval_days * c.ease,
    };
    c.ease = (c.ease + (0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02))).max(1.3);
    c.due = now + (c.interval_days * DAY as f64) as u64;
    c
}
```

Add `mod sm2;` to `src/main.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test sm2`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: SM-2 spaced repetition core"
```

---

### Task 6: Drill state persistence

**Files:**
- Create: `src/drill.rs`
- Modify: `src/main.rs` (add `mod drill;`)

**Interfaces:**
- Consumes: `sm2::Card`
- Produces:
  - `drill::load_state(path: &Path) -> HashMap<String, Card>` — missing file → empty map; corrupt file → eprintln warning + empty map. Never panics.
  - `drill::save_state(path: &Path, state: &HashMap<String, Card>) -> std::io::Result<()>` — creates parent dirs.
  - `drill::default_state_path() -> PathBuf` — `~/.collective/drill.json`.

- [ ] **Step 1: Write failing tests in `src/drill.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sm2::Card;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("col-drill-test-{name}.json"))
    }

    #[test]
    fn missing_file_gives_empty_state() {
        let p = tmp("missing");
        let _ = std::fs::remove_file(&p);
        assert!(load_state(&p).is_empty());
    }

    #[test]
    fn roundtrips_state() {
        let p = tmp("roundtrip");
        let mut state = std::collections::HashMap::new();
        state.insert("pmset-disable-sleep".to_string(), Card::default());
        save_state(&p, &state).unwrap();
        let loaded = load_state(&p);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded["pmset-disable-sleep"], Card::default());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn corrupt_file_resets_without_panic() {
        let p = tmp("corrupt");
        std::fs::write(&p, "{ not json !!").unwrap();
        assert!(load_state(&p).is_empty());
        let _ = std::fs::remove_file(&p);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test drill`
Expected: FAIL — functions not defined.

- [ ] **Step 3: Implement persistence in `src/drill.rs`**

```rust
use crate::sm2::Card;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_state_path() -> PathBuf {
    directories::BaseDirs::new()
        .expect("cannot locate home directory")
        .home_dir()
        .join(".collective/drill.json")
}

pub fn load_state(path: &Path) -> HashMap<String, Card> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| {
            eprintln!("warning: drill state corrupt at {}, resetting", path.display());
            HashMap::new()
        }),
        Err(_) => HashMap::new(),
    }
}

pub fn save_state(path: &Path, state: &HashMap<String, Card>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state).expect("state serializes"))
}
```

Add `mod drill;` to `src/main.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test drill`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: drill state persistence with corrupt-file reset"
```

---

### Task 7: `col drill` session

**Files:**
- Modify: `src/drill.rs` (session loop), `src/main.rs` (subcommand)

**Interfaces:**
- Consumes: `corpus::load()`, `sm2::review`, `drill::{load_state, save_state, default_state_path}`
- Produces:
  - `drill::pick_due<'a>(entries: &'a [Entry], state: &HashMap<String, Card>, domain: Option<&str>, now: u64) -> Vec<&'a Entry>` — unseen entries count as due; ≤20 results.
  - `drill::run(entries: &[Entry], domain: Option<&str>)` — interactive session.
  - CLI: `col drill [--domain <d>]`.

- [ ] **Step 1: Write failing test for `pick_due` in `src/drill.rs` tests module**

```rust
    use crate::corpus;
    use crate::sm2;

    #[test]
    fn pick_due_includes_unseen_and_excludes_future() {
        let entries = corpus::load();
        let now = 1_800_000_000u64;
        let mut state = std::collections::HashMap::new();
        // one card scheduled far in the future -> excluded
        let future = sm2::review(Card::default(), 4, now);
        state.insert("pmset-disable-sleep".to_string(), future);
        let due = pick_due(&entries, &state, None, now);
        assert!(due.len() <= 20);
        assert!(due.iter().all(|e| e.id != "pmset-disable-sleep"));
        assert!(due.iter().any(|e| e.id == "flush-dns-cache")); // unseen = due
    }

    #[test]
    fn pick_due_filters_by_domain() {
        let entries = corpus::load();
        let state = std::collections::HashMap::new();
        let due = pick_due(&entries, &state, Some("git"), 0);
        assert!(!due.is_empty());
        assert!(due.iter().all(|e| e.domains.iter().any(|d| d == "git")));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test drill`
Expected: FAIL — `pick_due` not defined.

- [ ] **Step 3: Implement `pick_due` and `run` in `src/drill.rs`**

```rust
use crate::entry::Entry;
use crate::sm2;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn pick_due<'a>(
    entries: &'a [Entry],
    state: &HashMap<String, Card>,
    domain: Option<&str>,
    now: u64,
) -> Vec<&'a Entry> {
    let mut due: Vec<&Entry> = entries
        .iter()
        .filter(|e| domain.is_none_or(|d| e.domains.iter().any(|x| x == d)))
        .filter(|e| state.get(&e.id).is_none_or(|c| c.due <= now))
        .collect();
    due.truncate(20);
    due
}

pub fn run(entries: &[Entry], domain: Option<&str>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before 1970")
        .as_secs();
    let path = default_state_path();
    let mut state = load_state(&path);
    let due = pick_due(entries, &state, domain, now);
    if due.is_empty() {
        println!("nothing due. come back later.");
        return;
    }
    println!("{} card(s) due. recall the command, Enter reveals.\n", due.len());
    let stdin = io::stdin();
    for e in due {
        println!("── {}", e.title);
        print!("your answer (or Enter to reveal): ");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        stdin.read_line(&mut buf).unwrap();
        let typed = buf.trim();
        println!("  {}", e.cmd);
        if !typed.is_empty() {
            let mark = if typed == e.cmd { "✓ exact" } else { "✗ differs" };
            println!("  you typed: {typed}  {mark}");
        }
        let grade = loop {
            print!("grade  1=again 2=hard 3=good 4=easy: ");
            io::stdout().flush().unwrap();
            let mut g = String::new();
            stdin.read_line(&mut g).unwrap();
            match g.trim().parse::<u8>() {
                Ok(n @ 1..=4) => break n,
                _ => continue,
            }
        };
        let card = state.get(&e.id).copied().unwrap_or_default();
        state.insert(e.id.clone(), sm2::review(card, grade, now));
        if let Err(err) = save_state(&path, &state) {
            eprintln!("warning: could not save drill state: {err}");
        }
        println!();
    }
    println!("session done.");
}
```

(Note: `is_none_or` needs Rust 1.82+; on older toolchains use `map_or(true, ...)`.)

- [ ] **Step 4: Wire subcommand in `src/main.rs`**

Add variant:

```rust
    /// Flashcard drill session (SM-2 spaced repetition)
    Drill {
        #[arg(long)]
        domain: Option<String>,
    },
```

Add match arm:

```rust
        Cmd::Drill { domain } => drill::run(&entries, domain.as_deref()),
```

- [ ] **Step 5: Run all tests + manual session**

Run: `cargo test`
Expected: all pass.

Manual: `cargo run -q -- drill --domain git` — answer a card, grade it, re-run: graded card no longer due (unless graded 1). Check `~/.collective/drill.json` exists.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: col drill flashcard sessions"
```

---

### Task 8: Track 1 — tldr-pages import

**Files:**
- Create: `tools/import_tldr.py`, `corpus/imported/` (generated), `NOTICE`

**Interfaces:**
- Consumes: tldr-pages repo checkout (CC-BY-4.0), Entry YAML schema from Task 1 (build.rs re-validates every generated file).
- Produces: `corpus/imported/tldr-<name>-<n>.yaml` files that pass `cargo build`.

- [ ] **Step 1: Write `tools/import_tldr.py`**

```python
#!/usr/bin/env python3
"""Convert tldr-pages markdown into collective corpus YAML.

Usage:
  git clone --depth 1 https://github.com/tldr-pages/tldr /tmp/tldr
  python3 tools/import_tldr.py /tmp/tldr/pages --platforms osx common --out corpus/imported

Requires: pip install pyyaml
License: tldr-pages content is CC-BY-4.0; every generated entry carries
attribution in its `source` field. See NOTICE.
"""
import argparse
import pathlib
import re
import yaml


def sanitize(name: str) -> str:
    return re.sub(r"[^a-z0-9-]+", "-", name.lower()).strip("-")


def parse_page(text: str):
    """Return (page_description, [{'desc': ..., 'cmd': ...}])."""
    desc_line, examples, pending = "", [], None
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("> ") and not desc_line and "More information" not in line:
            desc_line = line[2:].rstrip(".")
        elif line.startswith("- "):
            pending = line[2:].rstrip(":")
        elif line.startswith("`") and pending:
            cmd = line.strip("`").replace("{{", "<").replace("}}", ">")
            examples.append({"desc": pending, "cmd": cmd})
            pending = None
    return desc_line, examples


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pages_dir", help="path to tldr/pages")
    ap.add_argument("--platforms", nargs="+", default=["osx", "common"])
    ap.add_argument("--out", default="corpus/imported")
    args = ap.parse_args()

    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    seen_pages, count = set(), 0

    for plat in args.platforms:  # osx first: platform page wins over common
        for page in sorted((pathlib.Path(args.pages_dir) / plat).glob("*.md")):
            name = page.stem
            if name in seen_pages:
                continue
            seen_pages.add(name)
            page_desc, examples = parse_page(page.read_text(encoding="utf-8"))
            slug = sanitize(name)
            for i, ex in enumerate(examples, start=1):
                entry = {
                    "id": f"tldr-{slug}-{i}",
                    "title": f"{name}: {ex['desc']}",
                    "cmd": ex["cmd"],
                    "platform": ["macos"],
                    "domains": ["tldr-import"],
                    "danger": "low",
                    "explanation": page_desc or ex["desc"],
                    "source": (
                        f"https://github.com/tldr-pages/tldr/blob/main/pages/{plat}/{name}.md"
                        " (CC-BY-4.0)"
                    ),
                    "tags": [slug],
                }
                (out / f"{entry['id']}.yaml").write_text(
                    yaml.safe_dump(entry, sort_keys=False, allow_unicode=True),
                    encoding="utf-8",
                )
                count += 1
    print(f"wrote {count} entries from {len(seen_pages)} pages to {out}")


if __name__ == "__main__":
    main()
```

Create `NOTICE`:

```
Portions of the corpus under corpus/imported/ are derived from tldr-pages
(https://github.com/tldr-pages/tldr), licensed CC-BY-4.0. Each derived entry
links its source page in its `source` field.
```

- [ ] **Step 2: Run the import**

```bash
git clone --depth 1 https://github.com/tldr-pages/tldr /tmp/tldr
python3 -m pip install --quiet pyyaml 2>/dev/null || pip3 install --quiet pyyaml
python3 tools/import_tldr.py /tmp/tldr/pages --platforms osx common --out corpus/imported
```

Expected: `wrote N entries ...` where N is roughly 3000–6000 (osx + common).

- [ ] **Step 3: Validate through the build gate**

Run: `cargo build && cargo test`
Expected: build validates every imported YAML; all tests still pass. If build fails on a specific file, fix the converter (not the file), delete `corpus/imported/`, re-run the import.

- [ ] **Step 4: Spot-check quality and prune**

```bash
cargo run -q -- search "git rebase"
cargo run -q -- show tldr-git-rebase-1
ls corpus/imported | wc -l
```

If volume overwhelms search results (imported entries drowning curated gems), prune to the spec's macOS/shell/git focus: keep pages matching git/macOS tools, delete obvious non-dev pages. Document what was pruned in the commit message. (Spec target after filtering: ~500–1500 entries.)

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: import tldr-pages corpus (Track 1, CC-BY-4.0 attributed)"
```

---

### Task 9: Track 2 — multi-agent gem-mining research sweep

**Files:**
- Create: `corpus/*.yaml` (new gems), `research-notes.md`

This task is research orchestration, not TDD code. The build gate (`cargo build`) is the validator for everything produced.

- [ ] **Step 1: Run the research sweep**

Dispatch parallel research agents (Workflow tool or parallel Agent calls), one per lens from the spec:

1. macOS internals (`defaults write`, `pmset`, `networksetup`, `mdfind`, `caffeinate`, `tmutil`, `softwareupdate`, `xattr`, `codesign`) — sources: macos-defaults.com, HN, Apple forums, dotfiles
2. Dotfiles archaeology — mathiasbynens/dotfiles, holman/dotfiles, paulirish/dotfiles
3. HN/Reddit one-liner threads — vote-signal as quality filter
4. Shell wizardry — awk/sed/jq/xargs/find, `lsof`, process/port/network debugging, git plumbing
5. Blog canon — Julia Evans, Brandur, "things I wish I knew" genre

Prompt template for each agent (fill `<LENS>`/`<SOURCES>`):

```
Research lens: <LENS>. Sources to mine: <SOURCES>.
Find hacky, super-functional developer commands. Quality gate: each must be
NON-OBVIOUS or FREQUENTLY FORGOTTEN — no `ls -la` filler.
Return 15-30 entries as YAML matching EXACTLY this schema (one document per
entry, `---` separated):

id: <lowercase-digits-hyphens, unique, descriptive>
title: <what it does, imperative, specific>
cmd: <the command, real flags, placeholders as <angle-brackets>>
undo: <undo command or "">
platform: [macos] # and/or linux
domains: [<from: power, macos-admin, network, git, files, disk, debugging, shell, security>]
danger: <low|medium|high — high if destructive/irreversible, medium if sudo/history-rewriting/safety-bypassing>
explanation: >
  <2-3 lines: what, why useful, any gotcha>
source: <URL where found>
tags: [<3-5 search keywords>]

Also return a short "rejected but interesting" list with reasons.
```

- [ ] **Step 2: Verify pass**

For each batch, dispatch a verifier agent: check macOS command validity (flag existence per man pages), correct danger ratings (anything destructive → high), dedupe against existing corpus ids (`ls corpus/ corpus/imported/`). Verifier returns pass/fix/reject per entry.

- [ ] **Step 3: Land entries**

Write surviving entries as `corpus/<id>.yaml` (one file each, hand-skim every one — you are the final quality gate). Write `research-notes.md` with the rejected-but-interesting leads from all lenses.

Run: `cargo build && cargo test`
Expected: build gate validates all new YAML; tests pass.

- [ ] **Step 4: Smoke test the gems**

```bash
cargo run -q -- search "dns"
cargo run -q -- random   # run a few times, should surface new gems
cargo run -q -- drill --domain macos-admin
```

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: Track 2 research corpus — gem-mining sweep across 5 lenses"
```

---

## Done Criteria

- `cargo test` green (unit: entry/corpus/search/sm2/drill; integration: search/show/random).
- `cargo build` fails on any invalid corpus YAML (verified by deliberate break in Task 1).
- `col search sleep` surfaces `pmset-disable-sleep` (spec's integration test).
- Corpus: 10 curated seeds + filtered tldr import + Track 2 gems, all with provenance.
- `col drill` runs a session and persists state; corrupt state resets with warning.
