# Collective Interactive (TUI + collect) — Design Spec

Date: 2026-07-15
Status: Approved (brainstorming session)
Builds on: 2026-07-13-collective-cli-design.md (v1)

## What

Two v2 features for `collective`:

1. **Interactive TUI** — bare `collective` (no subcommand) launches a full-screen
   ratatui interface over the 1611-entry corpus: live fuzzy filter, scrollable
   table, detail pane showing the full command, star favorites, and
   select-to-prefill that drops the chosen command onto the user's shell prompt.
2. **`collective collect '<command>'`** — capture a command into the user's
   overlay corpus, with fields populated by AI (Anthropic API or local `claude`
   CLI) or entered manually.

Existing subcommands (`search`/`show`/`copy`/`random`/`drill`) are unchanged and
remain available for scripting.

## Why

v1 output truncates commands and offers no way to act on a result. Users want to
see full detail, operate on entries, keep favorites, and grow the corpus with
their own commands. The name collision with `/usr/bin/col` is already resolved
(binary is `collective`).

## Preserved v1 safety property

The TUI **never executes** a corpus command. "Select" means prefill-to-prompt +
clipboard, so `sudo`, destructive, and `<placeholder>` commands land on the
user's editable shell prompt for review. The shell prompt is the confirm gate;
no auto-execution anywhere.

## Architecture

New view + capture layers over existing `corpus::load()` and `search::search()`.
No changes to entry schema semantics. `Entry` gains a `Serialize` derive (was
Deserialize-only) so collected entries can be written back as YAML.

### Files

```
src/main.rs        # bare invocation -> tui::run(); collect subcommand;
                   # --print-shell <zsh|bash>; --print-cmd plumbing via $COLLECTIVE_PICK
src/tui/mod.rs     # event loop, App state, key dispatch, terminal setup/teardown
src/tui/ui.rs      # ratatui rendering: filter box, table, detail pane, help bar
src/favorites.rs   # ~/.collective/favorites.json load/save (drill.rs pattern)
src/collect.rs     # collect flow: prompt, assemble Entry, id uniquify, write overlay
src/ai.rs          # populate(cmd) -> Result<AiFields>; backend selection + JSON parse
src/entry.rs       # add #[derive(Serialize)] to Entry
shell/collective.zsh
shell/collective.bash
```

Dependencies added: `ratatui`, `ureq` (tiny blocking HTTP). `crossterm`,
`serde_json`, `directories` already present.

## TUI

### Layout

```
┌ collective ──────────────────────────────── 1611 ┐
│ filter> prevent sleep_                            │
├───────────────────────────────────────────────────┤
│ ★ id                      title            danger │
│   mac-prevent-sleep…      Prevent Mac…     med    │
│ > dot-dock-autohide       Auto-hide Dock   low    │
├─ detail ──────────────────────────────────────────┤
│ Auto-hide Dock and remove animation delay          │
│ cmd:  defaults write com.apple.dock autohide …     │
│ undo: defaults delete com.apple.dock autohide      │
│ domains: macos-admin   danger: low                 │
│ explanation … source …                             │
└─ ↵ prefill  y copy  f ★  F fav-only  / filter  q ─┘
```

Detail pane shows the FULL command, wrapped (fixes v1 truncation). `danger:high`
rows render red.

### Keys

| Key | Action |
|---|---|
| (type) | live fuzzy filter |
| `↑`/`↓`, `j`/`k` | move selection |
| `Enter` | select → prefill to shell + copy, exit |
| `y` | copy selected cmd to clipboard, stay open |
| `f` | toggle star on selected (persists immediately) |
| `F` | toggle favorites-only view |
| `Esc`/`q` | quit without selecting |

### State

```
App {
    all: Vec<Entry>,
    filtered: Vec<usize>,      // indices into all, after filter
    selected: usize,           // index into filtered
    filter: String,
    favorites: HashSet<String>,
    fav_only: bool,
}
```

Empty filter → all entries (sorted by id). Non-empty → `search::search` ordering.
Filter and star/fav-only mutations are pure functions on `App`, unit-tested
without a terminal. Terminal is restored (raw mode off, leave alt-screen) on
every exit path including panic, via a panic hook.

## Prefill mechanism

On `Enter`, the binary writes the chosen `cmd` to the file named by
`$COLLECTIVE_PICK` (if set) and copies it to the clipboard.

