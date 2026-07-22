# Collective v4 (release engineering + debt) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** CI on push/PR, tagged releases with 4 native binaries + auto-updated Homebrew tap, fzf-style TUI input (all letters typeable), and deterministic curated-first grouped search.

**Architecture:** Two GitHub Actions workflows plus a small formula-generator script; a key-dispatch rewrite in the TUI event loop (match on `(KeyCode, KeyModifiers)`); a sort-key change in `search::search` plus a CLI separator. No new modules, no new Rust dependencies.

**Tech Stack:** GitHub Actions, bash, Rust (ratatui/crossterm), Homebrew formula (Ruby).

## Global Constraints

- Binary `collective`, crate `collective`. Zero-warning build. Conventional commits. Commit after each task.
- Release targets exactly: `aarch64-apple-darwin` (macos-14), `x86_64-apple-darwin` (macos-13), `x86_64-unknown-linux-gnu` (ubuntu-latest), `aarch64-unknown-linux-gnu` (ubuntu-24.04-arm).
- Tap repo: `xooxoxxo/homebrew-tap`; formula path `Formula/collective.rb`; cross-repo push uses secret `TAP_GITHUB_TOKEN`.
- TUI keys after this change: printable chars type into filter; `Ctrl-Y` copy, `Ctrl-S` star, `Ctrl-O` fav-only, `Ctrl-U` curated-only, `Esc`/`Ctrl-C` quit. `q`-quit removed.
- Search: curated matches ALWAYS precede bulk imports (`is_bulk_import`); drop the ×2 multiplier; CLI separator `── tldr imports ──` only when both groups present.
- Version bump to `0.2.0`; first release tag `v0.2.0`.
- No new Rust dependencies.

## Current signatures (verified)

- `search::search<'a>(&'a [Entry], &str) -> Vec<(&'a Entry, u32)>`; scoring is `raw = 3*title + 2*tag + cmd`, then `s = raw * (curated?2:1)`, sort by score desc + id, truncate 10.
- `pub(crate) fn is_bulk_import(e: &Entry) -> bool` in `src/search.rs`.
- `cmd_search(entries, query, domain: Option<&str>, curated: bool)` in `src/main.rs` filters then calls `search::search`, prints rows.
- TUI event loop in `src/tui/mod.rs` matches `key.code` only (modifiers ignored — Ctrl-C currently hits `Char('c')`); imports `crossterm::event::{self, Event, KeyCode, KeyEventKind}`.
- Help bar string in `src/tui/ui.rs:92`.
- `Cargo.toml` version `0.1.0`.

## File Structure

```
.github/workflows/ci.yml        # NEW
.github/workflows/release.yml   # NEW
scripts/make-formula.sh         # NEW: version + artifact dir -> Formula ruby on stdout
src/tui/mod.rs                  # key dispatch rewrite
src/tui/ui.rs                   # help bar text
src/search.rs                   # grouped sort, drop multiplier, +unit test
src/main.rs                     # cmd_search separator
tests/cli.rs                    # separator integration tests
Cargo.toml                      # 0.2.0
README.md                       # badge, brew install, key table
```

---

### Task 1: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`
- Modify: `README.md` (badge)

**Interfaces:**
- Produces: green `ci` check on push/PR to main, macOS + Linux.

- [ ] **Step 1: Write `.github/workflows/ci.yml`**

```yaml
name: ci
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
jobs:
  test:
    strategy:
      matrix:
        os: [macos-14, ubuntu-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test
      - run: cargo build --release
```

- [ ] **Step 2: Add the badge to `README.md`**

Directly under the `# collective` heading line, add:

```markdown
[![ci](https://github.com/xooxoxxo/collective/actions/workflows/ci.yml/badge.svg)](https://github.com/xooxoxxo/collective/actions/workflows/ci.yml)
```

