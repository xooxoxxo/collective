# Collective v4 (release engineering + debt) — Design Spec

Date: 2026-07-22
Status: Approved (brainstorming session)
Builds on: v1 (CLI), v2 (TUI + collect), v3 (frictionless loop)

## What

Three pieces:

1. **CI + release binaries + Homebrew tap** — GitHub Actions test/build gate,
   tagged releases with macOS + Linux binaries, `brew install
   xooxoxxo/tap/collective` via an auto-updated tap.
2. **TUI input model (fzf-style)** — every printable character types into the
   filter; actions move to Ctrl chords. Fixes the carried debt of `f/F/y/c`
   being untypeable in queries.
3. **Grouped search results** — curated matches deterministically precede tldr
   imports; CLI prints a separator between groups. Replaces the probabilistic
   ×2 score boost.

## Decided out of scope

**v5 = packs/registry** (agreed direction, own spec next): binary keeps a small
embedded starter pack (~150 curated gems); bulk corpora become fetchable packs
(`pack add/list/update/remove`) resolved via a JSON registry index repo
(`xooxoxxo/collective-registry`); existing corpus splits into publishable
sample packs. Consequence blessed: the bulk tldr import leaves the binary in
v5. Nothing in v4 blocks or prebuilds this.

Still refused as bloat: in-app execution, plugins, config file, theming,
telemetry.

## 1. CI + Release + Tap

### CI — `.github/workflows/ci.yml`

- Trigger: push + pull_request on `main`.
- Matrix: `macos-14`, `ubuntu-latest`.
- Steps: checkout, stable Rust toolchain, Swatinem/rust-cache, `cargo test`,
  `cargo build --release`.
- README gets a CI badge.

### Release — `.github/workflows/release.yml`

- Trigger: tag push `v*`.
- Build matrix (native builds, no cross-compilation):

| runner | target triple |
|---|---|
| macos-14 | aarch64-apple-darwin |
| macos-13 | x86_64-apple-darwin |
| ubuntu-latest | x86_64-unknown-linux-gnu |
| ubuntu-24.04-arm | aarch64-unknown-linux-gnu |

- Each job: `cargo build --release`, package
  `collective-<version>-<target>.tar.gz` (binary only), compute sha256, upload
  artifact.
- Final job: create the GitHub release with all tarballs + checksums, then
  regenerate `Formula/collective.rb` (version, per-target URL + sha256,
  `on_macos`/`on_linux` × `Hardware::CPU.arm?` blocks) and push it to
  `xooxoxxo/homebrew-tap`.
- Cross-repo push authenticates with a `TAP_GITHUB_TOKEN` repo secret (PAT,
  repo scope on the tap). **Manual step:** user mints the PAT; we set the
  secret with `gh secret set` and create the tap repo.
- Version: bump `Cargo.toml` to `0.2.0`; first tag `v0.2.0`.

## 2. TUI input model (fzf-style)

Every printable char appends to the filter. Actions are Ctrl chords, matched on
`(KeyCode, KeyModifiers)` — the current dispatch ignores modifiers entirely
(latent bug: Ctrl-C matches `Char('c')` and toggles curated today).

| key | action |
|---|---|
| any printable | types into filter (`f`, `c`, `y`, `q`, `j`, `k` included) |
| `↑` / `↓` | move selection (j/k navigation removed — they type) |
| `Enter` | prefill + exit |
| `Ctrl-Y` | copy selected cmd, stay open |
| `Ctrl-S` | toggle star (raw mode disables IXON, so XOFF is not a concern) |
| `Ctrl-O` | toggle favorites-only |
| `Ctrl-U` | toggle curated-only |
| `Esc` / `Ctrl-C` | quit |
| `Backspace` | delete last filter char |

Help bar: `↵ prefill  ^Y copy  ^S ★  ^O fav-only  ^U curated  Esc quit`.

Only key dispatch and help-bar text change; the pure `App` state and its tests
are untouched. Verified manually (rendering layer), consistent with v2/v3.

## 3. Grouped search results

- `search::search` keeps its signature. Internals: drop the ×2 curated
  multiplier; sort matches by `(is_bulk_import asc, score desc, id asc)`;
  truncate to 10 after grouping. All curated matches precede all import
  matches; a strong import match can no longer outrank a weak curated one.
- CLI `cmd_search`: print a `── tldr imports ──` separator line at the
  curated→import transition, only when both groups are present in the output.
- TUI: inherits the ordering via `recompute()`; no separator row (selection
  math stays simple; the `c` toggle covers isolation).

## Testing

- Existing `curated_outranks_bulk_import` must still pass (now guaranteed
  structurally). Existing ranking tests kept.
- New unit test: a query matching both groups returns every curated hit before
  any import hit.
- New integration tests: separator present when both groups match; absent when
  output is single-group.
- TUI key dispatch: manual verification (list filter typing incl. `f/c/y/q`,
  each Ctrl chord, Esc/Ctrl-C quit); pure App tests unchanged.
- CI proves the Linux build/test claim on every push.

## Rollout order

1. CI workflow lands first (gates the rest).
2. TUI keys + grouped search (TDD where pure).
3. Version 0.2.0, README (badge, brew install, new key table), tag `v0.2.0` →
   release workflow → tap formula.

## Done criteria

- CI green on macOS + Linux for push/PR.
- `v0.2.0` release exists with 4 binaries + checksums; `brew install
  xooxoxxo/tap/collective` installs a working binary on macOS.
- All printable letters typeable in the TUI filter; Ctrl chords act; Ctrl-C
  quits.
- `collective search port` shows curated results, separator, then imports.
- Full test suite green, zero warnings.
