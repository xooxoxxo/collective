# Collective v5 (packs + registry) — Design Spec

Date: 2026-07-28
Status: Approved (brainstorming session)
Builds on: v1 (CLI), v2 (TUI + collect), v3 (frictionless loop), v4 (release + grouped search)

## What

The binary stops shipping the bulk corpus. It keeps a small embedded **starter**
(~152 curated entries) and gains a `pack` subcommand that fetches additional
corpora on demand from a JSON registry.

Three pieces:

1. **Corpus split** — `corpus/imported/` (1459 tldr entries) leaves the embedded
   build tree. The starter is `corpus/` root (10) + `corpus/gems/` (142).
2. **Pack format + loader** — a pack is one self-describing JSON file, fetched
   over HTTPS, stored in `~/.collective/packs/`, merged into the corpus at
   runtime between the embedded starter and the user overlay.
3. **Two ways to name a pack** — a curated short name resolved through a static
   `registry.json` in `xooxoxxo/collective-registry`, or an `<owner>/<repo>`
   source address resolved directly on GitHub with no registry entry at all.
   `pack add/list/search/update/remove`.

Blessed consequence: a fresh install searches ~152 curated gems. `collective
pack add tldr` restores the full corpus.

## Decided: single-file packs, not tarballs

Two candidates were red-teamed before this spec was written.

**Design A** — pack is a `.tar.gz` of many YAML files; `pack add` shells out to
`curl` and `tar`.

**Design B (chosen)** — pack is one JSON file; `pack add` fetches it with
`ureq` and writes it verbatim.

Design B wins on both axes:

- **`ureq` is already a dependency** (`src/ai.rs:111` calls the Anthropic API
  over HTTPS). Design A's original justification — "shelling out avoids adding
  an HTTP client" — was false. Neither design adds a dependency.
- **Design B deletes two vulnerability classes rather than mitigating them.**
  No `argv` is constructed and no external binary is invoked, so argument
  injection (a URL beginning with `-` reaching `curl`/`tar` as a flag) cannot
  occur. No archive is unpacked, so archive path traversal, absolute-path
  entries, symlink escape, and zip-slip variants cannot occur — there are no
  attacker-supplied path names anywhere in the flow. Design A closes these with
  validation code; Design B has nowhere for them to live.

Accepted costs of B: the pack (~549 KB for tldr) is buffered in memory during
install; an installed pack cannot be hand-edited per entry; a pack is a flat
entry list and cannot ship a directory tree. None matter at v5 scope. Revisit
tarballs only if a pack exceeds ~10 MB or needs per-file granularity.

## 1. Corpus split and repo layout

`corpus/imported/` moves to `packs/tldr/` — still in git, no longer embedded.

```
corpus/            # embedded via include_dir! — the starter
  *.yaml           #   10 root entries
  gems/*.yaml      #   142 curated gems
packs/
  tldr/*.yaml      # 1459 entries — source of truth, NOT embedded
```

This kills the chicken-and-egg problem: the pack artifact is generated from
files that stay under version control, so the tldr pack can always be rebuilt.

`build.rs` currently walks `corpus/` only. It must walk **both** `corpus/` and
`packs/`, applying the same schema validation and global duplicate-id check to
each. Only `corpus/` is embedded (`include_dir!` is unchanged). A malformed
pack entry therefore still fails the build, exactly as today.

## 2. Pack file format

One JSON file per pack: `{ "manifest": {...}, "entries": [...] }`.

```json
{
  "manifest": {
    "name": "tldr",
    "version": "1.0.0",
    "description": "tldr-pages bulk import",
    "source": "https://github.com/tldr-pages/tldr",
    "license": "CC-BY-4.0",
    "count": 1459
  },
  "entries": [ { "id": "...", "title": "...", "cmd": "...", ... } ]
}
```

`entries` are the existing `Entry` type verbatim — same serde derive, same
`deny_unknown_fields`, same `Entry::validate()`. No new entry schema.

JSON rather than YAML because packs are machine-generated, never hand-edited,
and parsed on every CLI invocation; `serde_json` is already a dependency and
parses several times faster than `serde_yaml`.