- [ ] **Step 3: Validate YAML locally**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('yaml ok')"`
Expected: `yaml ok`.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml README.md
git commit -m "ci: test and build on macOS and Linux"
```

---

### Task 2: fzf-style TUI input

**Files:**
- Modify: `src/tui/mod.rs` (imports + key dispatch), `src/tui/ui.rs` (help bar)

**Interfaces:**
- Consumes: existing `App` methods (`move_up/move_down/set_filter/toggle_star/toggle_fav_only/toggle_curated_only/selected_entry`) — unchanged.
- Produces: key dispatch matching `(KeyCode, KeyModifiers)`.

- [ ] **Step 1: Update the crossterm import in `src/tui/mod.rs`**

```rust
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
```

- [ ] **Step 2: Replace the key `match` in the event loop**

Replace the entire `match key.code { ... }` block with:

```rust
            match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => break,
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                (KeyCode::Up, _) => app.move_up(),
                (KeyCode::Down, _) => app.move_down(),
                (KeyCode::Enter, _) => {
                    if let Some(e) = app.selected_entry() {
                        picked = Some(e.cmd.clone());
                    }
                    break;
                }
                (KeyCode::Backspace, _) => {
                    let mut f = app.filter.clone();
                    f.pop();
                    app.set_filter(&f);
                }
                (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                    if let Some(e) = app.selected_entry() {
                        let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text(e.cmd.clone()));
                    }
                }
                (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                    if let Some(_id) = app.toggle_star() {
                        let _ = favorites::save(&fav_path, &app.favorites);
                    }
                }
                (KeyCode::Char('o'), KeyModifiers::CONTROL) => app.toggle_fav_only(),
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => app.toggle_curated_only(),
                // Everything printable types into the filter. SHIFT accompanies
                // uppercase chars, so allow it; any other modifier is ignored.
                (KeyCode::Char(ch), m)
                    if m.is_empty() || m == KeyModifiers::SHIFT =>
                {
                    let mut f = app.filter.clone();
                    f.push(ch);
                    app.set_filter(&f);
                }
                _ => {}
            }
```

(Delete the now-dead `KeyCode::Char('\n') => {}` arm along with the rest of the old block.)

- [ ] **Step 3: Update the help bar in `src/tui/ui.rs`**

Replace the help string:

```rust
    let help = Paragraph::new("↵ prefill  ^Y copy  ^S ★  ^O fav-only  ^U curated  Esc quit")
```

- [ ] **Step 4: Build + tests + manual key check**

Run: `cargo build && cargo test`
Expected: zero warnings; all tests pass (App tests untouched).

Manual (real terminal): bare `cargo run -q` — type `fcyqjk` into the filter (all appear), Ctrl-S stars, Ctrl-O/Ctrl-U toggle, Ctrl-Y copies, Esc and Ctrl-C both quit, terminal restored.

- [ ] **Step 5: Commit**

```bash
git add src/tui/mod.rs src/tui/ui.rs
git commit -m "feat: fzf-style TUI input — all letters type, actions on Ctrl chords"
```

---

### Task 3: Grouped search results

**Files:**
- Modify: `src/search.rs` (sort + drop multiplier + unit test), `src/main.rs` (`cmd_search` separator), `tests/cli.rs` (2 integration tests)

**Interfaces:**
- Consumes/Produces: `search::search` signature unchanged; ordering contract becomes: all curated hits precede all import hits; within a group best score first, id tie-break; 10 total.

- [ ] **Step 1: Write the failing unit test in `src/search.rs` tests module**

```rust
    #[test]
    fn curated_hits_all_precede_imports() {
        let entries = corpus::load();
        // "git log" matches curated gems and many tldr imports.
        let hits = search(&entries, "git log");
        assert!(!hits.is_empty());
        if let Some(first_import) = hits.iter().position(|(e, _)| is_bulk_import(e)) {
            assert!(
                hits[first_import..].iter().all(|(e, _)| is_bulk_import(e)),
                "found a curated hit after an import hit"
            );
            assert!(first_import > 0, "expected at least one curated hit first");
        }
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test search::`
Expected: `curated_hits_all_precede_imports` FAILS (a strong import can currently outrank a weak curated hit) — if it happens to pass on this query, strengthen by asserting on `search(&entries, "git")` too; the structural change is still required.

- [ ] **Step 3: Change the scoring/sort in `search::search`**

Replace the multiplier + sort section:

```rust
            let cmd = score_of(&e.cmd, &mut matcher);
            let raw = 3 * title + 2 * tag + cmd;
            (raw > 0).then_some((e, raw))
        })
        .collect();
    // Curated entries always precede bulk imports; within a group, best
    // score first, id as the tie-break. Deterministic — replaces the old
    // probabilistic x2 score boost.
    scored.sort_by(|a, b| {
        is_bulk_import(a.0)
            .cmp(&is_bulk_import(b.0))
            .then(b.1.cmp(&a.1))
            .then(a.0.id.cmp(&b.0.id))
    });
    scored.truncate(10);
    scored
