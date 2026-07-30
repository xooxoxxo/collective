# App Awareness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** collective knows which app each entry needs, whether it is on PATH, and grays/filters/skips accordingly — with an in-TUI app pane offering info and an install-command prefill.

**Architecture:** A new dependency-free `src/apps.rs` owns the registry types, app derivation, and the PATH scan; `build.rs` includes it via `#[path]` (the `entry.rs` precedent) to validate the new root-level `apps.yaml` at build time. TUI, drill, and CLI surfaces consume one `Availability` value computed per run.

**Tech Stack:** Rust, serde_yaml_bw, ratatui/crossterm (existing), std-only PATH scan.

**Spec:** `docs/superpowers/specs/2026-07-30-app-awareness-design.md`

## Global Constraints

- **Registry location deviates from the spec deliberately:** `apps.yaml` lives at the REPO ROOT, not `corpus/apps.yaml`. Both `build.rs` and `corpus::embedded()` walk `corpus/` parsing every `.yaml` as an Entry; a registry file inside would break both. The spec's content requirements are unchanged.
- Registry fields per app: `binary`, `name`, `description`, `homepage` required; `install:` optional with only `brew` and `apt` keys. Duplicate `binary` = build error. Entry `app:` naming an unregistered binary = build error.
- Derivation: skip leading `sudo`, `env`, `VAR=value` tokens; basename path-qualified candidates; builtin allowlist (`cd`, `export`, `alias`, `set`, `unset`, `source`, `eval`, `echo`, `read`, `trap`, `ulimit`) ⇒ no app.
- Availability: in-process PATH walk (regular file + any executable bit), no subprocesses, no persistent cache. No-app entries are always available.
- Never gray falsely: unknown binaries get a real PATH check; only a definitive "not found" grays.
- Platform install pick: `brew` on macOS, `apt` on Linux; missing key ⇒ point at homepage.
- Repo rules: `cargo clippy --all-targets -- -D warnings` exits 0 before EVERY commit; plain `cargo test` (no `--lib` — binary-only crate); `rm` is blocked, use `trash`/`mv`; commit locally, do NOT push (release is a separate decision).
- Every new test gets a falsification pass before its commit: deliberately break the line the test depends on, confirm the test fails, restore. State this in the task report.
- TDD per task: failing test → verify fail → implement → verify pass → commit.

---

### Task 1: `src/apps.rs` — registry, derivation, availability + build gate

**Files:**
- Create: `apps.yaml` (repo root)
- Create: `src/apps.rs`
- Modify: `src/main.rs:1-12` (add `mod apps;`)
- Modify: `build.rs` (validate `apps.yaml`)

**Interfaces:**
- Produces (later tasks rely on these exact signatures):
  - `apps::AppInfo { pub binary: String, pub name: String, pub description: String, pub homepage: String, pub install: Install }`, `apps::Install { pub brew: Option<String>, pub apt: Option<String> }`
  - `apps::registry() -> &'static std::collections::HashMap<String, AppInfo>` (keyed by binary)
  - `apps::derive_binary(cmd: &str) -> Option<String>`
  - `apps::Availability` with `Availability::scan<'a>(binaries: impl Iterator<Item = &'a str>, path_var: &str) -> Availability` and `available(&self, binary: Option<&str>) -> bool` (None ⇒ true)
  - `apps::install_for_platform(app: &AppInfo) -> Option<&str>` (brew on macOS, apt on Linux)
  - `apps.rs` has NO `crate::` imports — `build.rs` includes it via `#[path]`.

- [ ] **Step 1: Seed `apps.yaml`**

```yaml
apps:
  - binary: btop
    name: btop
    description: Modern system resource dashboard with GPU and disk I/O views
    homepage: https://github.com/aristocratos/btop
    install:
      brew: brew install btop
      apt: apt install btop
  - binary: rg
    name: ripgrep
    description: Recursively search directories with regex, gitignore-aware
    homepage: https://github.com/BurntSushi/ripgrep
    install:
      brew: brew install ripgrep
      apt: apt install ripgrep
```

(Full population is Task 3; two apps are enough to build the machinery.)

- [ ] **Step 2: Write the failing tests**

