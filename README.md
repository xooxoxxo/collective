# collective

[![ci](https://github.com/xooxoxxo/collective/actions/workflows/ci.yml/badge.svg)](https://github.com/xooxoxxo/collective/actions/workflows/ci.yml)

A searchable directory of hacky, nerdy, super-functional developer commands —
with an interactive TUI and console flashcard training. One offline Rust binary,
~1600 curated + imported commands baked in.

```
collective                       # interactive TUI (filter, detail, favorites, prefill)
collective search "prevent sleep"
collective show pmset-disable-sleep
collective copy lsof-listening-port
collective random
collective drill --domain git    # SM-2 spaced-repetition flashcards
collective collect 'pmset -a disablesleep 1'   # add your own (AI or manual)
```

## Install

```sh
brew install xooxoxxo/tap/collective   # macOS + Linux binaries
# or from source:
cargo install --path .
```

Add `~/.cargo/bin` to your `PATH` if it isn't already.

### Shell prefill (recommended)

The TUI never runs a command for you — selecting one places it on your shell
prompt, editable, for you to run. That needs a tiny wrapper:

```sh
collective --print-shell zsh  >> ~/.zshrc     # or: --print-shell bash >> ~/.bashrc
```

Reload your shell. Now `collective`, pick with `↑/↓`, press `Enter`, and the
command lands on your prompt.

### Shell completions

```sh
collective completions zsh > ~/.zfunc/_collective   # ensure ~/.zfunc is in $fpath
collective completions bash > /usr/local/etc/bash_completion.d/collective
collective completions fish > ~/.config/fish/completions/collective.fish
```

## TUI keys

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

## collect

`collective collect '<command>'` captures a command into your personal overlay
at `~/.collective/corpus/`, so it shows up in search and drills immediately —
no rebuild. Fields are filled by AI or by hand:

- **AI**: `ANTHROPIC_API_KEY` → Anthropic API; else a local `claude` CLI
  (`claude -p`); else it falls back to manual entry. Model via
  `COLLECTIVE_MODEL` (default `claude-haiku-4-5-20251001`).
- **Manual**: prompts for each field.

## Safety

`collective` shows and copies commands — it never executes them. Dangerous
entries (`sudo`, destructive, irreversible) render a red banner and show their
undo. You review and run everything yourself.

## Corpus

- Hand-curated seed gems + a research-mined set across macOS internals,
  dotfiles, shell wizardry, and modern CLI tools.
- Bulk command reference imported from
  [tldr-pages](https://github.com/tldr-pages/tldr) (CC-BY-4.0) — see `NOTICE`.
- Every entry carries a `source`. Add your own with `collect`.

## Development

```sh
cargo test    # unit + integration
cargo build   # also re-validates every corpus YAML at build time
```

Corpus entries are YAML under `corpus/`, validated by `build.rs` at compile
time — an invalid entry fails the build. Design docs live in
`docs/superpowers/`.