```

Also update the function's doc comment:

```rust
/// Weighted fuzzy search: title 3x, best tag 2x, cmd 1x. Curated matches are
/// always grouped before bulk tldr imports. Top 10, best first within groups.
```

- [ ] **Step 4: Run unit tests**

Run: `cargo test search::`
Expected: all pass, including `curated_outranks_bulk_import` and the new grouping test.

- [ ] **Step 5: Add the CLI separator in `cmd_search` (`src/main.rs`)**

Replace the print loop:

```rust
    let has_curated = hits.iter().any(|(e, _)| !search::is_bulk_import(e));
    let mut sep_printed = false;
    for (e, _) in hits {
        if has_curated && !sep_printed && search::is_bulk_import(e) {
            println!("── tldr imports ──");
            sep_printed = true;
        }
        let preview: String = e.cmd.chars().take(48).collect();
        println!("{:<28} {:<44} {}", e.id, truncate(&e.title, 44), preview);
    }
```

- [ ] **Step 6: Write integration tests (`tests/cli.rs`)**

```rust
#[test]
fn search_prints_separator_between_groups() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "port"])
        .assert()
        .success()
        .stdout(predicates::str::contains("── tldr imports ──"));
}

#[test]
fn search_curated_output_has_no_separator() {
    let out = Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "port", "--curated"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("── tldr imports ──"));
}
```

- [ ] **Step 7: Build + full test**

Run: `cargo build && cargo test`
Expected: zero warnings; all pass. If `search_prints_separator_between_groups` fails because "port" fills all 10 slots with curated hits, switch the query to "list" (huge import surface) and re-verify.

- [ ] **Step 8: Commit**

```bash
git add src/search.rs src/main.rs tests/cli.rs
git commit -m "feat: group curated results before tldr imports with CLI separator"
```

---

### Task 4: Release workflow, formula generator, version 0.2.0, README

**Files:**
- Create: `.github/workflows/release.yml`, `scripts/make-formula.sh`
- Modify: `Cargo.toml` (version), `README.md` (brew install + key table)

**Interfaces:**
- Produces: on tag `v*` — GitHub release with 4 tarballs + sha256s, and `Formula/collective.rb` pushed to `xooxoxxo/homebrew-tap` using secret `TAP_GITHUB_TOKEN`.

- [ ] **Step 1: Write `scripts/make-formula.sh`**

```bash
#!/usr/bin/env bash
# Emit the Homebrew formula for a release on stdout.
# Usage: make-formula.sh <version> <artifact-dir>
# <artifact-dir> must contain collective-<version>-<target>.tar.gz.sha256 files.
set -euo pipefail
VERSION="$1"
DIR="$2"

sha() { cut -d' ' -f1 <"$DIR/collective-${VERSION}-$1.tar.gz.sha256"; }
url() { echo "https://github.com/xooxoxxo/collective/releases/download/v${VERSION}/collective-${VERSION}-$1.tar.gz"; }

cat <<EOF
class Collective < Formula
  desc "Searchable directory of developer commands with TUI and flashcards"
  homepage "https://github.com/xooxoxxo/collective"
  version "${VERSION}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "$(url aarch64-apple-darwin)"
      sha256 "$(sha aarch64-apple-darwin)"
    else
      url "$(url x86_64-apple-darwin)"
      sha256 "$(sha x86_64-apple-darwin)"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "$(url aarch64-unknown-linux-gnu)"
      sha256 "$(sha aarch64-unknown-linux-gnu)"
    else
      url "$(url x86_64-unknown-linux-gnu)"
      sha256 "$(sha x86_64-unknown-linux-gnu)"
    end
  end

  def install
    bin.install "collective"
  end

  test do
    assert_match "collective", shell_output("#{bin}/collective --help")
  end
end
EOF
```

Run: `chmod +x scripts/make-formula.sh`

Note: the repo has no LICENSE file yet; the formula claims MIT. Add `LICENSE` (MIT, copyright the repo owner) in this task — a public repo with release artifacts needs one anyway:

Create `LICENSE` with the standard MIT text, `Copyright (c) 2026 xooxoxxo`.

- [ ] **Step 2: Test the generator locally with fake checksums**

```bash
D=$(mktemp -d)
for t in aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
  echo "deadbeef  collective-0.2.0-$t.tar.gz" > "$D/collective-0.2.0-$t.tar.gz.sha256"
done
bash scripts/make-formula.sh 0.2.0 "$D" | ruby -c /dev/stdin 2>/dev/null || bash scripts/make-formula.sh 0.2.0 "$D" | head -12
```

Expected: formula prints with all four URL/sha blocks filled (`ruby -c` says Syntax OK if ruby available; otherwise eyeball the head).

- [ ] **Step 3: Write `.github/workflows/release.yml`**

```yaml
name: release
on:
  push:
    tags: ["v*"]
