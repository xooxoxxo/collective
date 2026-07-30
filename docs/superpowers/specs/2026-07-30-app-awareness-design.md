# App awareness — Design Spec

Date: 2026-07-30
Status: Approved (brainstorming session)
Relates to: the `collective` CLI at v0.4.0
Companion: a second spec (pack reorganization) follows this one and depends on
the `app:` field and registry introduced here.

## What

Many corpus entries need a specific application (`btop`, `rg`, `fzf`, …) that
may not be installed. collective learns which app an entry needs, whether that
app is present on PATH, and what to do about it when it is not: gray the entry,
filter it, skip it in drills, and offer the app's info and install command from
inside the TUI.

## Why

An entry you cannot run is noise when you are searching and actively harmful
when you are drilling — you cannot build recall of a tool you don't have. The
same metadata that powers graying also answers the natural next question,
"how do I get it?", without leaving the terminal.

## 1. App registry — `corpus/apps.yaml`

One hand-curated file, embedded in the binary alongside the corpus.

```yaml
apps:
  - binary: rg
    name: ripgrep
    description: Recursively search directories with regex, gitignore-aware
    homepage: https://github.com/BurntSushi/ripgrep
    install:
      brew: brew install ripgrep
      apt: apt install ripgrep
```

- `binary`, `name`, `description`, `homepage` required; `install` optional,
  with optional `brew` and `apt` keys only (no other managers now).
- Validated in `build.rs` like entries: bad registry = no binary. Duplicate
  `binary` values are a build error.
- Every entry `app:` value must name a registered binary — build error
  otherwise. The reverse is not required: registry apps with no current entry
  are allowed (spec 2 will add entries).
- Initial population: the apps the embedded corpus and gems actually
  reference (roughly 30–50). tldr-pack apps are spec 2's work.

## 2. Entry schema — optional `app:`

`Entry` gains `app: Option<String>` (`#[serde(default)]`), the binary name.

When absent, the app is **derived** from `cmd`:

1. Tokenize on whitespace; skip leading `sudo`, `env`, and any `VAR=value`
   assignment tokens.
2. The next token is the candidate binary (basename it if it contains `/`).
3. If the candidate is on the shell-builtin allowlist (`cd`, `export`,
   `alias`, `set`, `unset`, `source`, `eval`, `echo`, `read`, `trap`,
   `ulimit`), the entry has **no app**.
4. Otherwise the candidate is the entry's binary — whether or not it appears
   in the registry.

The explicit field exists for entries where derivation is wrong (a `git
config` command that is really about `delta`, a pipeline whose interesting
tool is not the first token). Existing entries are not mass-edited; the field
is added where derivation misleads.

Registry membership and availability are independent: availability is checked
for any derived or declared binary; the registry additionally powers the app
pane (info + install).

## 3. Availability — in-process PATH scan

At startup of the TUI, a drill session, or a CLI search:

- Collect the unique binaries across loaded entries.
- For each, walk the directories in `$PATH` once and check for a regular file
  with the executable bit (`std::fs`, no subprocesses). One pass builds a
  `HashMap<String, bool>` for the run; a few hundred lookups cost
  milliseconds.
- Entries with no app (builtins, empty derivation) are always available —
  never gray falsely.
- No persistent cache. A fresh scan per run means installing an app is
  reflected the next time collective starts, which is exactly when it
  matters.

## 4. TUI behavior

- **Graying:** unavailable entries render their title and cmd in DarkGray.
  They remain searchable and selectable.
- **Filter:** `^T` toggles available-only, alongside `^O` (favorites) and
  `^U` (curated). Help bar gains both new keys.
- **App pane:** `^A` on the selected entry opens a pane showing the
  registered app's `name`, `description`, `homepage`, and the install command
  for the current platform — `brew` on macOS, `apt` on Linux; if the platform
  key is missing, the pane says `install: see homepage`.
  - **Enter** prefills the install command via the existing shell-wrapper
    path, exactly like selecting an entry's command.
  - **o** opens the homepage with `open <url>` (macOS) / `xdg-open` (Linux).
  - **Esc** closes the pane.
  - For an entry whose binary is not in the registry (or has no app), the
    pane is a one-line notice: `no app info for <binary>` / `built-in
    command`.

## 5. Drill behavior

The due-card queue drops entries whose app is unavailable before the session
starts. The session header reports it: `3 skipped (app not installed)`.
SM-2 state is untouched — skipped cards are not graded, their schedule
simply waits until the app exists.

## 6. CLI surfaces

- `collective search`: unavailable rows print with ANSI dim.
- `collective show <id>`: gains an `app:` line (registered name + binary)
  when the entry has one; when the app is missing from PATH, also prints
  `install: <platform command>` (or the homepage when no install string
  exists for this platform).

## 7. Testing

- Derivation table test: plain cmd, `sudo`, `env`, `VAR=val` prefixes,
  path-qualified binaries, builtins, empty cmd.
- PATH scan against temp directories: present with executable bit, present
  without it, absent, and a builtin (always available).
- Registry validation: missing required field, duplicate binary, entry
  `app:` referencing an unregistered binary — each fails the build-time
  validator.
- Drill filtering: due queue with a mix of available/unavailable apps skips
  the right cards and reports the count; SM-2 state unchanged for skipped.
- TUI: render assertion that an unavailable row carries the DarkGray style;
  filter toggle reduces the visible set.
- Process rule from the repo's handoff: every new test gets a falsification
  pass — break the line it depends on, confirm it fails, restore.

## Out of scope (spec 2: pack reorganization)

- Splitting the tldr pack to default-installed tools only, and the curated
  base-system allowlist that defines "default-installed".
- Purpose-bundle packs (modern-cli, git-extras, containers, …).
- Registry entries for apps only the tldr pack references.
- Any registry/packaging format changes.