Create `src/apps.rs` containing ONLY the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_plain_and_prefixed_commands() {
        assert_eq!(derive_binary("btop"), Some("btop".into()));
        assert_eq!(derive_binary("rg --files"), Some("rg".into()));
        assert_eq!(derive_binary("sudo pmset -a disablesleep 1"), Some("pmset".into()));
        assert_eq!(derive_binary("env FOO=1 jq '.x'"), Some("jq".into()));
        assert_eq!(derive_binary("RUST_LOG=debug cargo test"), Some("cargo".into()));
        assert_eq!(derive_binary("/usr/local/bin/htop"), Some("htop".into()));
    }

    #[test]
    fn builtins_and_empty_have_no_app() {
        assert_eq!(derive_binary("cd /tmp"), None);
        assert_eq!(derive_binary("export PATH=/x:$PATH"), None);
        assert_eq!(derive_binary(""), None);
        assert_eq!(derive_binary("sudo"), None);
    }

    #[test]
    fn registry_parses_and_contains_seed() {
        let reg = registry();
        assert!(reg.contains_key("rg"));
        assert_eq!(reg["rg"].name, "ripgrep");
        assert_eq!(reg["rg"].install.brew.as_deref(), Some("brew install ripgrep"));
    }

    #[test]
    fn scan_finds_executables_and_misses_absent() {
        let dir = std::env::temp_dir().join(format!("col-apps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("fakeapp");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let plain = dir.join("notexec");
        std::fs::write(&plain, "x").unwrap();

        let path_var = dir.to_str().unwrap().to_string();
        let names = ["fakeapp", "notexec", "missingapp"];
        let avail = Availability::scan(names.iter().copied(), &path_var);
        assert!(avail.available(Some("fakeapp")));
        assert!(!avail.available(Some("notexec")), "exec bit required");
        assert!(!avail.available(Some("missingapp")));
        assert!(avail.available(None), "no app is always available");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unscanned_binary_defaults_to_available() {
        let avail = Availability::scan(std::iter::empty(), "/nonexistent-path-dir");
        assert!(avail.available(Some("never-scanned")), "never gray falsely");
    }

    #[test]
    fn install_for_platform_picks_current_os() {
        let app = registry().get("rg").unwrap();
        let got = install_for_platform(app);
        #[cfg(target_os = "macos")]
        assert_eq!(got, Some("brew install ripgrep"));
        #[cfg(target_os = "linux")]
        assert_eq!(got, Some("apt install ripgrep"));
    }
}
```

Add `mod apps;` to the module list at the top of `src/main.rs`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test apps 2>&1 | tail -20`
Expected: compile error — `derive_binary`, `registry`, `Availability` not defined.

- [ ] **Step 4: Implement `src/apps.rs` (above the test module)**

```rust
//! App registry, app derivation from commands, and PATH availability.
//! No `crate::` imports: build.rs includes this file via #[path] to
//! validate apps.yaml at build time (same pattern as entry.rs).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppInfo {
    pub binary: String,
    pub name: String,
    pub description: String,
    pub homepage: String,
    #[serde(default)]
    pub install: Install,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Install {
    pub brew: Option<String>,
    pub apt: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub apps: Vec<AppInfo>,
}

impl Registry {
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for a in &self.apps {
            for (field, v) in [
                ("binary", &a.binary),
                ("name", &a.name),
                ("description", &a.description),
                ("homepage", &a.homepage),
            ] {
                if v.trim().is_empty() {
                    return Err(format!("app {:?}: empty {field}", a.binary));
                }
            }
            if !seen.insert(a.binary.clone()) {
                return Err(format!("duplicate app binary: {}", a.binary));
            }
        }
        Ok(())
    }
}

pub fn registry() -> &'static HashMap<String, AppInfo> {
    static REG: OnceLock<HashMap<String, AppInfo>> = OnceLock::new();
    REG.get_or_init(|| {
        let reg: Registry = serde_yaml_bw::from_str(include_str!("../apps.yaml"))
            .expect("apps.yaml validated at build time");
        reg.apps.into_iter().map(|a| (a.binary.clone(), a)).collect()
    })
}

const BUILTINS: &[&str] = &[
    "cd", "export", "alias", "set", "unset", "source", "eval", "echo", "read",
    "trap", "ulimit",
];

/// The binary a command needs, or None for builtins/empty commands.
pub fn derive_binary(cmd: &str) -> Option<String> {
    let tok = cmd
        .split_whitespace()
        .find(|t| *t != "sudo" && *t != "env" && !t.contains('='))?;
    let base = tok.rsplit('/').next().unwrap_or(tok);
    if BUILTINS.contains(&base) {
        return None;
    }
    Some(base.to_string())
}

/// PATH presence for the binaries scanned this run. Binaries never scanned
/// resolve to available — graying must never be a false positive.
pub struct Availability(HashMap<String, bool>);

impl Availability {
    pub fn scan<'a>(binaries: impl Iterator<Item = &'a str>, path_var: &str) -> Availability {
        let dirs: Vec<&str> = path_var.split(':').filter(|d| !d.is_empty()).collect();
        let mut map = HashMap::new();
        for bin in binaries {
            if map.contains_key(bin) {
                continue;
            }
            let found = dirs.iter().any(|d| is_executable(&std::path::Path::new(d).join(bin)));
            map.insert(bin.to_string(), found);
        }
        Availability(map)
    }

    pub fn available(&self, binary: Option<&str>) -> bool {
        match binary {
            None => true,
            Some(b) => *self.0.get(b).unwrap_or(&true),
        }
    }
}

fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// The install command for the running platform, if the registry has one.
pub fn install_for_platform(app: &AppInfo) -> Option<&str> {
    #[cfg(target_os = "macos")]
    return app.install.brew.as_deref();
    #[cfg(not(target_os = "macos"))]
    return app.install.apt.as_deref();
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test apps 2>&1 | tail -5`
Expected: all apps tests pass.

- [ ] **Step 6: Extend `build.rs` to validate `apps.yaml`**

Add below the existing `mod entry;` block:

```rust
#[path = "src/apps.rs"]
#[allow(dead_code)]
mod apps;
```

And at the end of `main()` (after the entry loop):

```rust
println!("cargo:rerun-if-changed=apps.yaml");
let text = fs::read_to_string("apps.yaml").expect("apps.yaml missing");
let reg: apps::Registry =
    serde_yaml_bw::from_str(&text).unwrap_or_else(|err| panic!("apps.yaml: {err}"));
reg.validate().unwrap_or_else(|err| panic!("apps.yaml: {err}"));
```

Note: `src/apps.rs` has a `#[cfg(test)]` module and no crate imports, so the `#[path]` include compiles in the build script exactly like `entry.rs` does.

- [ ] **Step 7: Verify build gate actually gates**

Temporarily duplicate the `rg` block in `apps.yaml`, run `cargo build 2>&1 | tail -3`, expect `duplicate app binary: rg` panic; restore the file, `cargo build` succeeds. (This is the falsification pass for the gate.)

- [ ] **Step 8: Full suite + commit**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
git add apps.yaml src/apps.rs src/main.rs build.rs
git commit -m "feat: app registry, derivation, and PATH availability core"
```

---

### Task 2: Entry `app:` field + build-time registry cross-check

**Files:**
- Modify: `src/entry.rs` (field + fixture)
- Modify: `build.rs` (cross-check)
- Modify: `src/corpus.rs`, `src/tui/mod.rs` (Entry literal fixtures gain `app: None`)

**Interfaces:**
- Consumes: `apps::Registry` from Task 1.
- Produces: `Entry.app: Option<String>` — later tasks call `e.app.as_deref()`.
- Produces: `apps::entry_binary(app_field: Option<&str>, cmd: &str) -> Option<String>` in `src/apps.rs` — explicit field wins, else derivation.

- [ ] **Step 1: Write the failing tests**

In `src/entry.rs` tests:

```rust
#[test]
fn app_field_is_optional_and_parsed() {
    let e: Entry = serde_yaml_bw::from_str(GOOD).unwrap();
    assert_eq!(e.app, None);
    let with = format!("{GOOD}\napp: pmset");
    let e: Entry = serde_yaml_bw::from_str(&with).unwrap();
    assert_eq!(e.app.as_deref(), Some("pmset"));
}
```

In `src/apps.rs` tests:

```rust
#[test]
fn entry_binary_prefers_explicit_field() {
    assert_eq!(entry_binary(Some("delta"), "git diff"), Some("delta".into()));
    assert_eq!(entry_binary(None, "rg --files"), Some("rg".into()));
    assert_eq!(entry_binary(None, "cd /tmp"), None);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test app_field entry_binary 2>&1 | tail -10`
Expected: compile error — no `app` field, no `entry_binary`.

- [ ] **Step 3: Implement**

`src/entry.rs` — after the `undo` field:

```rust
    #[serde(default)]
    pub app: Option<String>,
```

`src/apps.rs`:

```rust
/// The binary an entry needs: explicit `app:` field wins, else derived.
pub fn entry_binary(app_field: Option<&str>, cmd: &str) -> Option<String> {
    match app_field {
        Some(a) => Some(a.to_string()),
        None => derive_binary(cmd),
    }
}
```

Fix every `Entry { … }` literal the compiler flags by adding `app: None,` (the fixture in `src/corpus.rs::fixture_entry`, the three literals in `src/tui/mod.rs::curated_only_hides_bulk_imports`, and any others `cargo build` names).

- [ ] **Step 4: build.rs cross-check**

In `build.rs`, the entry loop currently reads each entry then inserts its id. Collect app fields too: change the loop body to also push `(p.display().to_string(), e.app.clone())` onto a `Vec<(String, Option<String>)> app_refs`, and after the registry is parsed and validated add:

```rust
let known: HashSet<&str> = reg.apps.iter().map(|a| a.binary.as_str()).collect();
for (file, app) in &app_refs {
    if let Some(a) = app {
        assert!(known.contains(a.as_str()), "{file}: app '{a}' not in apps.yaml");
    }
}
```

(Move the apps.yaml parse ABOVE the entry loop so `reg` exists when the loop ends, keeping one pass.)

- [ ] **Step 5: Verify the gate**

Add `app: nonexistent-tool` to `corpus/flush-dns-cache.yaml`, run `cargo build 2>&1 | tail -3`, expect the assert message; revert the file. Then `cargo test` passes.

- [ ] **Step 6: Commit**

```bash
cargo clippy --all-targets -- -D warnings
git add src/entry.rs src/apps.rs src/corpus.rs src/tui/mod.rs build.rs
git commit -m "feat: optional app field on entries, build-gated against the registry"
```

---

### Task 3: Populate `apps.yaml` from the embedded corpus

**Files:**
- Modify: `apps.yaml`
- Modify: individual `corpus/**/*.yaml` files ONLY where derivation is wrong (add `app:`)

**Interfaces:**
- Consumes: `derive_binary`, registry validation from Tasks 1–2.

- [ ] **Step 1: List candidate binaries**

Write a throwaway script (scratch, not committed) or use a one-off test to print `derive_binary` over all embedded entries' cmds with counts, e.g.:

```bash
grep -h '^cmd:' corpus/*.yaml corpus/gems/*.yaml | sed 's/^cmd: *//' > /tmp/cmds.txt
```

then a 10-line `#[test] #[ignore]` in apps.rs that derives and prints unique binaries (run with `cargo test -- --ignored print_binaries --nocapture`), or equivalent. Delete the helper after use.

- [ ] **Step 2: Curate the registry**

Register every derived binary that is a real installable application — the judgment rule: it has a homepage and a brew formula, and is NOT part of the base system (skip pmset, defaults, launchctl, mdfind, tmutil, csrutil, softwareupdate, xattr, killall, lsof, dscacheutil, networksetup, scutil, diskutil, git, ssh, curl, tar, awk, sed, grep, find, and similar). Expected registrations, roughly 25–40, including at least: `btop`, `htop`, `eza`, `bat`, `delta` (name git-delta), `entr`, `fzf`, `rg` (name ripgrep), `jq`, `gh`, `tmux`, `ncdu`, `tldr`, `nvim`, `yt-dlp`, `ffmpeg`, `pandoc`, `hyperfine`, `fd`, `dust`, `zoxide` — whichever of these the corpus actually references, plus what the derivation list surfaces.

For each: verify the brew install string with `brew info <formula> 2>/dev/null | head -1` (formula name, not binary name — e.g. `rg` → `ripgrep`, `delta` → `git-delta`, `fd` → `fd`). Homepage from the brew info output or the project's GitHub. `apt` string only where the Ubuntu package name is known and standard; omit otherwise (falls back to homepage on Linux).

- [ ] **Step 3: Add `app:` overrides where derivation misleads**

Scan the corpus for entries whose FIRST token is not the app the entry is about (e.g. a `git config` entry demonstrating `delta`, a pipeline like `rg --files | fzf`). Add `app: <binary>` to those YAML files. Expect a handful, not dozens.

- [ ] **Step 4: Verify**

```bash
cargo build      # registry + cross-check gates pass
cargo test       # suite green
```

Spot-check three registered apps: `cargo run --quiet -- show <an-entry-id>` still renders (full rendering lands in Task 7 — here just confirm nothing broke).

- [ ] **Step 5: Commit**

```bash
cargo clippy --all-targets -- -D warnings
git add apps.yaml corpus/
git commit -m "feat: populate app registry from the embedded corpus"
```

---

### Task 4: TUI graying + `^T` available-only filter

**Files:**
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/ui.rs`

**Interfaces:**
- Consumes: `apps::{Availability, entry_binary}`.
- Produces: `App.availability: apps::Availability`, `App.available_only: bool`, `App::toggle_available_only()`, `App::entry_available(&self, e: &Entry) -> bool` — Task 5 reuses all of these.

- [ ] **Step 1: Write the failing tests**

In `src/tui/mod.rs` tests:

```rust
fn fixture(id: &str, cmd: &str) -> Entry {
    Entry {
        id: id.into(),
        title: id.into(),
        cmd: cmd.into(),
        undo: None,
        app: None,
        platform: vec!["macos".into()],
        domains: vec!["shell".into()],
        danger: crate::entry::Danger::Low,
        explanation: "e".into(),
        source: "s".into(),
        tags: vec![],
    }
}

#[test]
fn available_only_hides_missing_apps() {
    let entries = vec![
        fixture("has-app", "definitely-not-on-path-xyzq --flag"),
        fixture("no-app", "cd /tmp"),
    ];
    let mut a = App::new(entries, HashSet::new());
    assert_eq!(a.visible().len(), 2);
    assert!(!a.entry_available(a.all.iter().find(|e| e.id == "has-app").unwrap()));
    assert!(a.entry_available(a.all.iter().find(|e| e.id == "no-app").unwrap()));
    a.toggle_available_only();
    assert_eq!(a.visible().len(), 1);
    assert_eq!(a.visible()[0].id, "no-app");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test available_only 2>&1 | tail -5`
Expected: compile error — no `entry_available` / `toggle_available_only` / `availability` field.

- [ ] **Step 3: Implement in `src/tui/mod.rs`**

`App` gains two fields:

```rust
    pub availability: crate::apps::Availability,
    pub available_only: bool,
```

`App::new` computes the scan before constructing (binaries collected from all entries):

```rust
    pub fn new(all: Vec<Entry>, favorites: HashSet<String>) -> App {
        let binaries: Vec<String> = all
            .iter()
            .filter_map(|e| crate::apps::entry_binary(e.app.as_deref(), &e.cmd))
            .collect();
        let availability = crate::apps::Availability::scan(
            binaries.iter().map(|s| s.as_str()),
            &std::env::var("PATH").unwrap_or_default(),
        );
        let mut app = App {
            all,
            filtered: Vec::new(),
            selected: 0,
            filter: String::new(),
            favorites,
            fav_only: false,
            curated_only: false,
            availability,
            available_only: false,
        };
        app.recompute();
        app
    }

    pub fn entry_available(&self, e: &Entry) -> bool {
        let bin = crate::apps::entry_binary(e.app.as_deref(), &e.cmd);
        self.availability.available(bin.as_deref())
    }

    pub fn toggle_available_only(&mut self) {
        self.available_only = !self.available_only;
        self.recompute();
    }
```

In `recompute`, after the `curated_only` retain:

```rust
        if self.available_only {
            let avail: Vec<bool> = idx
                .iter()
                .map(|&i| {
                    let e = &self.all[i];
                    let bin = crate::apps::entry_binary(e.app.as_deref(), &e.cmd);
                    self.availability.available(bin.as_deref())
                })
                .collect();
            let mut it = avail.iter();
            idx.retain(|_| *it.next().unwrap());
        }
```

(The two-pass shape avoids borrowing `self` inside `retain`.)

Key binding, next to `^U`:

```rust
                (KeyCode::Char('t'), KeyModifiers::CONTROL) => app.toggle_available_only(),
```

- [ ] **Step 4: Gray rows in `src/tui/ui.rs`**

Replace the row style selection:

```rust
        let style = if !app.entry_available(e) {
            Style::default().fg(Color::DarkGray)
        } else if e.danger == Danger::High {
            Style::default().fg(Color::Red)
        } else {
            Style::default()
        };
```

Update the help bar line:

```rust
    let help = Paragraph::new("↵ prefill  ^Y copy  ^S ★  ^O fav  ^U curated  ^T avail  ^A app  Esc quit")
```

(`^A` ships in Task 5; naming it now keeps the bar stable across the two commits.)

- [ ] **Step 5: Run tests to verify pass, then full suite**

Run: `cargo test available_only 2>&1 | tail -3` → PASS. Then `cargo test`.
Falsification: invert `available` in `entry_available` (`!self.availability…`), confirm the test fails, restore.

- [ ] **Step 6: Commit**

```bash
cargo clippy --all-targets -- -D warnings
git add src/tui/mod.rs src/tui/ui.rs
git commit -m "feat: gray unavailable entries and add ^T available-only filter"
```

---

### Task 5: TUI app pane (`^A`)

**Files:**
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/ui.rs`

**Interfaces:**
- Consumes: `apps::{registry, entry_binary, install_for_platform}`, `App.availability` from Task 4.
- Produces: `App.pane: Option<AppPane>` where `pub struct AppPane { pub binary: Option<String> }`; `App::open_pane()`, `App::close_pane()`; pane-selected install command flows through the existing `picked`/`deliver` path.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn pane_opens_for_selected_entry_and_closes() {
    let entries = vec![fixture("rg-entry", "rg --files")];
    let mut a = App::new(entries, HashSet::new());
    assert!(a.pane.is_none());
    a.open_pane();
    assert_eq!(a.pane.as_ref().unwrap().binary.as_deref(), Some("rg"));
    a.close_pane();
    assert!(a.pane.is_none());
}

#[test]
fn pane_install_cmd_comes_from_registry() {
    let entries = vec![fixture("rg-entry", "rg --files"), fixture("no-app", "cd /tmp")];
    let mut a = App::new(entries, HashSet::new());
    a.open_pane();
    let cmd = a.pane_install_cmd();
    #[cfg(target_os = "macos")]
    assert_eq!(cmd.as_deref(), Some("brew install ripgrep"));
    a.close_pane();
    a.move_down();
    a.open_pane();
    assert_eq!(a.pane_install_cmd(), None, "no registry app, no install");
}
```

(Note: `fixture` sorts by construction order here; `App::new` does not sort — `corpus::load` does. With two fixtures, "no-app" sits at index 1 only if ids sort that way; use ids `a-rg-entry` / `b-no-app` if ordering surprises — assert on `selected_entry().unwrap().id` first when debugging.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test pane 2>&1 | tail -5`
Expected: compile error — no `pane` field / methods.

- [ ] **Step 3: Implement in `src/tui/mod.rs`**

```rust
pub struct AppPane {
    pub binary: Option<String>,
}
```

`App` gains `pub pane: Option<AppPane>` (init `None` in `new`). Methods:

```rust
    pub fn open_pane(&mut self) {
        if let Some(e) = self.selected_entry() {
            let binary = crate::apps::entry_binary(e.app.as_deref(), &e.cmd);
            self.pane = Some(AppPane { binary });
        }
    }

    pub fn close_pane(&mut self) {
        self.pane = None;
    }

    pub fn pane_install_cmd(&self) -> Option<String> {
        let pane = self.pane.as_ref()?;
        let bin = pane.binary.as_deref()?;
        let app = crate::apps::registry().get(bin)?;
        crate::apps::install_for_platform(app).map(str::to_string)
    }

    pub fn pane_homepage(&self) -> Option<String> {
        let pane = self.pane.as_ref()?;
        let bin = pane.binary.as_deref()?;
        crate::apps::registry().get(bin).map(|a| a.homepage.clone())
    }
```

Event loop: when the pane is open it captures keys. Insert at the TOP of the `match` (before Esc):

```rust
                _ if app.pane.is_some() => match (key.code, key.modifiers) {
                    (KeyCode::Esc, _) => app.close_pane(),
                    (KeyCode::Enter, _) => {
                        if let Some(cmd) = app.pane_install_cmd() {
                            picked = Some(cmd);
                            break;
                        }
                    }
                    (KeyCode::Char('o'), m) if m.is_empty() => {
                        if let Some(url) = app.pane_homepage() {
                            open_url(&url);
                        }
                    }
                    _ => {}
                },
```

(A guard arm keyed on `app.pane.is_some()` must precede the normal bindings; Rust match arms are ordered, so place it first and keep the existing arms unchanged below it.)

`^A` binding with the other CONTROL keys:

```rust
                (KeyCode::Char('a'), KeyModifiers::CONTROL) => app.open_pane(),
```

And the opener helper at module level:

```rust
fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(not(target_os = "macos"))]
    let opener = "xdg-open";
    let _ = std::process::Command::new(opener)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
```

Note: the existing `if let Some(cmd) = picked` tail already runs `placeholder::fill_interactive` and `deliver` — install commands contain no placeholders, so the prefill path needs no change.

- [ ] **Step 4: Render the pane in `src/tui/ui.rs`**

In `draw`, replace the detail-pane construction with a branch: when `app.pane` is `Some`, the detail chunk renders the app pane instead of the entry detail:

```rust
    let detail = if let Some(pane) = &app.pane {
        let lines = match pane.binary.as_deref().and_then(|b| crate::apps::registry().get(b)) {
            Some(info) => {
                let install = app
                    .pane_install_cmd()
                    .unwrap_or_else(|| "see homepage".to_string());
                vec![
                    Line::from(Span::styled(
                        format!("{} ({})", info.name, info.binary),
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(info.description.clone()),
                    Line::from(format!("homepage: {}", info.homepage)),
                    Line::from(format!("install:  {install}")),
                    Line::from(""),
                    Line::from(Span::styled(
                        "↵ prefill install  o open homepage  Esc close",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]
            }
            None => vec![Line::from(match pane.binary.as_deref() {
                Some(b) => format!("no app info for {b}"),
                None => "built-in command".to_string(),
            })],
        };
        Paragraph::new(lines).wrap(Wrap { trim: true })
    } else {
        match app.selected_entry() {
            /* existing Some/None arms unchanged */
        }
    }
    .block(Block::default().borders(Borders::ALL).title(if app.pane.is_some() {
        "app"
    } else {
        "detail"
    }));
```

- [ ] **Step 5: Tests pass + falsification**

`cargo test pane` → PASS; `cargo test` green. Falsification: make `pane_install_cmd` return `None` unconditionally, confirm `pane_install_cmd_comes_from_registry` fails, restore.

- [ ] **Step 6: Manual smoke**

`cargo run` → select an entry for a registered app, `^A`, see the pane; Esc; quit. Confirm terminal restores.

- [ ] **Step 7: Commit**

```bash
cargo clippy --all-targets -- -D warnings
git add src/tui/mod.rs src/tui/ui.rs
git commit -m "feat: app info pane with install prefill and homepage open"
```

---

### Task 6: Drill skips unavailable apps

**Files:**
- Modify: `src/drill.rs`

**Interfaces:**
- Consumes: `apps::{Availability, entry_binary}`.
- Produces: `drill::is_due(e: &Entry, state: &HashMap<String, Card>, domain: Option<&str>, now: u64) -> bool` (extracted; `pick_due` uses it).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn unavailable_apps_are_skipped_and_counted() {
    let mk = |id: &str, cmd: &str| Entry {
        id: id.into(),
        title: id.into(),
        cmd: cmd.into(),
        undo: None,
        app: None,
        platform: vec!["macos".into()],
        domains: vec!["shell".into()],
        danger: crate::entry::Danger::Low,
        explanation: "e".into(),
        source: "s".into(),
        tags: vec![],
    };
    let entries = vec![
        mk("ok", "cd /tmp"),
        mk("missing", "definitely-not-on-path-xyzq --run"),
    ];
    let state = std::collections::HashMap::new();
    let avail = crate::apps::Availability::scan(
        entries
            .iter()
            .filter_map(|e| crate::apps::entry_binary(e.app.as_deref(), &e.cmd))
            .collect::<Vec<_>>()
            .iter()
            .map(|s| s.as_str()),
        &std::env::var("PATH").unwrap_or_default(),
    );
    let (runnable, skipped) = split_by_availability(&entries, &state, None, 0, &avail);
    assert_eq!(runnable.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["ok"]);
    assert_eq!(skipped, 1);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test unavailable_apps 2>&1 | tail -5`
Expected: compile error — no `split_by_availability`.

- [ ] **Step 3: Implement**

Extract the due predicate and add the split:

```rust
pub fn is_due(
    e: &Entry,
    state: &HashMap<String, Card>,
    domain: Option<&str>,
    now: u64,
) -> bool {
    domain.is_none_or(|d| e.domains.iter().any(|x| x == d))
        && state.get(&e.id).is_none_or(|c| c.due <= now)
}

/// Due entries split into (runnable, skipped-count) by app availability.
pub fn split_by_availability<'a>(
    entries: &'a [Entry],
    state: &HashMap<String, Card>,
    domain: Option<&str>,
    now: u64,
    avail: &crate::apps::Availability,
) -> (Vec<&'a Entry>, usize) {
    let due: Vec<&Entry> = entries.iter().filter(|e| is_due(e, state, domain, now)).collect();
    let mut runnable = Vec::new();
    let mut skipped = 0usize;
    for e in due {
        let bin = crate::apps::entry_binary(e.app.as_deref(), &e.cmd);
        if avail.available(bin.as_deref()) {
            runnable.push(e);
        } else {
            skipped += 1;
        }
    }
    (runnable, skipped)
}
```

Rewrite `pick_due`'s filter to use `is_due` (behavior unchanged):

```rust
    let mut due: Vec<&Entry> = entries.iter().filter(|e| is_due(e, state, domain, now)).collect();
```

In `run()`, replace the `pick_due` call:

```rust
    let avail = crate::apps::Availability::scan(
        entries
            .iter()
            .filter_map(|e| crate::apps::entry_binary(e.app.as_deref(), &e.cmd))
            .collect::<Vec<_>>()
            .iter()
            .map(|s| s.as_str()),
        &std::env::var("PATH").unwrap_or_default(),
    );
    let (runnable, skipped) = split_by_availability(entries, &state, domain, now, &avail);
    let mut due: Vec<&Entry> = runnable;
    use rand::seq::SliceRandom;
    due.shuffle(&mut rand::rng());
    due.sort_by_key(|e| state.get(&e.id).map_or(0, |c| c.due));
    due.truncate(20);
    if skipped > 0 {
        println!("{skipped} skipped (app not installed)");
    }
```

then keep the existing `if due.is_empty()` and session loop. Remove the now-unused body of `pick_due`'s shuffle/sort duplication by having `run()` no longer call `pick_due` — but KEEP `pick_due` itself (tests and any callers use it; it remains the pure "due + ranked" helper). If clippy flags duplication, extract the shuffle/sort/truncate into `fn rank(due: Vec<&Entry>, state: &…) -> Vec<&Entry>` used by both.

- [ ] **Step 4: Tests pass + falsification**

`cargo test unavailable_apps` → PASS; full `cargo test` green (existing `pick_due` tests must still pass — `is_due` extraction is behavior-neutral). Falsification: flip `avail.available` to `!avail.available` in the split, confirm the new test fails, restore.

- [ ] **Step 5: Commit**

```bash
cargo clippy --all-targets -- -D warnings
git add src/drill.rs
git commit -m "feat: drill sessions skip entries whose app is not installed"
```

---

### Task 7: CLI search dim + show app/install lines

**Files:**
- Modify: `src/main.rs` (`cmd_search`, `cmd_show`)

**Interfaces:**
- Consumes: `apps::{Availability, entry_binary, registry, install_for_platform}`.

- [ ] **Step 1: `cmd_search` dims unavailable rows**

At the top of `cmd_search`, build availability over the filtered set:

```rust
    let avail = apps::Availability::scan(
        filtered
            .iter()
            .filter_map(|e| apps::entry_binary(e.app.as_deref(), &e.cmd))
            .collect::<Vec<_>>()
            .iter()
            .map(|s| s.as_str()),
        &std::env::var("PATH").unwrap_or_default(),
    );
```

and wrap the print line:

```rust
        let bin = apps::entry_binary(e.app.as_deref(), &e.cmd);
        let line = format!("{:<28} {:<44} {}", e.id, truncate(&e.title, 44), preview);
        if avail.available(bin.as_deref()) {
            println!("{line}");
        } else {
            println!("\x1b[2m{line}\x1b[0m");
        }
```

- [ ] **Step 2: `cmd_show` prints app + install**

At the end of `cmd_show`, after the `source:` line:

```rust
    if let Some(bin) = apps::entry_binary(e.app.as_deref(), &e.cmd) {
        if let Some(info) = apps::registry().get(&bin) {
            println!("app: {} ({})", info.name, info.binary);
            let avail = apps::Availability::scan(std::iter::once(bin.as_str()),
                &std::env::var("PATH").unwrap_or_default());
            if !avail.available(Some(&bin)) {
                match apps::install_for_platform(info) {
                    Some(cmd) => println!("install: {cmd}"),
                    None => println!("install: see {}", info.homepage),
                }
            }
        }
    }
```

- [ ] **Step 3: Manual verification (no unit test — thin printing glue over tested cores)**

```bash
cargo run --quiet -- search "resource dashboard"   # btop row dim iff btop absent
cargo run --quiet -- show blog-btop-resource-dashboard
```

Expected on a machine without btop: `app: btop (btop)` + `install: brew install btop`. With btop installed: the app line only. Record which case your machine showed in the report.

- [ ] **Step 4: Full suite + commit**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
git add src/main.rs
git commit -m "feat: search dims unavailable apps; show prints app and install hint"
```
