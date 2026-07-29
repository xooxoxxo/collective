# pack.rs Module Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `src/pack.rs` into `src/pack/` along its five responsibilities, with no behaviour change whatsoever.

**Architecture:** A pure code move. Items relocate to the module matching their responsibility; shared `#[cfg(test)]` helpers move to a `testutil` module inside `pack/mod.rs`; visibility is widened only where a call now crosses a module boundary, and never beyond `pub(super)`. Each task moves one module's worth of code and ends with the full suite green.

**Tech Stack:** Rust 2021. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-07-29-pack-module-split-design.md`

## Global Constraints

- **No behaviour change of any kind.** Not a renamed function, not a reordered argument, not an altered error message. Function bodies move verbatim.
- **No API surface change.** `pack::parse`, `pack::installed`, `pack::remove`, `pack::add`, `pack::update`, `pack::search_registry`, `pack::validate_pack_name`, `pack::classify`, `pack::owner_repo_url`, `pack::Manifest`, `pack::Pack`, `pack::Arg`, `pack::Registry`, `pack::RegistryPack`, and `pack::REGISTRY_URL` keep exactly those paths.
- **`src/main.rs` and `src/corpus.rs` MUST NOT be edited.** They are the proof the surface is unchanged. If either fails to compile, the split is wrong — fix the split, not the caller.
- **No new tests, no changed assertions, no renamed tests.** Tests move verbatim. The suite is the instrument measuring this refactor; changing it destroys the measurement.
- **129 tests pass at the end of every task**, with the same names as before.
- `cargo clippy --all-targets -- -D warnings` exits 0; `cargo fmt` applied.
- No new dependencies.
- Nothing gains `pub` that was not already `pub`. Cross-module private calls become `pub(super)`.
- This package has NO lib target — run `cargo test`, never `cargo test --lib`.

## File Structure

| file | holds | approx lines |
|---|---|---|
| `src/pack/mod.rs` | `mod` declarations, `pub use` re-exports, `#[cfg(test)] mod testutil` | ~60 |
| `src/pack/types.rs` | `Manifest`, `Pack`, `Arg`, `validate_pack_name`, `segment_ok`, `classify`, `owner_repo_url` + 5 tests | ~150 |
| `src/pack/parse.rs` | `parse` + 1 test | ~60 |
| `src/pack/store.rs` | `installed`, `remove`, `install` + 8 tests | ~230 |
| `src/pack/fetch.rs` | `MAX_PACK_BYTES`, `fetch`, `add` + 2 tests | ~130 |
| `src/pack/registry.rs` | `REGISTRY_URL`, `Registry`, `RegistryPack`, `filter_registry`, `lookup_registry`, `registry`, `registry_url_for`, `search_registry`, `update` + 4 tests | ~180 |

### Shared test helpers

Four helpers currently live in `pack.rs`'s single test module and are used by
several groups: `temp_dir(tag)`, `seed(dir, name)`, `no_embedded()`, and
`pack_with(name, id)`. They move to `#[cfg(test)] mod testutil` in `pack/mod.rs`
and are imported where needed, rather than being duplicated per module.

---

### Task 1: Create the module and move types

**Files:**
- Create: `src/pack/mod.rs`, `src/pack/types.rs`
- Modify: `src/pack.rs` (remove what moved)

**Interfaces:**
- Produces: `pack::types::{Manifest, Pack, Arg, validate_pack_name, classify, owner_repo_url}`, all re-exported from `pack/mod.rs` so external paths stay `pack::Manifest` etc.

- [ ] **Step 1: Create `src/pack/types.rs`**

Move these items **verbatim** from `src/pack.rs`, keeping their doc comments and
bodies byte-identical: `Manifest` (line 7), `Pack` (27), `Arg` (34),
`validate_pack_name` (43), `segment_ok` (59), `classify` (67),
`owner_repo_url` (83).

Its imports are:

```rust
use crate::entry::Entry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
```

Then move these five tests verbatim into a `#[cfg(test)] mod tests` at the end
of `types.rs`, with `use super::*;`: `accepts_plain_pack_names`,
`rejects_names_that_could_escape_the_packs_dir`,
`classifies_the_three_argument_forms`, `rejects_hostile_source_addresses`,
`builds_a_raw_githubusercontent_url`.

`segment_ok` stays private — it is used only by `classify`, which now lives in
the same file.

- [ ] **Step 2: Create `src/pack/mod.rs`**

```rust
mod types;

pub use types::{classify, owner_repo_url, validate_pack_name, Arg, Manifest, Pack};
```

- [ ] **Step 3: Delete the moved items from `src/pack.rs`**