## 3. Two ways to name a pack

A pack is identified either by a **curated short name** resolved through a
registry, or by a **`<owner>/<repo>` source address** resolved directly on
GitHub. The second form follows the model `vercel-labs/skills` uses for skill
distribution, where the repository is the unit and no central index exists.

**Short name** — `registry.json` at a fixed raw URL in
`xooxoxxo/collective-registry`:

```json
{ "packs": [
  { "name": "tldr", "description": "...", "license": "CC-BY-4.0",
    "count": 1459, "url": "https://…/tldr.json" }
] }
```

**Source address** — `<owner>/<repo>` resolves by convention to
`https://raw.githubusercontent.com/<owner>/<repo>/HEAD/pack.json`. No registry
entry, no release asset, no publish step: pushing `pack.json` to a repo
publishes the pack.

Both forms end in the same place — one HTTPS GET of one JSON file — so §5's
pipeline and its guarantees are identical for either. The registry buys
discoverability (`pack search` needs something to search) and curation; the
source address buys zero-friction third-party publishing.

**No sha256 field.** A checksum published in the same registry, by the same
owner, that points at an asset that owner controls, provides no independent
integrity guarantee: an attacker able to swap the asset can edit the checksum in
the same breath. HTTPS already covers transport integrity. Adding sha256 here
would be ceremony that reads as a security property without being one. If pack
publishing is ever opened beyond the registry owner, revisit with real signing.

## 4. Commands

| command | behavior |
|---|---|
| `pack list` | installed packs from disk: name, version, entry count, origin |
| `pack search [query]` | fetch `registry.json`, filter by name/description |
| `pack add <name>` | resolve via registry, fetch, validate, install |
| `pack add <owner>/<repo>` | resolve via raw.githubusercontent, same pipeline |
| `pack add <path.json>` | install from a local file, same validation |
| `pack update [name]` | refetch from the recorded origin and reinstall |
| `pack remove <name>` | delete `~/.collective/packs/<name>.json` |

Install accepts a registry name, an `<owner>/<repo>` pair, or a local path.
Arbitrary URLs remain unaccepted: every remote fetch targets either a URL the
registry owner published or a path under `raw.githubusercontent.com`.

`pack update` **always refetches** rather than comparing versions. Version
comparison would need a registry lookup that the `<owner>/<repo>` form cannot
do, so refetching is both the simpler code path and the only one that works for
both source types. `manifest.version` is display metadata, not update logic.

## 5. Install pipeline and its security requirements

`pack add` performs, in order:

1. **Classify the argument, then validate the on-disk pack name.** The argument
   is a local path if it ends in `.json`, an `<owner>/<repo>` pair if it contains
   exactly one `/`, otherwise a registry short name. The **on-disk pack name** —
   the short name, or `manifest.name` for the other two forms — must match
   `^[a-z0-9-]+$` before it is used to build any path. `Path::join` does not
   neutralize `..`, so an unchecked name such as `../../.zshrc` escapes the
   packs directory on both write and remove. This is the single most important
   requirement in this spec and applies to `add`, `update`, and `remove` alike.
   Note that `manifest.name` is publisher-controlled and reaches the filesystem,
   so it is validated on exactly the same terms as a name the user typed.
2. **Resolve to a URL.** A short name is looked up in `registry.json`. An
   `<owner>/<repo>` argument — recognized by containing exactly one `/` — is
   validated against `^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$` and interpolated into
   the raw.githubusercontent path. That charset excludes `/` and therefore
   cannot contain `..`, so neither segment can walk out of the intended path.
   Reject any resolved URL whose scheme is not `https`.
3. **Fetch with a configured `ureq::Agent`** — explicit connect and read
   timeouts, redirects left at ureq's default cap of 5 (GitHub release assets
   302 to `objects.githubusercontent.com`, so redirects cannot be disabled).
4. **Bound the response** by reading through `into_reader().take(N)` rather than
   trusting the `content-length` header, which a hostile server can understate.
   `N` = 32 MB. Without this, `into_json()` on an unbounded body is an OOM.
