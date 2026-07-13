# Collective — Design Spec

Date: 2026-07-13
Status: Approved (brainstorming session)

## What

`collective` (binary: `col`) — a Rust CLI that is the ultimate directory / cheatsheet / glossary of hacky, nerdy, super-functional developer scripts (e.g. `pmset -a disablesleep 1`), with flashcard training sessions right in the console.

## Goals

- Searchable, offline, instant script directory in the terminal.
- Corpus of high-quality gems: non-obvious OR frequently forgotten. No `ls -la` filler.
- Spaced-repetition drills so commands stick.
- v1 scope: macOS tricks + shell one-liners + git + common dev CLIs.

## Non-Goals (v1)

- No command execution — `col` shows/copies only. No run mode, no sandbox needed.
- No website, no community PR pipeline (later phases).
- No Linux/Windows corpus (schema supports it via `platform`, corpus doesn't target it).

## Architecture

Rust CLI, single binary. Corpus = YAML files in `corpus/`, one file per entry, embedded into the binary at build time (`include_dir!`). User overlay at `~/.collective/corpus/` merged at load. No DB, no async — corpus is small (<5MB), loads in memory <50ms.

### Entry schema

```yaml
id: pmset-disable-sleep
title: Disable sleep entirely on macOS
cmd: pmset -a disablesleep 1
undo: pmset -a disablesleep 0        # optional
platform: [macos]
domains: [power, macos-admin]
danger: low|medium|high              # sudo? destructive?
explanation: >
  One-liner what/why
source: url                          # provenance
tags: [sleep, laptop, clamshell]
```

### Commands (v1)

| Command | Behavior |
|---|---|
| `col search <query>` | Fuzzy search title/tags/cmd, weighted (title 3x, tags 2x, cmd 1x). Top 10, one line each: `id  title  cmd-preview`. |
| `col show <id>` | Full entry: cmd, explanation, undo, danger, source. |
| `col copy <id>` | Copy cmd to clipboard. |
| `col drill [--domain X]` | Flashcard session (see Drill Mode). |
| `col random` | Random gem — drip-feed / shell-startup use. |

### Crates

`clap`, `serde_yaml`, `nucleo-matcher`, `arboard`, `crossterm`, `directories`.

### Danger display

`danger: high` entries render a red banner and show the `undo` command before the cmd itself. Nothing is ever auto-executed.

## Drill Mode

SM-2 spaced repetition. State in `~/.collective/drill.json` as `{entry_id: {ease, interval, due}}`.

Session: `col drill` picks ≤20 due cards. Shows title (e.g. "Prevent Mac sleeping"), user recalls/types, reveals cmd, self-grades 1–4 (again/hard/good/easy). Typed answer optionally diffed against cmd for exact-recall practice. `--domain git` filters card pool.

Corrupt/missing drill state → reset with a warning, never crash.

## Deep Research Plan (corpus harvest)

Runs after schema is locked. Two tracks.

### Track 1 — Dataset import (mechanical)

- tldr-pages (CC-BY): import macOS/common/git subset via converter script, note provenance + license per entry.
- cheat.sh / cheatsheets repo (MIT): dedupe against tldr.
- commandlinefu top-voted: license unclear per-entry — treat as inspiration list only unless verified.
- Expected: ~500–1500 normalized entries after filter + dedup.

### Track 2 — Gem mining (multi-agent research sweep)

Parallel research agents, one lens each; each returns entries **already in schema YAML** with source URL and danger rating:

1. **macOS internals** — `defaults write`, `pmset`, `networksetup`, `mdfind`, `caffeinate`, `tmutil`, `softwareupdate`, `xattr`, `codesign`. Sources: macos-defaults.com, HN, Apple forums, dotfiles.
2. **Dotfiles archaeology** — top-starred dotfiles (mathiasbynens, holman, paulirish); mine aliases/functions with clear intent.
3. **HN/Reddit threads** — recurring "best one-liners" threads (HN CLI tricks, r/commandline, r/macos); vote-signal as quality filter.
4. **Shell wizardry** — awk/sed/jq/xargs/find one-liners; process/port/network debugging (`lsof -i` etc.); git plumbing gems.
5. **Blog canon** — evergreen posts (Julia Evans, Brandur, "things I wish I knew" genre).

**Verify pass:** second agent per batch checks command validity on macOS (syntax, flag existence) and flags dangerous entries.

**Quality gate:** entry must be non-obvious OR frequently forgotten.

**Target:** ~300 gems from Track 2 + filtered Track 1 base. Deliverables: populated `corpus/` + `research-notes.md` with rejected-but-interesting leads.

## Testing

- Corpus schema validated at build time — build fails on bad YAML; bad entry never ships.
- Unit tests: schema parse, search ranking, SM-2 math.
- Integration test: `col search sleep` returns the pmset entry.

## Build Order

1. Schema + validator + 10 seed entries (pmset etc.)
2. `search` / `show` / `copy`
3. Track 1 import (tldr converter)
4. Track 2 research sweep (multi-agent)
5. `drill`
6. `random` + polish
