# pack.rs module split — Design Spec

Date: 2026-07-29
Status: Approved (brainstorming session)
Relates to: `src/pack.rs`, introduced in the v5 packs work

## What

Split `src/pack.rs` into a `src/pack/` module directory along the five
responsibilities it currently holds. **No behaviour changes.** Every `pack::X`
path used elsewhere stays valid, every test passes untouched.

## Why

`src/pack.rs` is the largest file in the project at 711 lines. That headline
number overstates the case — 335 of those lines are tests, so production code is
about 375, comparable to navi's largest source file. Size is not the argument.

The argument is that one file holds five distinct responsibilities:

1. Data types and the validators that keep untrusted strings out of paths
2. Parsing a pack document
3. Reading and writing the local pack directory
4. Fetching over the network
5. The registry index and the operations that consult it

They change for different reasons and are tested in different ways — the
validators are pure and table-tested, the store functions need a temp directory,
the fetch path is deliberately exercised only through local files. Keeping them
together means every change to any one of them requires reading past the other
four.

## Non-goals

- **No behaviour change of any kind.** Not a renamed function, not a reordered
  argument, not an adjusted error message. If a caller's output would differ,
  the change is wrong.
- **No API surface change.** `pack::parse`, `pack::installed`, `pack::remove`,
  `pack::add`, `pack::update`, `pack::search_registry` keep exactly those paths.
- **No new tests and no changed assertions.** Tests move verbatim with the code
  they cover. Adding coverage is a separate piece of work; mixing it in would
  make it impossible to tell a refactoring mistake from a new test's failure.
- **No new dependencies.**

## Target structure

```
src/pack/
  mod.rs        module declarations and re-exports (thin)
  types.rs      Manifest, Pack, Arg, validate_pack_name, segment_ok,
                classify, owner_repo_url
  parse.rs      parse
  store.rs      installed, remove, install
  fetch.rs      MAX_PACK_BYTES, fetch, add
  registry.rs   REGISTRY_URL, Registry, RegistryPack, filter_registry,
                lookup_registry, registry, registry_url_for,
                search_registry, update
```

### Why these boundaries

**`types.rs`** holds everything pure and IO-free: the data model plus the two
validators that decide whether a user- or publisher-supplied string may become
a filesystem path or a URL. Those validators are the security control of the
whole feature, and they belong somewhere a reader can find and audit without
wading through HTTP code.

**`parse.rs`** is separate from `types.rs` because parsing enforces policy the
types do not — it drops schema-invalid entries with a warning, rejects a
duplicate id within a pack, and can require the manifest name to match what was
asked for. That is behaviour, not structure.

**`store.rs`** is every function that touches `~/.collective/packs`. `install`
lives here rather than with `fetch` because installing is a filesystem
operation — validate the name, refuse a cross-origin overwrite, write to a
per-process temp file, rename into place. It never fetches anything.

**`fetch.rs`** is the network boundary: the size cap, the configured agent, and
`add`, which orchestrates resolve → retrieve → parse → install. `add` sits with
`fetch` because retrieving is the part that distinguishes it from a plain
install.

**`registry.rs`** is the index and everything that consults it. `update` lives
here because refetching from a recorded origin is a registry-shaped operation,
and because it is the only caller that passes `Some(name)` to `parse`.

### Visibility

`segment_ok` is currently private to `pack.rs` and used only by `classify`;
both move to `types.rs`, so it stays private. `token`-style helpers that cross a
new module boundary become `pub(crate)` or `pub(super)` — never `pub` — so the
external surface does not grow. `MAX_PACK_BYTES` stays private to `fetch.rs`.

The crossings that the split creates, and the visibility each needs:

| item | moves to | called from | visibility |
|---|---|---|---|
| `parse` | `parse.rs` | `store`, `fetch`, `registry`, and `corpus.rs` | stays `pub` — it is external API |
| `install` | `store.rs` | `fetch` (`add`), `registry` (`update`) | `pub(super)` |
| `fetch` | `fetch.rs` | `registry` (`update`, `registry`) | `pub(super)` |
| `registry_url_for` | `registry.rs` | `fetch` (`add`) | `pub(super)` |
| `installed` | `store.rs` | `registry` (`update`), and `main.rs` | stays `pub` — it is external API |
| `validate_pack_name`, `classify`, `owner_repo_url` | `types.rs` | everywhere | stay `pub` — already are |

Nothing gains `pub` that was not already `pub`. `segment_ok` is used only by
`classify` and stays private within `types.rs`; `MAX_PACK_BYTES` stays private
within `fetch.rs`; `filter_registry` and `lookup_registry` stay private within
`registry.rs`.

## The safety property

This refactor is correct if and only if the shipped behaviour is unchanged. Two
checks establish that, and both must be performed rather than assumed:

1. **All 129 tests pass with no test file edited except by relocation.** A diff
   of the test bodies before and after must show only moves, never changes.
2. **The public API surface is unchanged**, verified by comparing the set of
   `pack::` paths reachable from outside the module before and after. `main.rs`
   and `corpus.rs` must compile without edits — if either needs a change, the
   split has altered the surface and is wrong.

`cargo clippy --all-targets -- -D warnings` must exit 0, as it does today.

## Testing

No new tests. The existing suite is the instrument: 129 passing before, 129
passing after, with the same names.

Test relocation follows the code — `types.rs` takes the validator and classifier
tests, `parse.rs` the parse tests, `store.rs` the installed/remove/install
tests, `fetch.rs` the add tests, `registry.rs` the registry and update tests.
Shared test helpers (`temp_dir`, `seed`, `pack_with`, `no_embedded`) are
currently defined once in `pack.rs`'s test module and used by several groups.
They move to a `#[cfg(test)]` helper module inside `pack/mod.rs` and are
imported by the modules that need them, rather than being duplicated.

## Rollout order

1. Create `src/pack/mod.rs` re-exporting from the existing file's contents,
   moving `types.rs` out first — the leaf with no dependencies on the others.
2. `parse.rs`, then `store.rs` — each depends only on `types`.
3. `fetch.rs`, then `registry.rs` — the two that depend on the rest.
4. Delete the now-empty `src/pack.rs`.

Each step ends with the full suite green, so a mistake is localised to the step
that introduced it.

## Done criteria

- `src/pack.rs` no longer exists; `src/pack/` holds six files, none over about
  200 lines.
- 129 tests pass, with the same test names as before.
- `main.rs` and `corpus.rs` are untouched.
- `cargo clippy --all-targets -- -D warnings` exits 0.
- No new dependency, no new `pub` item on the module's external surface.