Remove lines 7–89 (the seven items) and the five moved tests from its test
module. `src/pack.rs` keeps everything else for now.

Rust will not allow both `src/pack.rs` and `src/pack/` to exist. Rename the
remainder to `src/pack/rest.rs` temporarily and add `mod rest;` plus
`pub use rest::*;` to `mod.rs`. Subsequent tasks empty `rest.rs` out; Task 5
deletes it.

Add to the top of `rest.rs`:

```rust
use super::types::{classify, owner_repo_url, validate_pack_name, Arg, Manifest, Pack};
```

- [ ] **Step 4: Run the suite**

Run: `cargo test`
Expected: PASS, 129 tests, same names.
Run: `cargo clippy --all-targets -- -D warnings`
Expected: exit 0.

If `src/main.rs` or `src/corpus.rs` fails to compile, the re-exports in
`mod.rs` are wrong — fix `mod.rs`, do not edit those files.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "refactor: extract pack types into their own module

Moves the data model and the validators that keep untrusted strings out of
paths and URLs into src/pack/types.rs. Pure code move, no behaviour change."
```

---

### Task 2: Move parse

**Files:**
- Create: `src/pack/parse.rs`
- Modify: `src/pack/mod.rs`, `src/pack/rest.rs`

**Interfaces:**
- Consumes: `types::{Manifest, Pack, validate_pack_name}` from Task 1
- Produces: `pack::parse(text: &str, expected_name: Option<&str>) -> Result<Pack, String>`, re-exported so `crate::pack::parse` still resolves — `src/corpus.rs` calls it by that path

- [ ] **Step 1: Create `src/pack/parse.rs`**

Move `parse` (line 91 of the original `pack.rs`) verbatim, with its doc comment.
Imports:

```rust
use super::types::{validate_pack_name, Pack};
```

Move the test `pack_json_roundtrips` verbatim into a `#[cfg(test)] mod tests`
with `use super::*;`. That test builds a `Pack` from JSON and calls
`entries[0].validate()`, so it also needs:

```rust
    use crate::pack::types::Pack;
```

only if `use super::*` does not already bring it — check by compiling, and keep
whichever single import makes it build.

- [ ] **Step 2: Wire it up**

In `mod.rs` add `mod parse;` and extend the re-export line to include it:

```rust
pub use parse::parse;
```

In `rest.rs`, delete `parse` and add `use super::parse::parse;` so the remaining
callers there still resolve.

- [ ] **Step 3: Run the suite**

Run: `cargo test`
Expected: PASS, 129 tests.
Run: `cargo clippy --all-targets -- -D warnings`
Expected: exit 0.

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "refactor: extract pack parsing into its own module

parse enforces policy the types do not - dropping schema-invalid entries,
rejecting a duplicate id within a pack, and optionally requiring the manifest
name to match. Pure code move, no behaviour change."
```

---

### Task 3: Move the local store

**Files:**
- Create: `src/pack/store.rs`
- Modify: `src/pack/mod.rs`, `src/pack/rest.rs`

**Interfaces:**
- Consumes: `types::{Manifest, Pack, validate_pack_name}`, `parse::parse`
- Produces: `pack::installed(dir: &Path) -> Vec<Manifest>` and `pack::remove(dir: &Path, name: &str) -> Result<(), String>` (both stay `pub`); `install(dir: &Path, pack: Pack, origin: &str, embedded: &HashSet<String>) -> Result<String, String>` becomes `pub(super)` — Tasks 4 and 5 call it

- [ ] **Step 1: Move the shared test helpers into `mod.rs` first**

Add to `src/pack/mod.rs`:

```rust
#[cfg(test)]
pub(crate) mod testutil {
    use super::parse::parse;
    use super::types::Pack;
    use std::collections::HashSet;
    use std::path::PathBuf;

    pub(crate) fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("col-pk-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    pub(crate) fn seed(dir: &std::path::Path, name: &str) {
        let json = format!(
            r#"{{"manifest":{{"name":"{name}","version":"1.0.0","count":0}},"entries":[]}}"#
        );
        std::fs::write(dir.join(format!("{name}.json")), json).unwrap();
    }

    pub(crate) fn no_embedded() -> HashSet<String> {
        HashSet::new()
    }