5. **Deserialize and validate** — parse the JSON, then run `Entry::validate()`
   on every entry. Reject the whole pack if the manifest is missing or its
   `name` disagrees with the requested name.
6. **Warn on shadowing** — if any incoming id collides with an embedded starter
   id, print the colliding ids. A pack silently redefining a known id such as
   `flush-dns-cache` with a hostile `cmd` is the one content attack the merge
   order makes possible; naming it at install time is the cheap defense.
7. **Refuse a cross-origin overwrite.** Two sources can claim the same pack
   name — `pack add someone/tldr` whose manifest says `"name": "tldr"` would
   land on the official `tldr.json`. On install, record the resolved fetch URL
   as an `origin` field written by the CLI (distinct from the publisher-authored
   `manifest.source`, which is advisory and may lie). If a pack of that name is
   already installed with a different `origin`, abort and tell the user to
   `pack remove <name>` first. Same origin overwrites freely — that is what
   `pack update` is.
8. **Write atomically** — write to a temp file in the same directory, then
   `fs::rename` into place. A single rename syscall also makes two concurrent
   `pack add` runs safe without a lock, and leaves no truncated pack behind on
   interrupt or disk-full.

### Accepted residual risks

- **Registry compromise.** An attacker who controls the registry repo controls
  what a pack contains. No transport measure addresses this; signing is the
  only real answer and is out of scope.