Shell wrappers read that file and place the command on the prompt:

```zsh
collective() {
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && print -z "$cmd"
}
```
```bash
collective() {
  local pick; pick=$(mktemp)
  COLLECTIVE_PICK="$pick" command collective "$@"
  local cmd; cmd=$(cat "$pick"); rm -f "$pick"
  [[ -n "$cmd" ]] && { READLINE_LINE="$cmd"; READLINE_POINT=${#cmd}; }
}
```

Without the wrapper (bare binary), select still copies to clipboard and prints
the command on exit — graceful degradation. Install is one line:
`collective --print-shell zsh >> ~/.zshrc`. Both wrappers also ship in `shell/`.

## Favorites

`src/favorites.rs`, mirroring `drill.rs`:
- `~/.collective/favorites.json` — JSON array of ids, written sorted.
- `load() -> HashSet<String>` — missing → empty; corrupt → warn + empty, never crash.
- `save(&HashSet<String>) -> io::Result<()>` — creates parent dir.

`f` toggles selected id and saves write-through (crash never loses a star). `F`
toggles `fav_only`. Favorites reference ids; a favorited id with no matching
entry is skipped at render, no error.

## collect

`collective collect '<command>'` — command is the argument.

Flow:
1. Prompt: `Populate with AI, or fill in manually? [a/m]` (`--manual` flag skips
   straight to manual; used by the integration test).
2. **AI** (`a`): `ai::populate(cmd)` returns `{title, domains, danger,
   explanation, tags, undo, platform}`. Show the assembled entry; `Enter`
   accepts, `e` re-opens a chosen field for manual edit.
3. **Manual** (`m`): prompt each field in turn with sensible defaults
   (`platform` default `[macos]`, `undo` default empty).
4. id = slugified title, uniquified against corpus + overlay (`-2`, `-3` …).
5. `Entry::validate()` must pass before writing.
6. Write `~/.collective/corpus/<id>.yaml`. `source` = `collect:<hostname>`
   (placeholder provenance until the future server assigns GitHub-signed origin).

### AI backend (`src/ai.rs`)

`populate(cmd) -> Result<AiFields>` selects a backend in order:
1. `ANTHROPIC_API_KEY` set → direct Anthropic API via `ureq` (POST /v1/messages).
2. else `claude` on PATH → shell out
   `claude -p '<prompt>' --output-format json --model <model>`, parse `.result`
   for the model's JSON.
3. else → return an error the caller turns into a manual-entry fallback with a
   printed note.

`COLLECTIVE_MODEL` (default `claude-haiku-4-5-20251001`) feeds both the API
`model` field and `claude --model`. The prompt instructs the model to return
strict JSON with the seven fields. Malformed JSON or transport error → `Err`,
and `collect.rs` falls back to manual (the command is never lost). Backend
selection and JSON parsing are pure/injectable and unit-tested; no live API
calls in tests.

## Error handling

- Favorites / overlay corrupt or missing → warn + continue, never crash.
- AI: no key + no `claude` → clean manual fallback; API/CLI error or malformed
  JSON → print error, manual fallback.
- TUI restores terminal state on every exit path including panic (panic hook).

## Testing

- `favorites.rs`: missing→empty, roundtrip, corrupt→reset.
- `collect.rs`: id slugify + uniquification; assembled Entry passes
  `Entry::validate`; written YAML round-trips to an equal Entry.
- `ai.rs`: backend selection (key→Api; no key + fake `claude`→Cli; neither→err);
  JSON parse (well-formed→fields, malformed→Err). No live calls.
- TUI `App`: filter updates `filtered`; star toggle mutates set + persists;
  `fav_only` filters. Pure state, no terminal.
- Integration: `collective collect '<cmd>' --manual` with piped stdin writes a
  valid overlay file; `collective --print-shell zsh` emits the wrapper.

Not tested: live AI calls, terminal rendering, clipboard (GUI) — consistent
with v1.

## Out of scope (future project)

The **submission server**: users push their overlay corpus to a service; weekly
curation selects and showcases new additions; every submission is signed with
the submitter's GitHub account (non-anonymous provenance). Not designed here.
The `source: collect:<host>` field and overlay-as-staging are the seams that
make it possible later; it gets its own spec → plan cycle when the time comes.