    pub(crate) fn pack_with(name: &str, id: &str) -> Pack {
        parse(
            &format!(
                r#"{{"manifest":{{"name":"{name}","count":1}},"entries":[
                   {{"id":"{id}","title":"T","cmd":"c","platform":["macos"],
                     "domains":["shell"],"danger":"low","explanation":"e","source":"s"}}]}}"#
            ),
            None,
        )
        .unwrap()
    }
}
```

These four bodies must match what is currently in `pack.rs`'s test module
byte-for-byte. Copy them from there rather than retyping; if any differs, the
tests that use them may change behaviour, which this refactor forbids.

- [ ] **Step 2: Create `src/pack/store.rs`**

Move `installed` (line 127 of the original), `remove` (152), and `install` (201)
verbatim. Change `install`'s signature line from `pub fn install(` to
`pub(super) fn install(` — nothing outside the `pack` module calls it. Leave
`installed` and `remove` as `pub`; `main.rs` calls both.

Imports:

```rust
use super::parse::parse;
use super::types::{validate_pack_name, Manifest, Pack};
use std::collections::HashSet;
use std::path::PathBuf;
```

Move these eight tests verbatim into `#[cfg(test)] mod tests`:
`installed_lists_packs_by_manifest`, `remove_deletes_only_the_named_pack`,
`remove_rejects_a_traversing_name_before_touching_disk`,
`remove_reports_a_missing_pack`, `install_writes_the_pack_and_records_origin`,
`install_overwrites_freely_from_the_same_origin`,
`install_refuses_to_overwrite_a_pack_from_a_different_origin`,
`install_reports_ids_that_shadow_embedded_entries`.

The test module needs:

```rust
    use super::*;
    use crate::pack::testutil::{no_embedded, pack_with, seed, temp_dir};
```

- [ ] **Step 3: Wire it up**

In `mod.rs` add `mod store;` and `pub use store::{installed, remove};`.
In `rest.rs`, delete the three moved functions and their eight tests, and add
`use super::store::install;`.

- [ ] **Step 4: Run the suite**

Run: `cargo test`
Expected: PASS, 129 tests, same names.
Run: `cargo clippy --all-targets -- -D warnings`
Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "refactor: extract the local pack store into its own module

Everything that touches ~/.collective/packs now lives together: listing,
removal, and the atomic install that validates the name, refuses a
cross-origin overwrite, and publishes by rename. install narrows to
pub(super) - nothing outside the module called it. Pure code move."
```

---

### Task 4: Move fetch

**Files:**
- Create: `src/pack/fetch.rs`
- Modify: `src/pack/mod.rs`, `src/pack/rest.rs`

**Interfaces:**
- Consumes: `types::{classify, owner_repo_url, Arg}`, `parse::parse`, `store::install`, and `registry_url_for` (still in `rest.rs` at this point)
- Produces: `pack::add(dir: &Path, source: &str, embedded: &HashSet<String>) -> Result<String, String>` (stays `pub`); `fetch(url: &str) -> Result<String, String>` becomes `pub(super)` — Task 5's `registry` module calls it

- [ ] **Step 1: Create `src/pack/fetch.rs`**

Move `MAX_PACK_BYTES` (line 163 of the original), `fetch` (171), and `add` (257)
verbatim. Change `fn fetch(` to `pub(super) fn fetch(`. `MAX_PACK_BYTES` stays
private. `add` stays `pub`.

Imports:

```rust
use super::parse::parse;
use super::store::install;
use super::types::{classify, owner_repo_url, Arg};
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
```

`add` calls `registry_url_for`, which is still in `rest.rs` until Task 5. Add
`use super::rest::registry_url_for;` and make it `pub(super)` in `rest.rs` for
now; Task 5 moves it to `registry.rs` and updates this import.

Move these two tests verbatim: `add_installs_from_a_local_path`,
`add_rejects_a_manifest_name_that_would_escape_the_packs_dir`. Test module
needs:

```rust
    use super::*;
    use crate::pack::testutil::{no_embedded, temp_dir};
```

- [ ] **Step 2: Wire it up**

In `mod.rs` add `mod fetch;` and `pub use fetch::add;`.
In `rest.rs`, delete the three moved items and their two tests, and add
`use super::fetch::fetch;`.

- [ ] **Step 3: Run the suite**

Run: `cargo test`
Expected: PASS, 129 tests.
Run: `cargo clippy --all-targets -- -D warnings`
Expected: exit 0.

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "refactor: extract the network boundary into its own module

The size cap, the configured agent, and add - which orchestrates resolve,
retrieve, parse, install - now live together. fetch narrows to pub(super).
Pure code move, no behaviour change."
```

---

### Task 5: Move the registry and delete the remainder

**Files:**
- Create: `src/pack/registry.rs`
- Modify: `src/pack/mod.rs`, `src/pack/fetch.rs`
- Delete: `src/pack/rest.rs`

**Interfaces:**
- Consumes: `types::validate_pack_name`, `parse::parse`, `store::{install, installed}`, `fetch::fetch`
- Produces: `pack::search_registry`, `pack::update`, `pack::REGISTRY_URL`, `pack::Registry`, `pack::RegistryPack`; `registry_url_for` becomes `pub(super)` for `fetch::add`

- [ ] **Step 1: Create `src/pack/registry.rs`**

Move verbatim from `rest.rs`: `REGISTRY_URL`, `RegistryPack`, `Registry`,
`filter_registry`, `lookup_registry`, `registry`, `registry_url_for`,
`search_registry`, `update`. `registry_url_for` is `pub(super)`;
`filter_registry`, `lookup_registry`, and `registry` stay private;
`search_registry`, `update`, `REGISTRY_URL`, `Registry`, `RegistryPack` stay
`pub`.

Imports:

```rust
use super::fetch::fetch;
use super::parse::parse;
use super::store::{install, installed};
use super::types::{validate_pack_name, Manifest};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
```

Move these four tests verbatim: `registry_filters_by_name_and_description`,
`registry_lookup_rejects_a_non_https_url`,
`update_reports_when_nothing_is_installed`,
`update_refuses_a_pack_installed_from_a_local_file`. Test module needs:

```rust
    use super::*;
    use crate::pack::testutil::{no_embedded, pack_with, temp_dir};
```

- [ ] **Step 2: Delete `rest.rs` and finish `mod.rs`**

`rest.rs` is now empty — delete the file and its `mod rest;` / `pub use rest::*;`
lines. In `fetch.rs`, change `use super::rest::registry_url_for;` to
`use super::registry::registry_url_for;`.

`src/pack/mod.rs` ends as:

```rust
mod fetch;
mod parse;
mod registry;
mod store;
mod types;

pub use fetch::add;
pub use parse::parse;
pub use registry::{search_registry, update, Registry, RegistryPack, REGISTRY_URL};
pub use store::{installed, remove};
pub use types::{classify, owner_repo_url, validate_pack_name, Arg, Manifest, Pack};

#[cfg(test)]
pub(crate) mod testutil { /* unchanged from Task 3 */ }
```

- [ ] **Step 3: Prove the API surface is unchanged**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
git diff 59945e3 --stat -- src/main.rs src/corpus.rs
```

Expected: 129 tests pass with the same names; clippy exits 0; **the `git diff`
prints nothing**, proving `main.rs` and `corpus.rs` were never edited. A
non-empty diff means the split changed the external surface and must be fixed
in `mod.rs`, not in the callers.

- [ ] **Step 4: Confirm no file is oversized and `pack.rs` is gone**

```bash
ls src/pack.rs 2>&1 | head -1     # expect: No such file or directory
wc -l src/pack/*.rs
```

Expected: `src/pack.rs` absent; no file in `src/pack/` much over 230 lines.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "refactor: extract the registry and retire pack.rs

The registry index and the operations that consult it - search and update -
move to src/pack/registry.rs, and the transitional rest.rs is deleted.

src/pack.rs held five responsibilities; each now has its own file, none over
about 230 lines. No behaviour change: 129 tests pass with the same names and
main.rs and corpus.rs were never edited, which is what proves the external
API surface is identical."
```

---

## Self-Review

**Spec coverage:**

| spec section | task |
|---|---|
| `types.rs` boundary and contents | Task 1 |
| `parse.rs` separate from types | Task 2 |
| `store.rs` owns the packs directory | Task 3 |
| `fetch.rs` is the network boundary | Task 4 |
| `registry.rs` holds the index and `update` | Task 5 |
| Visibility table (`install`, `fetch`, `registry_url_for` → `pub(super)`; nothing new `pub`) | Tasks 3, 4, 5 |
| Shared test helpers to a `testutil` module | Task 3 Step 1 |
| Safety property: 129 tests, `main.rs`/`corpus.rs` untouched | Task 5 Step 3 verifies explicitly; every task re-runs the suite |
| `src/pack.rs` no longer exists | Task 5 Step 4 |

No gaps.

**Placeholder scan:** No TBDs. The one instruction that defers a judgment —
Task 2's note to keep whichever import makes `parse.rs` compile — is a
compile-determined detail, not an unspecified requirement.

**Type consistency:** `install` is `pub(super)` from Task 3 and called by Task 4
(`add`) and Task 5 (`update`). `fetch` is `pub(super)` from Task 4 and called by
Task 5. `registry_url_for` is `pub(super)` from Task 5 and imported by Task 4's
`fetch.rs`, which Task 5 Step 2 repoints from `rest` to `registry`. The
`testutil` helpers introduced in Task 3 are used by Tasks 3, 4, and 5 with the
same four names throughout.

**Sequencing:** every task ends with a green suite. The transitional `rest.rs`
exists because Rust forbids `src/pack.rs` and `src/pack/` coexisting; it shrinks
each task and is deleted in Task 5.