permissions:
  contents: write
jobs:
  build:
    strategy:
      matrix:
        include:
          - os: macos-14
            target: aarch64-apple-darwin
          - os: macos-13
            target: x86_64-apple-darwin
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --release
      - name: package
        run: |
          VERSION=${GITHUB_REF_NAME#v}
          mkdir -p dist
          cp target/release/collective dist/
          tar -czf "collective-${VERSION}-${{ matrix.target }}.tar.gz" -C dist collective
          shasum -a 256 "collective-${VERSION}-${{ matrix.target }}.tar.gz" > "collective-${VERSION}-${{ matrix.target }}.tar.gz.sha256"
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.target }}
          path: collective-*.tar.gz*
  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with:
          path: artifacts
          merge-multiple: true
      - name: create GitHub release
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          gh release create "$GITHUB_REF_NAME" artifacts/* \
            --title "$GITHUB_REF_NAME" --generate-notes
      - name: update Homebrew tap
        env:
          TAP_TOKEN: ${{ secrets.TAP_GITHUB_TOKEN }}
        run: |
          VERSION=${GITHUB_REF_NAME#v}
          bash scripts/make-formula.sh "$VERSION" artifacts > collective.rb
          git clone "https://x-access-token:${TAP_TOKEN}@github.com/xooxoxxo/homebrew-tap" tap
          mkdir -p tap/Formula
          cp collective.rb tap/Formula/collective.rb
          cd tap
          git config user.name "collective-release-bot"
          git config user.email "noreply@github.com"
          git add Formula/collective.rb
          git commit -m "collective ${VERSION}"
          git push
```

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('yaml ok')"`
Expected: `yaml ok`.

- [ ] **Step 4: Bump the version**

`Cargo.toml`: `version = "0.1.0"` → `version = "0.2.0"`. Run `cargo build` once so `Cargo.lock` picks it up.

- [ ] **Step 5: Update `README.md`**

In the Install section, add brew as the first option:

```markdown
```sh
brew install xooxoxxo/tap/collective   # macOS + Linux binaries
# or from source:
cargo install --path .
```
```

Replace the TUI keys table rows to match the new bindings:

```markdown
| key | action |
|-----|--------|
| type | live fuzzy filter (every letter types) |
| `↑`/`↓` | move selection |
| `Enter` | prefill selected command to your shell + copy |
| `Ctrl-Y` | copy to clipboard, stay open |
| `Ctrl-S` | toggle favorite (persisted) |
| `Ctrl-O` | show favorites only |
| `Ctrl-U` | show curated only (hide tldr imports) |
| `Esc` / `Ctrl-C` | quit |
```

- [ ] **Step 6: Build + full test**

Run: `cargo build && cargo test`
Expected: zero warnings; all pass.

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/release.yml scripts/make-formula.sh LICENSE Cargo.toml Cargo.lock README.md
git commit -m "feat: release workflow, Homebrew formula generator, v0.2.0"
```

---

### Task 5: Release ops (controller/human — not a subagent task)

Ordered checklist; steps 2–3 involve the human.

- [ ] 1. Create the tap repo: `gh repo create xooxoxxo/homebrew-tap --public` (as `oytuneyucel`), with a stub README.
- [ ] 2. **Human:** mint a PAT with `repo` scope on `xooxoxxo/homebrew-tap` (fine-grained: contents read/write on that repo only).
- [ ] 3. Set the secret: `gh secret set TAP_GITHUB_TOKEN --repo xooxoxxo/collective` (paste the PAT).
- [ ] 4. Merge `feat/v4-release-and-debt` → `main` (tests green), push main. CI runs — must be green on both OSes before tagging.
- [ ] 5. Tag: `git tag v0.2.0 && git push origin v0.2.0`. Release workflow runs.
- [ ] 6. Verify: release page has 4 tarballs + 4 checksums; `Formula/collective.rb` landed in the tap; `brew install xooxoxxo/tap/collective && collective --help` works locally.

## Done Criteria

- CI green (macOS + Linux) on main.
- `v0.2.0` release with 4 binaries + checksums; formula in tap; `brew install xooxoxxo/tap/collective` works.
- TUI: every printable char typeable in the filter; Ctrl chords act; Esc/Ctrl-C quit.
- `collective search port` shows curated hits, `── tldr imports ──`, then imports; `--curated` output has no separator.
- Full suite green, zero warnings.
