# Collective v3 (frictionless loop) — Design Spec

Date: 2026-07-21
Status: Approved (brainstorming session)
Builds on: 2026-07-13 (v1 CLI), 2026-07-15 (v2 TUI + collect)

## What

Four small, coherent features that make the everyday loop and corpus growth
frictionless — no bloat, each reusing existing seams:

1. **`collect --last`** — capture the previous shell command from history.
2. **Placeholder filling** — commands with `<tokens>` prompt to fill before delivery/copy.
3. **Filters** — `search --domain`/`--curated`; TUI `c` curated-only toggle.
4. **Shell completions** — `collective completions <shell>`.

## Non-goals (explicit anti-bloat)

No in-app command execution, no plugin system, no config file, no theming, no
telemetry, no accounts. The submission server remains its own future project.

## Architecture

All four are thin additions over existing modules. New files: `src/placeholder.rs`.
Modified: `src/main.rs` (flags, dispatch, completions), `src/collect.rs`
(`--last`), `src/tui/mod.rs` (curated toggle, fill-on-deliver), `src/tui/ui.rs`
(help bar), `src/search.rs` (filter application, reuse `is_bulk_import`),
`shell/collective.zsh` + `.bash` (last-command capture), `tests/cli.rs`.
Dependency added: `clap_complete`.

---

## 1. collect --last

The binary cannot read the parent shell's history; the wrapper feeds it.

- `Cmd::Collect.command` becomes optional; add `#[arg(long)] last: bool`.
- Resolution in `collect::run`: if `--last`, read `COLLECTIVE_LAST_CMD` env; if
  set and non-empty, use it as the command; if unset/empty, error:
  `--last needs the shell wrapper — run 'collective --print-shell <shell>' and reload, or pass the command explicitly`.
  If neither `--last` nor a positional command is given, clap/`run` errors.
- Shell wrappers gain last-command capture. The emitted wrapper detects a
  `collect --last` invocation and exports `COLLECTIVE_LAST_CMD` from history
  before calling the binary:
  - zsh: `COLLECTIVE_LAST_CMD="$(fc -ln -1)"` (trimmed of leading blanks).
  - bash: `COLLECTIVE_LAST_CMD="$(history 1 | sed 's/^ *[0-9]* *//')"`.
  Users re-run the `--print-shell` install line once to pick this up.

## 2. Placeholder filling

New `src/placeholder.rs`, pure + testable:
- `tokens(cmd: &str) -> Vec<String>` — unique `<...>` tokens, first-seen order.
- `fill(cmd: &str, answers: &[(String, String)]) -> String` — substitute each
  `<token>` with its answer; a token with an empty answer is left as `<token>`.

Interactive helper (not unit-tested, stdin): `fill_interactive(cmd) -> String`
— no tokens → return unchanged; else prompt each token (`<name>: `) on stdin
and substitute; empty input leaves the token in place for later editing.

Wiring:
- TUI `Enter`: capture `picked`, exit loop, `restore()` the terminal, THEN run
  `fill_interactive` in cooked mode, THEN `deliver`. No raw-mode input widget.
- CLI `copy <id>`: `fill_interactive` before writing the clipboard.
- `show` unchanged (reference view).

## 3. Filters

- CLI `search` gains `--domain <d>` (retain entries whose `domains` contain `d`)
  and `--curated` (exclude entries where `is_bulk_import(e)` — the existing
  `tldr-import` predicate in `search.rs`). Filters apply to the entry set before
  weighted ranking; combinable.
- TUI: new `App` field `curated_only: bool`; key `c` toggles it; folded into
  `recompute()` alongside `fav_only` (drop entries where `is_bulk_import`). Help
  bar adds `c curated`.
- Reuse `is_bulk_import`; do not duplicate the predicate. Expose it `pub(crate)`
  if needed by the TUI module.

## 4. Shell completions

- `collective completions <zsh|bash|fish>` prints a completion script via
  `clap_complete::generate`, handled early in `main()` (like `--print-shell`),
  before corpus load. Add `clap_complete` dependency.
- README documents install (e.g. `collective completions zsh > ~/.zfunc/_collective`).

## Testing

- `placeholder.rs`: `tokens` (none / one / repeated collapses to one / multiple
  distinct); `fill` (all filled; empty answer leaves `<token>`).
- `search`: `--domain` narrows to matching domain; `--curated` excludes
  tldr-import entries.
- `tui::App`: `curated_only` toggle removes bulk imports from `visible()`.
- `collect --last`: unit — resolves command from `COLLECTIVE_LAST_CMD`, errors
  when unset; integration — env set + `--manual` writes an overlay entry.
- `completions`: integration — `collective completions zsh` exits 0 and emits
  `_collective`.
- Not tested (consistent with v1/v2): interactive stdin prompts, TUI rendering,
  clipboard, live AI calls.

## Done criteria

- `cargo test` green, zero warnings.
- `collect --last` (with wrapper) captures the prior command; clear error without it.
- A `<placeholder>` command prompts to fill before prefill/copy; no-token commands unaffected.
- `search --domain git --curated` returns only curated git entries; TUI `c` hides bulk imports.
- `collective completions zsh` emits a valid completion script.
- Existing commands and the TUI otherwise unchanged.