- **Third-party packs are a user trust decision.** `pack add <owner>/<repo>`
  installs a stranger's command corpus, exactly as `brew tap`, `cargo install
  --git`, and `npx skills add` do. The trust judgment belongs to the person
  typing the name and cannot be delegated to the tool. What the tool owes them
  is honesty about what arrived: the shadowing warning (step 6) and the origin
  shown by `pack list` exist for this case, and matter more here than for
  registry packs.
- **Redirect scheme downgrade.** ureq 2.12.1 does not refuse an https→http
  redirect. Reaching one requires already controlling the registry URL, at
  which point the attacker would publish a hostile https URL directly — so the
  redirect path grants no capability the compromise did not already grant.
- **`cmd` content is arbitrary by design.** `Entry::validate()` checks that
  `cmd` is non-empty, never what it does; a corpus of shell commands is the
  product. The `danger` field is advisory metadata, not enforcement.

### Deliberately not doing

- **sha256 in the registry** — no independent guarantee; see §3.
- **Disabling HTTP proxies** — `ai.rs` already honors proxy env vars, and
  breaking corporate proxies to defend against an attacker who already controls
  the environment is a bad trade.
- **Symlink checks when reading pack files** — writing a symlink into
  `~/.collective/` requires home-directory write access, which is game over
  independently. Design B never creates one.
- **`pack inspect`** — `collective show <id>` already exists.
- **Per-entry provenance in search output** — see §6.

## 6. Loading and merge order

`corpus::load()` gains a third source:

```rust
merge(merge(embedded(), packs()), overlay())
```

Precedence: **embedded < packs < user overlay.** The existing two-argument
`merge(base, over)` already implements "over wins by id" and chains correctly;
no signature change.

`packs()` reads `~/.collective/packs/*.json` **in sorted filename order**, so
when two packs define the same id the alphabetically later pack wins —
deterministic and independent of filesystem ordering. Within a single pack a
duplicate id is a malformed pack: warn and keep the first.

Pack entries run through `Entry::validate()` at load time, and an invalid entry
warns and is skipped rather than aborting — identical to today's `overlay()`
behavior. A corrupt pack degrades; it never prevents the CLI from running.

**Grouped search is unchanged.** `search::is_bulk_import` keys off the
`tldr-import` domain, which tldr pack entries retain, so ranking behaves exactly
as in v4 once the pack is installed, and is inert before it is. The known
ceiling: a future pack that is not tldr will not be grouped as an import. Add
per-entry provenance when a second pack actually exists — not before.

## 7. Test regressions

No entry in `corpus/` or `corpus/gems/` carries the `tldr-import` domain, so
after the split the embedded corpus contains exactly zero bulk imports. Six
tests depend on that not being true: one fails outright, five keep passing while
testing nothing. All six are fragile for the same reason — they assert ranking
behavior against whatever happens to be in the embedded corpus.

| test | location | effect |
|---|---|---|
| `search_prints_separator_between_groups` | `tests/cli.rs:174` | **fails** — no imports, separator never prints |
| `search_curated_excludes_tldr_imports` | `tests/cli.rs:100` | passes vacuously — nothing to exclude |
| `search_curated_output_has_no_separator` | `tests/cli.rs:184` | passes vacuously |
| `curated_outranks_bulk_import` | `src/search.rs:93` | passes vacuously — no import to outrank |
| `curated_hits_all_precede_imports` | `src/search.rs:107` | passes vacuously — guarded block never runs |
| `both_groups_share_the_cap` | `src/search.rs:122` | vacuous; also the v4 carried debt |

Fix: `search()` takes `&[Entry]`, so the three `src/search.rs` unit tests
rebuild on synthetic fixtures — a handful of curated entries plus a handful
carrying `domains: [tldr-import]` — and assert ordering and the 6/4 cap
directly. This removes the dependency on corpus contents entirely and **pays off
the v4 carried debt** (`both_groups_share_the_cap` assuming ≥6 curated "git"
hits) rather than carrying it forward.

The three `tests/cli.rs` tests install a small fixture pack into a temp
`~/.collective/packs/` and assert separator and `--curated` behavior against it,
which doubles as the end-to-end test that pack loading works.

`caps_at_ten_results` asserts `<= 10` and is unaffected by corpus size.

New tests: pack name validation rejects `..`/`/`/empty/uppercase; `<owner>/<repo>`
parsing accepts valid pairs and rejects `..`, extra slashes, and empty segments;
response size cap trips; manifest name mismatch rejected; cross-origin overwrite
refused; invalid entry warns and is skipped without aborting; pack-vs-pack
precedence is sorted-filename deterministic; three-layer merge precedence holds;
`add`→`list`→`remove` roundtrip in a temp home.

## 8. Release pipeline

`.github/workflows/release.yml` gains a step that generates `tldr.json` from
`packs/tldr/*.yaml` and uploads it as a release asset. The registry's `url` for
tldr points at that asset. Bumping a pack means regenerating the JSON, attaching
it to a new release, and bumping the version in `registry.json`.

The generator is a small script — read the YAML directory, validate, emit
`{manifest, entries}`. It reuses the same `Entry` type, so a pack that would
fail `build.rs` cannot be published.

## Rollout order

1. `packs/tldr/` move + `build.rs` validates both trees (binary shrinks here).
2. Test rewrite onto synthetic fixtures (green before packs exist).
3. Pack format, `packs()` loader, three-layer merge.
4. `pack` subcommand: list/remove, then add (both resolution forms), update, search.
5. Pack generator script + release workflow step + registry repo + `registry.json`.
6. Version bump, README, tag.

Steps 1–4 stand alone: the `<owner>/<repo>` form needs no registry, so packs are
installable and testable before step 5 exists.

## Done criteria

- Fresh binary embeds ~152 entries; `corpus/imported/` no longer in the build tree.
- `collective pack add tldr` installs 1459 entries; search results and grouping
  match v4 behavior exactly with the pack installed.
- `pack add <owner>/<repo>` installs from a plain repo containing `pack.json`,
  with no registry entry involved.
- `pack list/update/remove` roundtrip cleanly; `pack add ../../evil` is rejected;
  a second pack claiming an installed name from a different origin is refused.
- Every test green with zero warnings, and no test depends on bulk imports being
  embedded.
- `brew upgrade` to the new version leaves an existing `~/.collective/` intact.

## Out of scope

Submission server, pack signing, arbitrary-URL install, inter-pack dependencies,
auto-update, themed gem packs, per-entry provenance display. Still refused as
bloat: in-app execution, plugins, config file, theming, telemetry.
