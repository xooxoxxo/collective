# Pack reorganization — Design Spec

Date: 2026-07-31
Status: Approved (brainstorming session)
Relates to: the `collective` CLI after the app-awareness merge (50742c6)
Depends on: `2026-07-30-app-awareness-design.md` (the `app:` field, `apps.yaml`
registry, and `apps::entry_binary` derivation)

## What

The tldr pack (1,459 entries, 342 distinct apps) becomes six packs: a lean
`tldr` holding only default-installed tools, and five purpose bundles. A
committed allowlist defines "default-installed" and `build.rs` enforces it.
The registry repo gains the five new packs; `apps.yaml` gains brew-verified
rows so the `^A` app pane works for bundle headliners.

## Why

The positioning decision (2026-07-29) keeps bulk content as opt-in packs. But
one 1,459-entry blob makes a poor opt-in: a user who wants git tooling should
not have to take transmission and traefik with it, and "tldr" implying
base-system reference should not surprise anyone with `cargo-yank`. The
2026-07-30 audit of the import also showed it is an **alphabetical slice**
(brew-*, cargo-*, clang*, git-*, ssh*, t-range), which purpose bundles carve
far more honestly than the single name does.

## 1. The carve

`packs/tldr/*.yaml` files MOVE (git mv; ids, content, and filenames
unchanged unless §4 adds an `app:` line) into:

| pack dir | contents | selection rule |
|---|---|---|
| `packs/tldr` | base-system commands + core git subcommands | derived binary ∈ allowlist, or entry has no app (shell builtins) |
| `packs/git-extras` | third-party git-* tools | id matches `tldr-git-*` and the subcommand is NOT in git's builtin list |
| `packs/rust-dev` | cargo, cargo-*, trunk-rs, try-rs | id prefix |
| `packs/homebrew` | brew, brew-* | id prefix |
| `packs/build-tools` | clang, clang-*, clangd, gcc, makensis | listed ids |
| `packs/toolbox` | everything left — the alphabetical tail (traefik, trivy, transmission*, trufflehog, tarsnap, sshfs, sshpass, sshuttle, bash-it, bashmarks, …), honestly named | remainder |

- **Ids never change.** Drill state (`~/.collective/drill.json`) is keyed by
  entry id and survives the move untouched. The `tldr-` id prefix stays even
  in bundle packs — it records provenance, not pack membership.
- **`domains: [tldr-import]` stays** on every moved entry, so `^U`
  curated-only keeps filtering all six packs.
- Core-vs-third-party git classification uses git's own builtin subcommand
  list (`git --list-cmds=builtins` output, committed as a fixture in the
  plan) — filename-based, deterministic.
- The migration is a script whose OUTPUT is committed and reviewed; the
  script itself is throwaway. A completeness check proves every original id
  lands in exactly one pack and the total stays 1,459.

## 2. The allowlist — `tldr-allowlist.yaml` at repo root

The reviewable definition of "default-installed": a single union list
(POSIX core ∪ macOS base ∪ common Linux base ∪ git). Root location for the
same reason as `apps.yaml` — the `packs/` walker parses every `.yaml` inside
as an Entry.

```yaml
allow:
  [awk, bash, cut, curl, defaults, find, git, grep, gzip, launchctl, make,
   rsync, scp, sed, sort, ssh, ssh-add, ssh-agent, ssh-copy-id, ssh-keygen,
   ssh-keyscan, sshd, tar, tr, true, truncate, xargs, zsh]
```

The list covers exactly the binaries that keep current entries in the lean
tldr; future additions are one-line PRs. Deliberate calls: `git` and `make`
in (CLT/base everywhere that matters); `clang`/`gcc` out (build-tools pack);
`tree`, `troff`, `sshfs`, `sshpass`, `sshuttle` out (not default → toolbox).

**Enforcement in `build.rs`:** for every entry under `packs/tldr/`, its
`apps::entry_binary` result must be `None` (builtin) or a member of the
allowlist — otherwise the build fails naming the file. Other pack dirs are
not gated by the allowlist.

## 3. App attribution in bundles

Derivation alone cannot see through git's subcommand dispatch: `git fame …`
derives to `git`, which is allowlisted and always available — so a missing
`git-fame` would never gray. The fix uses the app-awareness field:

- **git-extras pack:** every entry gets an explicit `app:` naming its
  PROVIDING binary. Members of the git-extras suite get `app: git-extras`:
  the suite installs all its `git-<name>` binaries together with a
  `git-extras` binary, so that one binary is a deliberate availability proxy
  for the whole suite — one registry row (`brew install git-extras`) covers
  ~60 entries. Standalone tools (git-lfs, git-flow, git-delta,
  git-filter-repo, git-cliff, git-secret, git-annex, git-fame, git-standup,
  git-imerge, git-bug, …) get `app: <their own binary>` and their own
  registry rows. The provider mapping is a committed fixture in the plan.
- **rust-dev, homebrew, build-tools, toolbox:** derivation is the default
  (`cargo x` → cargo, `brew x` → brew — correct providers already). Explicit
  `app:` only where an entry's real provider differs AND is registered.
- The existing build gate (`app:` must name a registered binary) stands
  unchanged — every explicit `app:` added here therefore has a registry row.

## 4. Registry rows — `apps.yaml`

Brew/apt-verified, incremental (~30–50 rows): `brew` itself, `cargo` (via
the rustup formula), `clang` (apt only; macOS has it via CLT), `git-extras`,
the standalone git tools above with real formulas, and tail apps that verify
(`traefik`, `trivy`, `trufflehog`, `transmission-cli`, `tarsnap`, `sshuttle`,
…). Anything unverifiable is skipped — `^A` shows "no app info", which is
true. Verification method and skip list recorded in the implementation
report, same as the app-awareness Task 3.

## 5. Release and registry wiring

- `release.yml`: the single build-pack step becomes a loop over the six pack
  dirs, producing `tldr.json`, `git-extras.json`, `rust-dev.json`,
  `homebrew.json`, `build-tools.json`, `toolbox.json`, all attached to the
  release.
- `xooxoxxo/collective-registry` `registry.json`: five new rows; the tldr
  row's description updated to say base-system-only.
- Migration for existing installs: the next `pack update` fetches the lean
  tldr. Release notes name the split and the five packs to `pack add` for
  the removed content. No in-binary migration code — YAGNI.

## 6. Testing

- Allowlist gate falsification: plant a non-allowlisted entry under
  `packs/tldr/`, build fails naming it, remove.
- Completeness: original 1,459 ids = union of the six packs' ids, disjoint.
- Per-pack build smoke: `build-pack` runs clean over each dir.
- git classification spot-checks: `git-stash` (builtin → tldr),
  `git-standup` (third-party → git-extras, `app:` set and registered).
- Registry validation and `app:` cross-check gates unchanged and green.
- Falsification pass for every new test, per repo process rule.

## Out of scope

- Re-importing tldr-pages to fix the alphabetical-slice bias (future work;
  this reorganizes what exists).
- Per-plugin cargo attribution (cargo-clippy vs cargo-binstall granularity)
  beyond registered standalone plugins.
- In-binary pack migration logic.
- Landing page / README copy updates beyond the pack list (fast follow after
  release).
