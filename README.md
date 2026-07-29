# collective

[![ci](https://github.com/xooxoxxo/collective/actions/workflows/ci.yml/badge.svg)](https://github.com/xooxoxxo/collective/actions/workflows/ci.yml)

A searchable, offline directory of developer commands you'd otherwise re-google —
then drill them with spaced repetition until you don't have to.

One Rust binary, no runtime, no network needed. ~150 curated commands are
compiled in and work the moment you install. Add packs for more.

```
collective                       # interactive TUI (filter, detail, favourites, prefill)
collective search "prevent sleep"
collective show pmset-disable-sleep
collective copy lsof-listening-port
collective random
collective drill --domain git    # type the command from memory; it checks you
collective collect 'pmset -a disablesleep 1'
collective pack add tldr         # +1459 tldr-pages entries
```

## Install

```sh
brew install xooxoxxo/tap/collective   # macOS + Linux
# or from source:
cargo install --path .
```

Add `~/.cargo/bin` to your `PATH` if installing from source.

### Shell prefill (recommended)

The TUI can put a command **on your prompt** instead of running it, so you can
edit before executing. That needs a shell function:

```sh
collective --print-shell zsh  >> ~/.zshrc
collective --print-shell bash >> ~/.bashrc
```

Then `collective`, pick an entry, press Enter — the command lands on your
command line, unexecuted.

### Shell completions

```sh
collective completions zsh  > "${fpath[1]}/_collective"
collective completions bash > /etc/bash_completion.d/collective
collective completions fish > ~/.config/fish/completions/collective.fish
```

## What makes it different

Most command references are lookup tools. This one is built around the idea that
looking a command up for the fifth time is the problem, not the solution.

- **Curated, not scraped.** Every shipped entry is hand-written with an
  explanation, a danger rating, and — where one exists — an **undo command**.
  The schema is enforced at build time: an entry missing a required field fails
  the build, so a malformed entry cannot ship.
- **You drill them.** `collective drill` is a spaced-repetition flashcard
  session where you type the command from memory and it checks your answer.
- **Offline and instant.** The starter corpus is compiled into the binary. No
  network, no API key, no daemon.
- **Your own commands win.** Anything you capture into your personal overlay
  overrides a shipped entry with the same id.

## Search

```sh
collective search "flush dns"
collective search git --domain vcs      # restrict to one domain
collective search port --curated        # exclude bulk pack imports
```

Fuzzy-matched and weighted: **title ×3, best-matching tag ×2, command ×1,
explanation ×1**. The explanation is searched too, so you can find an entry by
describing what it does rather than recalling its name.

Results are grouped — curated entries always rank above bulk pack imports, with
a `── tldr imports ──` separator between the groups. When both groups match, the
ten rows split 6 curated / 4 imports so a large pack can't crowd out the
hand-written entries.

## Interactive TUI

Run `collective` with no arguments. Every printable key types into the filter
(fzf-style), so actions live on Ctrl chords:

| key | action |
|---|---|
| any printable | type into the filter |
| `↑` / `↓` | move selection |
| `Enter` | prefill the command onto your shell prompt and exit |
| `Ctrl-Y` | copy to clipboard, stay open |
| `Ctrl-S` | star / unstar |
| `Ctrl-O` | show favourites only |
| `Ctrl-U` | show curated only |
| `Backspace` | delete a filter character |
| `Esc` / `Ctrl-C` | quit |

## Drill

```sh
collective drill
collective drill --domain git
```

A card shows the title; you type the command from memory. The answer is checked
**normalised** — formatting is forgiven, substance is not:

```
── Find the process listening on a port
your answer (or Enter to reveal): lsof -i :8080 -sTCP:LISTEN
  lsof -i :<port> -sTCP:LISTEN
  you typed: lsof -i :8080 -sTCP:LISTEN  ✓ correct
graded: good   [Enter accepts · 1-4 overrides]:
```

What counts as correct:

- **Whitespace collapses.** `git  log   --oneline` matches `git log --oneline`.
- **Flags may be reordered.** `git log -n5 --oneline` matches
  `git log --oneline -n5`.
- **Positional arguments may not.** `cp b a` does **not** match `cp a b` —
  their order carries meaning.
- **`<placeholder>` slots accept either form.** For `lsof -i :<port>`, both
  `:<port>` and `:8080` pass; an empty slot fails.
- **Case matters.** Shell commands are case-sensitive.

Get it wrong and it points at the first token that differs:

```
  you typed: ls -z /tmp  ✗ not quite
                ^ first difference
```

Your grade is derived from the result — a match grades *good*, a miss or a
reveal grades *again* — and Enter accepts it. Type `1`–`4` to override when the
checker is being stricter than you'd like. Scheduling uses SM-2 and persists in
`~/.collective/drill.json`.

Press Enter with no input to reveal a command you genuinely don't know; that
grades *again* and is the right move for long one-liners nobody should type from
memory.

## Packs

The binary ships a curated starter. Packs add more, and are ordinary JSON
documents fetched over HTTPS — nothing is unpacked and no shell is invoked.

```sh
collective pack search              # browse the registry
collective pack add tldr            # curated short name, via the registry
collective pack add owner/repo      # any repo publishing a pack.json
collective pack add ./local.json    # a local file
collective pack list
collective pack update              # refetch everything from its recorded origin
collective pack remove tldr
```

`pack add tldr` installs 1459 tldr-pages entries, restoring the full corpus.

**Publishing a pack** needs no registry entry: push a `pack.json` to any public
repo and it's installable as `owner/repo`. The format is one self-describing
document:

```json
{
  "manifest": { "name": "...", "version": "...", "description": "...",
                "source": "...", "license": "...", "count": 0 },
  "entries": [ { "id": "...", "title": "...", "cmd": "...",
                 "platform": ["macos"], "domains": ["shell"],
                 "danger": "low", "explanation": "...", "source": "..." } ]
}
```

The registry at
[xooxoxxo/collective-registry](https://github.com/xooxoxxo/collective-registry)
exists for discoverability — `pack search` needs an index — and to curate
official packs. Adding an entry there is a PR.

## Collect

```sh
collective collect 'pmset -a disablesleep 1'
collective collect --manual 'lsof -i :8080'   # skip AI, enter fields yourself
collective collect --last                     # capture the previous command
```

Saves to your personal overlay at `~/.collective/corpus/<id>.yaml`, which always
overrides shipped entries with the same id. Picked up on the next run — no
rebuild.

If `ANTHROPIC_API_KEY` is set (or the `claude` CLI is on your `PATH`), the
explanation, danger rating, and undo command are drafted for you; `--manual`
skips that entirely. `--last` needs the shell wrapper installed.

## Safety

Commands that can hurt you are marked, not hidden:

- Every entry carries a `danger` rating of `low`, `medium`, or `high`.
- High-danger entries print a red warning and their **undo command first**, on
  the theory that you should know your exit before you run something.
- Nothing is ever executed for you. `copy` copies, the TUI prefills — running it
  is always your keystroke.

## Corpus

Three layers, later winning over earlier:

```
embedded starter   ~152 entries, compiled into the binary
  ← packs          ~/.collective/packs/*.json
    ← your overlay ~/.collective/corpus/*.yaml
```

So a pack can override a shipped entry, and your own capture overrides both.
Within packs, the alphabetically later filename wins a duplicate id.

Entry schema: `id`, `title`, `cmd`, optional `undo`, `platform[]`, `domains[]`,
`danger`, `explanation`, `source`, optional `tags[]`. Ids are
`[a-z0-9-]` only — that charset is what keeps a pack from writing outside its
directory.

## Development

```sh
cargo test                                    # 129 tests
cargo clippy --all-targets -- -D warnings
cargo run -- search dns
```

The corpus is validated at build time: `build.rs` walks both `corpus/` and
`packs/`, parses every YAML file against the `Entry` schema, and fails the build
on a bad entry or a duplicate id across both trees.

Source layout:

| path | responsibility |
|---|---|
| `src/entry.rs` | the `Entry` schema and its validation |
| `src/corpus.rs` | three-layer loading (embedded < packs < overlay) |
| `src/search.rs` | weighted fuzzy search and group ranking |
| `src/answer.rs` | normalised answer matching for drills |
| `src/drill.rs` | the flashcard session loop |
| `src/sm2.rs` | SM-2 scheduling |
| `src/pack/` | pack types, parsing, store, fetch, registry |
| `src/tui/` | the interactive terminal UI |
| `src/collect.rs`, `src/ai.rs` | capture, with optional AI drafting |
| `src/bin/build-pack.rs` | turns a corpus directory into a `pack.json` |

Design documents live in `docs/superpowers/specs/`.

## Licence

MIT.
