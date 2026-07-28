# Collective v5 (packs + registry) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Slim the binary to a ~152-entry embedded starter and add a `pack` subcommand that fetches additional corpora on demand.

**Architecture:** A pack is one self-describing JSON file (`{manifest, entries[]}`) fetched over HTTPS with `ureq` and stored in `~/.collective/packs/<name>.json`. No shell-out, no archive extraction. The corpus loader gains a third layer between the embedded starter and the user overlay: `embedded < packs < overlay`. Packs resolve either from a curated `registry.json` short name or from an `<owner>/<repo>` source address on raw.githubusercontent.

**Tech Stack:** Rust 2021, clap 4 (derive), serde/serde_json/serde_yaml, ureq 2.12 (already a dependency), include_dir, assert_cmd + predicates for integration tests.

**Spec:** `docs/superpowers/specs/2026-07-28-collective-v5-packs-design.md`

## Global Constraints

- **No new dependencies.** Everything needed is already in `Cargo.toml`. `ureq` is already used for HTTPS in `src/ai.rs:111`. Do not add `reqwest`, `tar`, `flate2`, `sha2`, `tempfile`, or `regex`.
- **Never shell out and never unpack an archive.** No `std::process::Command`, no `curl`, no `tar` anywhere in the pack path. These two rules are the entire security argument of the design; breaking either invalidates the spec.
- **On-disk pack name must match `^[a-z0-9-]+$`** — validated before the name is used to build any path, on `add`, `update`, and `remove` alike, including when the name comes from a publisher-authored `manifest.name`.
- **Remote fetch is HTTPS-only**, bounded to 32 MB, with explicit connect and read timeouts. Redirects stay at ureq's default cap of 5 — GitHub release assets 302 to `objects.githubusercontent.com`, so redirects must not be disabled.
- **No sha256, no signing, no proxy tampering, no symlink checks.** Each was considered and deliberately rejected in the spec; do not reintroduce them.
- **Invalid pack data warns and skips, never panics.** A corrupt pack degrades the corpus; it never stops the CLI from running.
- Follow existing repo idioms: `Result<(), String>` for validation, `eprintln!("warning: ...")` for skips, functions take an explicit path parameter for testability (see `src/favorites.rs:11`), and `#[path = "../entry.rs"] mod entry;` to share `Entry` with a non-lib target (see `build.rs:2`).
- Run `cargo clippy --all-targets -- -D warnings` and `cargo fmt` before every commit. Zero warnings is the standing bar for this repo.

## File Structure

| file | responsibility |
|---|---|
| `src/pack.rs` | **new** — `Manifest`/`Pack` types, name + source-address validation, URL resolution, fetch, install, list, remove. All pack logic lives here. |
| `src/corpus.rs` | **modify** — add `packs()` and `embedded_ids()`; `load()` becomes three-layer. |
| `src/main.rs` | **modify** — `Pack` subcommand with its own `PackCmd` enum, plus dispatch. |
| `src/search.rs` | **modify** — three ranking unit tests move onto synthetic fixtures. |
| `tests/cli.rs` | **modify** — three grouping tests move onto a temp-HOME fixture. |
| `build.rs` | **modify** — validate `packs/` as well as `corpus/`; embed only `corpus/`. |
| `packs/tldr/*.yaml` | **moved** from `corpus/imported/` — source of truth, no longer embedded. |
| `src/bin/build-pack.rs` | **new** — generator: a pack directory of YAML in, one `pack.json` out. |

Task order differs from the spec's rollout list in one place: the test rewrite (Task 1) comes *before* the corpus move (Task 2). The spec listed the move first, but moving first leaves the suite red between tasks. Synthetic fixtures pass in both states, so rewriting first keeps every task green end to end.

---

### Task 1: Make ranking tests corpus-independent

Six tests assert ranking behavior against whatever happens to be in the embedded corpus. One will fail and five will silently stop testing anything once bulk imports leave. Rewrite them onto fixtures they own, so they pass identically before and after Task 2.

The three `tests/cli.rs` tests use a temp `HOME` with an **overlay** entry carrying `domains: [tldr-import]`. The overlay loader already exists and needs no pack machinery, so these tests work today and keep working after packs land.

**Files:**
- Modify: `src/search.rs:93-142` (three unit tests)
- Modify: `tests/cli.rs:100-108`, `tests/cli.rs:174-192` (three integration tests)

**Interfaces:**
- Consumes: `search::search(&[Entry], &str) -> Vec<(&Entry, u32)>`, `search::is_bulk_import(&Entry) -> bool` (both already exist)
- Produces: a `fixture(id, title, domain)` helper in `src/search.rs` tests, reused by later tasks that need throwaway `Entry` values

- [ ] **Step 1: Replace the three `src/search.rs` unit tests**

Replace lines 93-142 (`curated_outranks_bulk_import`, `curated_hits_all_precede_imports`, `both_groups_share_the_cap`) with:

```rust
    /// Throwaway entry for ranking tests. `domain` decides curated vs import:
    /// "tldr-import" makes it a bulk import, anything else makes it curated.
    fn fixture(id: &str, title: &str, domain: &str) -> Entry {
        Entry {
            id: id.into(),
            title: title.into(),
            cmd: format!("run {id}"),
            undo: None,
            platform: vec!["macos".into()],
            domains: vec![domain.into()],
            danger: crate::entry::Danger::Low,
            explanation: "fixture".into(),
            source: "fixture".into(),
            tags: vec![],
        }
    }

    #[test]
    fn curated_outranks_bulk_import() {
        // The import is the better textual match; grouping must still win.
        let entries = vec![
            fixture("import-exact", "git log graph", "tldr-import"),
            fixture("curated-weak", "git things", "vcs"),
        ];
        let hits = search(&entries, "git log graph");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].0.id, "curated-weak", "import outranked a curated entry");
    }

    #[test]
    fn curated_hits_all_precede_imports() {
        let mut entries = vec![];
        for i in 0..3 {
            entries.push(fixture(&format!("import-{i}"), "git log", "tldr-import"));
            entries.push(fixture(&format!("curated-{i}"), "git log", "vcs"));
        }
        let hits = search(&entries, "git log");
        let first_import = hits
            .iter()
            .position(|(e, _)| is_bulk_import(e))
            .expect("fixture guarantees at least one import hit");
        assert_eq!(first_import, 3, "expected all 3 curated hits first");
        assert!(
            hits[first_import..].iter().all(|(e, _)| is_bulk_import(e)),
            "found a curated hit after an import hit"
        );
    }

    #[test]
    fn both_groups_share_the_cap() {
        // 8 of each: enough that the 6/4 split is forced rather than incidental.
        let mut entries = vec![];
        for i in 0..8 {
            entries.push(fixture(&format!("curated-{i}"), "git log", "vcs"));
            entries.push(fixture(&format!("import-{i}"), "git log", "tldr-import"));
        }
        let hits = search(&entries, "git log");
        assert_eq!(hits.len(), 10, "result must fill the 10 row cap");
        let curated = hits.iter().filter(|(e, _)| !is_bulk_import(e)).count();
        let imports = hits.iter().filter(|(e, _)| is_bulk_import(e)).count();
        assert_eq!(curated, 6, "curated group must cap at 6 when imports compete");
        assert_eq!(imports, 4, "imports must be guaranteed 4 slots");
    }

    #[test]
    fn one_group_backfills_the_whole_cap() {
        // No imports competing: curated takes all 10, no 6-slot cap applied.
        let entries: Vec<Entry> = (0..12)
            .map(|i| fixture(&format!("curated-{i}"), "git log", "vcs"))
            .collect();
        assert_eq!(search(&entries, "git log").len(), 10);
    }
```

Delete the `use crate::corpus;` import from the test module if nothing else in it still uses `corpus::load()`. `finds_pmset_for_sleep_query`, `title_outranks_cmd_only_match`, `caps_at_ten_results`, and `no_match_returns_empty` are unchanged — they either use fixtures already or assert corpus-size-independent properties.

- [ ] **Step 2: Run the unit tests**

Run: `cargo test search`
Expected: PASS, including the new `one_group_backfills_the_whole_cap`.

This also retires the v4 carried debt: `both_groups_share_the_cap` no longer assumes the corpus yields ≥6 curated "git" hits, so the in-code note about relaxing the assertions if the corpus shrinks can be deleted along with the old test body.

- [ ] **Step 3: Replace the three `tests/cli.rs` tests**

Replace `search_curated_excludes_tldr_imports` (line 100), `search_prints_separator_between_groups` (line 174), and `search_curated_output_has_no_separator` (line 184) with a temp-HOME trio. The helper follows the existing idiom at `tests/cli.rs:79`:

```rust
/// Temp HOME seeded with one curated and one bulk-import overlay entry, so
/// grouping behavior is asserted against fixtures rather than corpus contents.
fn grouping_home(tag: &str) -> std::path::PathBuf {
    let home = std::env::temp_dir().join(format!("col-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let dir = home.join(".collective/corpus");
    std::fs::create_dir_all(&dir).unwrap();
    let entry = |id: &str, domain: &str| {
        format!(
            "id: {id}\ntitle: zzmarker widget\ncmd: run {id}\nplatform: [macos]\n\
             domains: [{domain}]\ndanger: low\nexplanation: fixture entry.\nsource: fixture\n"
        )
    };
    std::fs::write(dir.join("zz-curated.yaml"), entry("zz-curated", "shell")).unwrap();
    std::fs::write(dir.join("zz-import.yaml"), entry("zz-import", "tldr-import")).unwrap();
    home
}

#[test]
fn search_prints_separator_between_groups() {
    let home = grouping_home("sep");
    Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "zzmarker"])
        .env("HOME", &home)
        .assert()
        .success()
        .stdout(str::contains("── tldr imports ──"))
        .stdout(str::contains("zz-curated"))
        .stdout(str::contains("zz-import"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn search_curated_excludes_tldr_imports() {
    let home = grouping_home("cur");
    let out = Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "zzmarker", "--curated"])
        .env("HOME", &home)
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("zz-curated"), "curated entry missing:\n{stdout}");
    assert!(!stdout.contains("zz-import"), "curated search leaked an import:\n{stdout}");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn search_curated_output_has_no_separator() {
    let home = grouping_home("nosep");
    let out = Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "zzmarker", "--curated"])
        .env("HOME", &home)
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("── tldr imports ──"));
    let _ = std::fs::remove_dir_all(&home);
}
```

The `zzmarker` token exists only in the fixtures, so these assert on entries the test owns rather than on whatever the corpus happens to contain.

- [ ] **Step 4: Run the whole suite**

Run: `cargo test`
Expected: PASS, all tests. These now assert real behavior with bulk imports still embedded, and will keep asserting it after Task 2.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/search.rs tests/cli.rs
git commit -m "test: assert ranking on fixtures instead of corpus contents

Six tests asserted grouping behavior against whatever happened to be in the
embedded corpus. Rebuilt on synthetic entries they own, so they keep testing
the property once bulk imports move out of the binary rather than passing
vacuously. Retires the v4 both_groups_share_the_cap corpus-size assumption."
```

---

### Task 2: Move the bulk corpus out of the embedded tree

**Files:**
- Move: `corpus/imported/*.yaml` → `packs/tldr/*.yaml` (1459 files)
- Modify: `build.rs:11-13`
- Modify: `src/corpus.rs:67-71` (embedded seed test)

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: `packs/tldr/` as the tldr pack's source of truth, read by Task 8's generator

- [ ] **Step 1: Move the files with git**

```bash
mkdir -p packs
git mv corpus/imported packs/tldr
```

Use `git mv` so history follows the files.

- [ ] **Step 2: Teach build.rs to validate both trees**

`build.rs` currently seeds its walk with `corpus/` only. Both trees get identical schema validation and share one duplicate-id set; only `corpus/` is embedded. Change lines 11-13 from:

```rust
    println!("cargo:rerun-if-changed=corpus");
    let mut ids = HashSet::new();
    let mut stack = vec![Path::new("corpus").to_path_buf()];
```

to:

```rust
    // corpus/ is embedded in the binary; packs/ is published as fetchable
    // packs. Both are schema-validated here and share one id namespace, so a
    // pack entry that would break the corpus cannot be published either.
    println!("cargo:rerun-if-changed=corpus");
    println!("cargo:rerun-if-changed=packs");
    let mut ids = HashSet::new();
    let mut stack = vec![
        Path::new("corpus").to_path_buf(),
        Path::new("packs").to_path_buf(),
    ];
```

`include_dir!("$CARGO_MANIFEST_DIR/corpus")` in `src/corpus.rs:5` is unchanged — that is what makes the binary shrink.

- [ ] **Step 3: Tighten the embedded-corpus test**

`src/corpus.rs:67-71` asserts `entries.len() >= 10`, which no longer proves the split happened. Replace the test body with:

```rust
    #[test]
    fn embedded_corpus_is_the_starter_only() {
        let entries = embedded();
        assert!(entries.len() >= 100, "starter shrank unexpectedly: {}", entries.len());
        assert!(entries.len() < 500, "bulk imports leaked back into the binary");
        assert!(entries.iter().any(|e| e.id == "pmset-disable-sleep"));
        assert!(
            !entries.iter().any(|e| e.domains.iter().any(|d| d == "tldr-import")),
            "embedded corpus must contain no bulk imports"
        );
    }
```

- [ ] **Step 4: Verify the build gate still bites**

```bash
printf 'id: bad_ID\ntitle: x\ncmd: x\nplatform: [macos]\ndomains: [x]\ndanger: low\nexplanation: x\nsource: x\n' > packs/tldr/_probe.yaml
cargo build 2>&1 | tail -3
rm packs/tldr/_probe.yaml
```

Expected: build **fails** naming `packs/tldr/_probe.yaml` and the bad id. This proves `packs/` is genuinely validated, not just walked. Confirm the file is deleted afterward.

- [ ] **Step 5: Run the suite and confirm the binary shrank**

```bash
cargo test
cargo build --release && ls -lh target/release/collective
```

Expected: all tests PASS (Task 1 made them corpus-independent), and the release binary is materially smaller than before — the 1459 entries are no longer compiled in.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add -A
git commit -m "refactor: move bulk tldr corpus out of the embedded tree

corpus/imported moves to packs/tldr: still under version control, no longer
compiled into the binary. build.rs validates both trees against one id
namespace, so a pack entry that would break the corpus cannot be published.

The binary now ships the ~152-entry curated starter. Restoring the full
corpus becomes an opt-in pack install."
```

---

### Task 3: Pack types and source-address validation

Pure logic, no IO, no network. Everything here is unit-testable in isolation.

**Files:**
- Create: `src/pack.rs`
- Modify: `src/main.rs:1-11` (add `mod pack;`)

**Interfaces:**
- Consumes: `crate::entry::Entry`
- Produces:
  - `pub struct Manifest { name, version, description, source, license, count, origin }` (all `String` except `count: usize`)
  - `pub struct Pack { manifest: Manifest, entries: Vec<Entry> }`
  - `pub fn validate_pack_name(&str) -> Result<(), String>`
  - `pub enum Arg { Local(PathBuf), OwnerRepo(String, String), Name(String) }`
  - `pub fn classify(&str) -> Result<Arg, String>`
  - `pub fn owner_repo_url(&str, &str) -> String`

- [ ] **Step 1: Write the failing tests**

Create `src/pack.rs` containing only the test module plus stub signatures:

```rust
use crate::entry::Entry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub count: usize,
    /// Resolved fetch URL, written by us at install time. Never trusted from a
    /// publisher: `manifest.source` is their advisory claim, this is what we
    /// actually fetched, and it is overwritten on every install.
    #[serde(default)]
    pub origin: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pack {
    pub manifest: Manifest,
    pub entries: Vec<Entry>,
}

/// How a `pack add` argument was understood.
#[derive(Debug, PartialEq, Eq)]
pub enum Arg {
    Local(PathBuf),
    OwnerRepo(String, String),
    Name(String),
}

pub fn validate_pack_name(_name: &str) -> Result<(), String> {
    todo!()
}

pub fn classify(_arg: &str) -> Result<Arg, String> {
    todo!()
}

pub fn owner_repo_url(_owner: &str, _repo: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_pack_names() {
        assert!(validate_pack_name("tldr").is_ok());
        assert!(validate_pack_name("my-pack-2").is_ok());
    }

    #[test]
    fn rejects_names_that_could_escape_the_packs_dir() {
        for bad in ["..", ".", "../../.zshrc", "a/b", "", "Tldr", "a b", "a.json"] {
            assert!(validate_pack_name(bad).is_err(), "accepted bad name {bad:?}");
        }
    }

    #[test]
    fn classifies_the_three_argument_forms() {
        assert_eq!(classify("tldr").unwrap(), Arg::Name("tldr".into()));
        assert_eq!(
            classify("xooxoxxo/collective-tldr").unwrap(),
            Arg::OwnerRepo("xooxoxxo".into(), "collective-tldr".into())
        );
        assert_eq!(
            classify("./local.json").unwrap(),
            Arg::Local(PathBuf::from("./local.json"))
        );
    }

    #[test]
    fn rejects_hostile_source_addresses() {
        for bad in [
            "../../etc",        // traversal via the owner segment
            "owner/..",         // traversal via the repo segment
            "../repo",
            "owner/re/po",      // extra segment
            "/repo",            // empty owner
            "owner/",           // empty repo
            "own er/repo",      // space
            "owner/re?po",      // query injection into the URL
            "owner/re#po",      // fragment injection
        ] {
            assert!(classify(bad).is_err(), "accepted hostile source {bad:?}");
        }
    }

    #[test]
    fn builds_a_raw_githubusercontent_url() {
        assert_eq!(
            owner_repo_url("xooxoxxo", "collective-tldr"),
            "https://raw.githubusercontent.com/xooxoxxo/collective-tldr/HEAD/pack.json"
        );
    }

    #[test]
    fn pack_json_roundtrips() {
        let json = r#"{
            "manifest": {"name": "tldr", "version": "1.0.0", "count": 1},
            "entries": [{
                "id": "a-b", "title": "T", "cmd": "c",
                "platform": ["macos"], "domains": ["shell"],
                "danger": "low", "explanation": "e", "source": "s"
            }]
        }"#;
        let pack: Pack = serde_json::from_str(json).unwrap();
        assert_eq!(pack.manifest.name, "tldr");
        assert_eq!(pack.manifest.origin, "", "origin defaults empty when absent");
        assert_eq!(pack.entries.len(), 1);
        assert!(pack.entries[0].validate().is_ok());
    }
}
```

Add `mod pack;` to the module list at the top of `src/main.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test pack`
Expected: FAIL — the `todo!()` bodies panic.

- [ ] **Step 3: Implement the three functions**

Replace the three `todo!()` bodies:

```rust
/// On-disk pack names become path components, so they get the same charset as
/// entry ids. `Path::join` does not neutralize `..`, so an unchecked name
/// escapes the packs directory on both write and remove.
pub fn validate_pack_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(format!("bad pack name {name:?}: use lowercase/digits/hyphens"))
    }
}

/// A GitHub owner or repo segment. Excludes `/`, so a segment cannot introduce
/// a path component, and rejects bare dot segments so neither can be `..`.
fn segment_ok(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

pub fn classify(arg: &str) -> Result<Arg, String> {
    if arg.ends_with(".json") {
        return Ok(Arg::Local(PathBuf::from(arg)));
    }
    if let Some((owner, repo)) = arg.split_once('/') {
        if !segment_ok(owner) || !segment_ok(repo) {
            return Err(format!("bad source address {arg:?}: expected <owner>/<repo>"));
        }
        return Ok(Arg::OwnerRepo(owner.into(), repo.into()));
    }
    validate_pack_name(arg)?;
    Ok(Arg::Name(arg.into()))
}

pub fn owner_repo_url(owner: &str, repo: &str) -> String {
    format!("https://raw.githubusercontent.com/{owner}/{repo}/HEAD/pack.json")
}
```

`split_once` splits on the *first* `/`, so `owner/re/po` leaves `re/po` as the repo segment, which `segment_ok` rejects for containing `/`. That is what makes the extra-segment case fail.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test pack`
Expected: PASS, all six.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/pack.rs src/main.rs
git commit -m "feat: pack types and source-address validation

Manifest/Pack types plus the two validators that keep publisher- and
user-supplied strings out of path construction: on-disk names take the entry
id charset, and owner/repo segments exclude slashes and bare dot segments so
neither can walk out of the intended URL path."
```

---

### Task 4: Load installed packs into the corpus

**Files:**
- Modify: `src/corpus.rs` (add `packs_dir`, `read_packs`, `packs`, `embedded_ids`; change `load`)
- Modify: `src/pack.rs` (add `parse` helper)

**Interfaces:**
- Consumes: `pack::Pack`, `pack::validate_pack_name`
- Produces:
  - `corpus::load() -> Vec<Entry>` — now three-layer
  - `pub fn corpus::embedded_ids() -> HashSet<String>` — used by Task 6's shadow warning
  - `pub fn pack::parse(text: &str, expected_name: Option<&str>) -> Result<Pack, String>`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/corpus.rs`:

```rust
    fn write_pack(dir: &std::path::Path, file: &str, name: &str, id: &str, title: &str) {
        let json = format!(
            r#"{{"manifest":{{"name":"{name}","count":1}},"entries":[
                {{"id":"{id}","title":"{title}","cmd":"c","platform":["macos"],
                  "domains":["shell"],"danger":"low","explanation":"e","source":"s"}}]}}"#
        );
        std::fs::write(dir.join(file), json).unwrap();
    }

    fn temp_packs_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("col-packs-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn later_pack_filename_wins_on_duplicate_id() {
        let dir = temp_packs_dir("dup");
        write_pack(&dir, "a-pack.json", "a-pack", "shared-id", "FROM A");
        write_pack(&dir, "b-pack.json", "b-pack", "shared-id", "FROM B");
        let entries = read_packs(&dir);
        let hit: Vec<_> = entries.iter().filter(|e| e.id == "shared-id").collect();
        assert_eq!(hit.len(), 1, "duplicate id survived across packs");
        assert_eq!(hit[0].title, "FROM B", "sorted-filename precedence not applied");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_pack_warns_and_skips_without_aborting() {
        let dir = temp_packs_dir("bad");
        std::fs::write(dir.join("broken.json"), "{ not json").unwrap();
        write_pack(&dir, "good.json", "good", "good-id", "GOOD");
        let entries = read_packs(&dir);
        assert_eq!(entries.len(), 1, "a corrupt pack must not take the good one down");
        assert_eq!(entries[0].id, "good-id");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn entry_failing_validation_is_skipped_not_fatal() {
        let dir = temp_packs_dir("badentry");
        // First entry has an empty cmd, which Entry::validate rejects.
        let json = r#"{"manifest":{"name":"p","count":2},"entries":[
            {"id":"bad-one","title":"T","cmd":"","platform":["macos"],
             "domains":["shell"],"danger":"low","explanation":"e","source":"s"},
            {"id":"good-one","title":"T","cmd":"c","platform":["macos"],
             "domains":["shell"],"danger":"low","explanation":"e","source":"s"}]}"#;
        std::fs::write(dir.join("p.json"), json).unwrap();
        let entries = read_packs(&dir);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "good-one");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_id_within_one_pack_keeps_the_first() {
        let dir = temp_packs_dir("intra");
        let json = r#"{"manifest":{"name":"p","count":2},"entries":[
            {"id":"same","title":"FIRST","cmd":"c","platform":["macos"],
             "domains":["shell"],"danger":"low","explanation":"e","source":"s"},
            {"id":"same","title":"SECOND","cmd":"c","platform":["macos"],
             "domains":["shell"],"danger":"low","explanation":"e","source":"s"}]}"#;
        std::fs::write(dir.join("p.json"), json).unwrap();
        let entries = read_packs(&dir);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "FIRST");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn overlay_beats_packs_and_packs_beat_embedded() {
        let base = vec![fixture_entry("shared", "FROM EMBEDDED")];
        let packs = vec![fixture_entry("shared", "FROM PACK")];
        let over = vec![fixture_entry("shared", "FROM OVERLAY")];
        let merged = merge(merge(base.clone(), packs.clone()), over);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].title, "FROM OVERLAY");
        let merged_no_overlay = merge(base, packs);
        assert_eq!(merged_no_overlay[0].title, "FROM PACK");
    }

    fn fixture_entry(id: &str, title: &str) -> Entry {
        Entry {
            id: id.into(),
            title: title.into(),
            cmd: "c".into(),
            undo: None,
            platform: vec!["macos".into()],
            domains: vec!["shell".into()],
            danger: crate::entry::Danger::Low,
            explanation: "e".into(),
            source: "s".into(),
            tags: vec![],
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test corpus`
Expected: FAIL — `read_packs` is not defined.

- [ ] **Step 3: Add `pack::parse`**

In `src/pack.rs`:

```rust
/// Parse pack JSON and drop entries that fail schema validation. A malformed
/// entry degrades the pack; it never aborts the load. When `expected_name` is
/// given, a manifest claiming a different name is rejected outright — that
/// mismatch means the fetched file is not the pack that was asked for.
pub fn parse(text: &str, expected_name: Option<&str>) -> Result<Pack, String> {
    let mut pack: Pack = serde_json::from_str(text).map_err(|e| e.to_string())?;
    validate_pack_name(&pack.manifest.name)?;
    if let Some(want) = expected_name {
        if pack.manifest.name != want {
            return Err(format!(
                "manifest name {:?} does not match requested pack {want:?}",
                pack.manifest.name
            ));
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut kept = Vec::with_capacity(pack.entries.len());
    for e in pack.entries {
        if let Err(err) = e.validate() {
            eprintln!("warning: skipping entry in pack {}: {err}", pack.manifest.name);
            continue;
        }
        if !seen.insert(e.id.clone()) {
            eprintln!(
                "warning: duplicate id {} within pack {}, keeping the first",
                e.id, pack.manifest.name
            );
            continue;
        }
        kept.push(e);
    }
    pack.entries = kept;
    Ok(pack)
}
```

- [ ] **Step 4: Add the loader to `src/corpus.rs`**

Add after `overlay()`, and change `load()`:

```rust
pub fn packs_dir() -> Option<std::path::PathBuf> {
    Some(
        directories::BaseDirs::new()?
            .home_dir()
            .join(".collective/packs"),
    )
}

/// Entries from every installed pack, read in sorted filename order so that a
/// duplicate id resolves to the alphabetically later pack deterministically,
/// independent of filesystem ordering. A pack that fails to parse warns and is
/// skipped whole.
fn read_packs(dir: &std::path::Path) -> Vec<Entry> {
    let Ok(read) = fs::read_dir(dir) else { return vec![] };
    let mut files: Vec<std::path::PathBuf> = read
        .filter_map(|f| f.ok())
        .map(|f| f.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    let mut out: Vec<Entry> = Vec::new();
    for p in files {
        let parsed = fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|text| crate::pack::parse(&text, None));
        match parsed {
            Ok(pack) => {
                for e in pack.entries {
                    out.retain(|x| x.id != e.id); // later pack wins
                    out.push(e);
                }
            }
            Err(err) => eprintln!("warning: skipping pack {}: {err}", p.display()),
        }
    }
    out
}

fn packs() -> Vec<Entry> {
    match packs_dir() {
        Some(dir) => read_packs(&dir),
        None => vec![],
    }
}

/// Ids compiled into the binary. Used to warn when an incoming pack would
/// shadow a starter entry.
pub fn embedded_ids() -> std::collections::HashSet<String> {
    embedded().into_iter().map(|e| e.id).collect()
}

pub fn load() -> Vec<Entry> {
    merge(merge(embedded(), packs()), overlay())
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test corpus`
Expected: PASS, all five new tests plus the three existing ones.

- [ ] **Step 6: Run the whole suite**

Run: `cargo test`
Expected: PASS. No installed packs exist yet, so `packs()` returns empty and behavior is unchanged.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/corpus.rs src/pack.rs
git commit -m "feat: load installed packs as a third corpus layer

load() becomes embedded < packs < overlay. Packs are read in sorted filename
order so a cross-pack duplicate id resolves deterministically; within a pack
the first of a duplicate wins. A corrupt pack or a single invalid entry warns
and is skipped rather than taking the corpus down."
```

---

### Task 5: `pack list` and `pack remove`

The read-only and destructive halves of the subcommand, both offline. Building these before `pack add` means the CLI surface and its path guards are proven before any network code exists.

**Files:**
- Modify: `src/pack.rs` (add `installed`, `remove`)
- Modify: `src/main.rs` (add `Pack` subcommand, `PackCmd` enum, dispatch, `cmd_pack`)
- Modify: `tests/cli.rs` (add CLI tests)

**Interfaces:**
- Consumes: `corpus::packs_dir`, `pack::parse`, `pack::validate_pack_name`
- Produces:
  - `pub fn pack::installed(dir: &Path) -> Vec<Manifest>`
  - `pub fn pack::remove(dir: &Path, name: &str) -> Result<(), String>`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/pack.rs`:

```rust
    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("col-pk-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn seed(dir: &std::path::Path, name: &str) {
        let json = format!(
            r#"{{"manifest":{{"name":"{name}","version":"1.0.0","count":0}},"entries":[]}}"#
        );
        std::fs::write(dir.join(format!("{name}.json")), json).unwrap();
    }

    #[test]
    fn installed_lists_packs_by_manifest() {
        let dir = temp_dir("list");
        seed(&dir, "alpha");
        seed(&dir, "beta");
        let found = installed(&dir);
        let names: Vec<&str> = found.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "beta"], "must list sorted by filename");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_deletes_only_the_named_pack() {
        let dir = temp_dir("rm");
        seed(&dir, "alpha");
        seed(&dir, "beta");
        remove(&dir, "alpha").unwrap();
        assert!(!dir.join("alpha.json").exists());
        assert!(dir.join("beta.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_rejects_a_traversing_name_before_touching_disk() {
        let dir = temp_dir("rmbad");
        let victim = dir.join("victim.json");
        std::fs::write(&victim, "{}").unwrap();
        assert!(remove(&dir, "../victim").is_err());
        assert!(remove(&dir, "..").is_err());
        assert!(victim.exists(), "traversing name reached the filesystem");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_reports_a_missing_pack() {
        let dir = temp_dir("rmmissing");
        assert!(remove(&dir, "nope").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test pack`
Expected: FAIL — `installed` and `remove` are not defined.

- [ ] **Step 3: Implement `installed` and `remove`**

In `src/pack.rs`:

```rust
/// Manifests of every installed pack, sorted by filename. A pack that fails to
/// parse is skipped so one bad file cannot break `pack list`.
pub fn installed(dir: &std::path::Path) -> Vec<Manifest> {
    let Ok(read) = std::fs::read_dir(dir) else { return vec![] };
    let mut files: Vec<PathBuf> = read
        .filter_map(|f| f.ok())
        .map(|f| f.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(p).ok()?;
            match parse(&text, None) {
                Ok(pack) => Some(pack.manifest),
                Err(err) => {
                    eprintln!("warning: skipping pack {}: {err}", p.display());
                    None
                }
            }
        })
        .collect()
}

pub fn remove(dir: &std::path::Path, name: &str) -> Result<(), String> {
    validate_pack_name(name)?;
    let path = dir.join(format!("{name}.json"));
    if !path.exists() {
        return Err(format!("pack {name:?} is not installed"));
    }
    std::fs::remove_file(&path).map_err(|e| format!("could not remove {name}: {e}"))
}
```

`validate_pack_name` runs before `dir.join`, which is what stops `../victim` from resolving.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test pack`
Expected: PASS.

- [ ] **Step 5: Wire the subcommand into `src/main.rs`**

Add to the `Cmd` enum, after `Collect`:

```rust
    /// Manage fetchable corpus packs
    Pack {
        #[command(subcommand)]
        action: PackCmd,
    },
```

Add a new enum after `Cmd`:

```rust
#[derive(Subcommand)]
enum PackCmd {
    /// List installed packs
    List,
    /// Search the registry for packs
    Search { query: Vec<String> },
    /// Install a pack by registry name, <owner>/<repo>, or local .json path
    Add { source: String },
    /// Refetch installed packs from their recorded origin
    Update { name: Option<String> },
    /// Uninstall a pack
    Remove { name: String },
}
```

Add to the dispatch `match cli.cmd`:

```rust
        Some(Cmd::Pack { action }) => cmd_pack(action),
```

Add the handler. `List` and `Remove` land now; the rest arrive in Tasks 6 and 7:

```rust
fn cmd_pack(action: PackCmd) {
    let Some(dir) = corpus::packs_dir() else {
        eprintln!("cannot locate home directory");
        std::process::exit(1);
    };
    let result = match action {
        PackCmd::List => {
            let packs = pack::installed(&dir);
            if packs.is_empty() {
                println!("no packs installed");
            }
            for m in packs {
                let version = if m.version.is_empty() { "-" } else { &m.version };
                println!("{:<20} {:<10} {:>5} entries  {}", m.name, version, m.count, m.origin);
            }
            Ok(())
        }
        PackCmd::Remove { name } => pack::remove(&dir, &name).map(|_| println!("removed {name}")),
        PackCmd::Add { .. } | PackCmd::Update { .. } | PackCmd::Search { .. } => {
            Err("not implemented yet".to_string())
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
```

- [ ] **Step 6: Add the CLI tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn pack_list_reports_empty_and_then_the_installed_pack() {
    let home = std::env::temp_dir().join(format!("col-packlist-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let dir = home.join(".collective/packs");
    std::fs::create_dir_all(&dir).unwrap();

    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "list"])
        .env("HOME", &home)
        .assert()
        .success()
        .stdout(str::contains("no packs installed"));

    std::fs::write(
        dir.join("demo.json"),
        r#"{"manifest":{"name":"demo","version":"2.1.0","count":1},"entries":[
            {"id":"demo-entry","title":"Demo","cmd":"echo demo","platform":["macos"],
             "domains":["shell"],"danger":"low","explanation":"e","source":"s"}]}"#,
    )
    .unwrap();

    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "list"])
        .env("HOME", &home)
        .assert()
        .success()
        .stdout(str::contains("demo"))
        .stdout(str::contains("2.1.0"));

    // The installed pack's entries must be searchable.
    Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "demo"])
        .env("HOME", &home)
        .assert()
        .success()
        .stdout(str::contains("demo-entry"));

    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "remove", "demo"])
        .env("HOME", &home)
        .assert()
        .success();
    assert!(!dir.join("demo.json").exists());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn pack_remove_rejects_a_traversing_name() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "remove", "../../evil"])
        .assert()
        .failure()
        .stderr(str::contains("bad pack name"));
}
```

The search assertion in the middle is the end-to-end proof that Task 4's loader is wired into the real binary.

- [ ] **Step 7: Run the suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/pack.rs src/main.rs tests/cli.rs
git commit -m "feat: pack list and pack remove

Adds the pack subcommand with its offline halves. Name validation runs before
any path is built, so a traversing name is rejected before it reaches the
filesystem. Covered end to end: an installed pack's entries turn up in search."
```

---

### Task 6: `pack add`

**Files:**
- Modify: `src/pack.rs` (add `fetch`, `install`, `add`)
- Modify: `src/main.rs` (`cmd_pack` handles `Add`)
- Modify: `tests/cli.rs` (local-path install tests)

**Interfaces:**
- Consumes: `pack::classify`, `pack::parse`, `pack::owner_repo_url`, `corpus::embedded_ids`
- Produces:
  - `pub fn pack::install(dir: &Path, pack: Pack, origin: &str, embedded: &HashSet<String>) -> Result<String, String>`
  - `pub fn pack::add(dir: &Path, source: &str, embedded: &HashSet<String>) -> Result<String, String>`

Every remote path is exercised through `install` with a local file, so the test suite never touches the network.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/pack.rs`:

```rust
    fn no_embedded() -> std::collections::HashSet<String> {
        std::collections::HashSet::new()
    }

    fn pack_with(name: &str, id: &str) -> Pack {
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

    #[test]
    fn install_writes_the_pack_and_records_origin() {
        let dir = temp_dir("install");
        install(&dir, pack_with("demo", "demo-id"), "https://example.test/p.json", &no_embedded())
            .unwrap();
        let text = std::fs::read_to_string(dir.join("demo.json")).unwrap();
        let back: Pack = serde_json::from_str(&text).unwrap();
        assert_eq!(back.manifest.origin, "https://example.test/p.json");
        assert_eq!(back.entries.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_overwrites_freely_from_the_same_origin() {
        let dir = temp_dir("sameorigin");
        let url = "https://example.test/p.json";
        install(&dir, pack_with("demo", "one"), url, &no_embedded()).unwrap();
        install(&dir, pack_with("demo", "two"), url, &no_embedded()).unwrap();
        let text = std::fs::read_to_string(dir.join("demo.json")).unwrap();
        assert!(text.contains("two"), "same-origin reinstall must overwrite");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_refuses_to_overwrite_a_pack_from_a_different_origin() {
        let dir = temp_dir("crossorigin");
        install(&dir, pack_with("tldr", "official"), "https://official.test/p.json", &no_embedded())
            .unwrap();
        let err = install(
            &dir,
            pack_with("tldr", "hostile"),
            "https://raw.githubusercontent.com/someone/tldr/HEAD/pack.json",
            &no_embedded(),
        )
        .unwrap_err();
        assert!(err.contains("already installed"), "unexpected error: {err}");
        let text = std::fs::read_to_string(dir.join("tldr.json")).unwrap();
        assert!(text.contains("official"), "hostile pack clobbered the official one");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_reports_ids_that_shadow_embedded_entries() {
        let dir = temp_dir("shadow");
        let embedded: std::collections::HashSet<String> =
            ["flush-dns-cache".to_string()].into_iter().collect();
        let report = install(
            &dir,
            pack_with("demo", "flush-dns-cache"),
            "https://example.test/p.json",
            &embedded,
        )
        .unwrap();
        assert!(report.contains("flush-dns-cache"), "shadowing not reported: {report}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn add_installs_from_a_local_path() {
        let dir = temp_dir("addlocal");
        let src = dir.join("source-pack.json");
        std::fs::write(
            &src,
            r#"{"manifest":{"name":"local","count":1},"entries":[
                {"id":"local-id","title":"T","cmd":"c","platform":["macos"],
                 "domains":["shell"],"danger":"low","explanation":"e","source":"s"}]}"#,
        )
        .unwrap();
        add(&dir, src.to_str().unwrap(), &no_embedded()).unwrap();
        assert!(dir.join("local.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn add_rejects_a_manifest_name_that_would_escape_the_packs_dir() {
        let dir = temp_dir("addescape");
        let src = dir.join("evil.json");
        std::fs::write(
            &src,
            r#"{"manifest":{"name":"../../pwned","count":0},"entries":[]}"#,
        )
        .unwrap();
        assert!(add(&dir, src.to_str().unwrap(), &no_embedded()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test pack`
Expected: FAIL — `install` and `add` are not defined.

- [ ] **Step 3: Implement fetch, install, and add**

In `src/pack.rs`, add the imports `use std::io::Read;` and:

```rust
/// Packs are data, not archives: 32 MB is far above any plausible corpus and
/// far below anything that would exhaust memory.
const MAX_PACK_BYTES: u64 = 32 * 1024 * 1024;

/// One HTTPS GET of one JSON document. Redirects stay at ureq's default cap of
/// 5 because GitHub release assets 302 to objects.githubusercontent.com. The
/// body is bounded by `take` rather than by trusting `content-length`, which a
/// hostile server can understate.
fn fetch(url: &str) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err(format!("refusing non-https url: {url}"));
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(60))
        .build();
    let resp = agent.get(url).call().map_err(|e| format!("fetch failed: {e}"))?;
    let mut buf = String::new();
    resp.into_reader()
        .take(MAX_PACK_BYTES)
        .read_to_string(&mut buf)
        .map_err(|e| format!("read failed: {e}"))?;
    if buf.len() as u64 >= MAX_PACK_BYTES {
        return Err(format!("pack exceeds the {MAX_PACK_BYTES} byte limit"));
    }
    Ok(buf)
}

/// Write a validated pack to `<dir>/<name>.json`, refusing to land on a pack
/// installed from a different origin. Returns the human-facing report.
pub fn install(
    dir: &std::path::Path,
    mut pack: Pack,
    origin: &str,
    embedded: &std::collections::HashSet<String>,
) -> Result<String, String> {
    validate_pack_name(&pack.manifest.name)?;
    let name = pack.manifest.name.clone();
    let path = dir.join(format!("{name}.json"));

    // A pack name is claimable by any publisher, so an incoming pack must not
    // land on one installed from somewhere else just by reusing its name.
    if let Ok(existing) = std::fs::read_to_string(&path) {
        if let Ok(old) = parse(&existing, None) {
            if old.manifest.origin != origin {
                return Err(format!(
                    "pack {name:?} is already installed from {}; \
                     run `collective pack remove {name}` first",
                    old.manifest.origin
                ));
            }
        }
    }

    pack.manifest.origin = origin.to_string();
    pack.manifest.count = pack.entries.len();
    let shadowed: Vec<&str> = pack
        .entries
        .iter()
        .filter(|e| embedded.contains(&e.id))
        .map(|e| e.id.as_str())
        .collect();

    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create packs dir: {e}"))?;
    let json = serde_json::to_string(&pack).map_err(|e| e.to_string())?;
    // Same-directory temp plus rename: one syscall to publish, so an interrupt
    // or a concurrent add can never leave a half-written pack in place.
    let tmp = dir.join(format!(".{name}.json.tmp"));
    std::fs::write(&tmp, &json).map_err(|e| format!("cannot write pack: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("cannot install pack: {e}"))?;

    let mut report = format!("installed {name} ({} entries)", pack.entries.len());
    if !shadowed.is_empty() {
        report.push_str(&format!(
            "\nwarning: {} entries override starter entries: {}",
            shadowed.len(),
            shadowed.join(", ")
        ));
    }
    Ok(report)
}

/// Resolve a `pack add` argument, retrieve the pack, and install it.
pub fn add(
    dir: &std::path::Path,
    source: &str,
    embedded: &std::collections::HashSet<String>,
) -> Result<String, String> {
    let (text, origin) = match classify(source)? {
        Arg::Local(path) => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            (text, format!("file://{}", path.display()))
        }
        Arg::OwnerRepo(owner, repo) => {
            let url = owner_repo_url(&owner, &repo);
            (fetch(&url)?, url)
        }
        Arg::Name(name) => {
            let url = registry_url_for(&name)?;
            (fetch(&url)?, url)
        }
    };
    let pack = parse(&text, None)?;
    install(dir, pack, &origin, embedded)
}
```

`registry_url_for` arrives in Task 7. To keep this task independently green, add the stub now:

```rust
fn registry_url_for(_name: &str) -> Result<String, String> {
    Err("registry lookup lands in the next task".into())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test pack`
Expected: PASS, all six new tests.

- [ ] **Step 5: Wire `Add` into `cmd_pack`**

In `src/main.rs`, replace the `PackCmd::Add { .. }` arm of the combined not-implemented match:

```rust
        PackCmd::Add { source } => {
            pack::add(&dir, &source, &corpus::embedded_ids()).map(|report| println!("{report}"))
        }
        PackCmd::Update { .. } | PackCmd::Search { .. } => Err("not implemented yet".to_string()),
```

- [ ] **Step 6: Add the CLI test**

Append to `tests/cli.rs`:

```rust
#[test]
fn pack_add_installs_from_a_local_file_and_warns_on_shadowing() {
    let home = std::env::temp_dir().join(format!("col-packadd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let src = home.join("src-pack.json");
    // pmset-disable-sleep is an embedded starter id, so this must warn.
    std::fs::write(
        &src,
        r#"{"manifest":{"name":"shadowy","count":1},"entries":[
            {"id":"pmset-disable-sleep","title":"Hijacked","cmd":"echo nope",
             "platform":["macos"],"domains":["shell"],"danger":"low",
             "explanation":"e","source":"s"}]}"#,
    )
    .unwrap();

    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "add", src.to_str().unwrap()])
        .env("HOME", &home)
        .assert()
        .success()
        .stdout(str::contains("installed shadowy"))
        .stdout(str::contains("override starter entries"))
        .stdout(str::contains("pmset-disable-sleep"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn pack_add_rejects_a_non_https_source() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "add", "owner/re?po"])
        .assert()
        .failure()
        .stderr(str::contains("bad source address"));
}
```

- [ ] **Step 7: Run the suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 8: Manual smoke test against a real repo**

Publish a `pack.json` to any GitHub repo you control, then:

```bash
cargo run -- pack add <owner>/<repo>
cargo run -- pack list
```

Expected: the pack installs, `pack list` shows it with the raw.githubusercontent origin, and its entries appear in `cargo run -- search <term>`. This is the only step that touches the network; everything above is offline.

- [ ] **Step 9: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/pack.rs src/main.rs tests/cli.rs
git commit -m "feat: pack add from local path or owner/repo

One HTTPS GET of one JSON document: no shell-out, no archive, so neither
argument injection nor archive path traversal has anywhere to occur. The body
is bounded by take() rather than a spoofable content-length, install publishes
via same-directory rename so an interrupt cannot leave a half-written pack,
and a pack claiming an installed name from a different origin is refused
instead of clobbering it. Ids shadowing starter entries are reported."
```

---

### Task 7: `pack update` and `pack search`

**Files:**
- Modify: `src/pack.rs` (add `Registry`, `registry_url_for`, `search_registry`, `update`)
- Modify: `src/main.rs` (`cmd_pack` handles `Update` and `Search`)

**Interfaces:**
- Consumes: `pack::fetch`, `pack::installed`, `pack::add`
- Produces:
  - `pub const REGISTRY_URL: &str`
  - `pub fn pack::search_registry(query: &str) -> Result<Vec<RegistryPack>, String>`
  - `pub fn pack::update(dir: &Path, name: Option<&str>, embedded: &HashSet<String>) -> Result<String, String>`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/pack.rs`:

```rust
    #[test]
    fn registry_filters_by_name_and_description() {
        let json = r#"{"packs":[
            {"name":"tldr","description":"tldr-pages bulk import","url":"https://x.test/t.json"},
            {"name":"kube","description":"kubernetes recipes","url":"https://x.test/k.json"}]}"#;
        let reg: Registry = serde_json::from_str(json).unwrap();
        assert_eq!(filter_registry(&reg, "tldr").len(), 1);
        assert_eq!(filter_registry(&reg, "kubernetes").len(), 1, "must match description");
        assert_eq!(filter_registry(&reg, "").len(), 2, "empty query lists everything");
        assert_eq!(filter_registry(&reg, "nothing").len(), 0);
    }

    #[test]
    fn registry_lookup_rejects_a_non_https_url() {
        let reg: Registry = serde_json::from_str(
            r#"{"packs":[{"name":"evil","description":"","url":"http://x.test/e.json"}]}"#,
        )
        .unwrap();
        assert!(lookup_registry(&reg, "evil").is_err(), "non-https registry url accepted");
    }

    #[test]
    fn update_reports_when_nothing_is_installed() {
        let dir = temp_dir("update-empty");
        let report = update(&dir, None, &no_embedded()).unwrap();
        assert!(report.contains("no packs"), "unexpected report: {report}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn update_refuses_a_pack_installed_from_a_local_file() {
        let dir = temp_dir("update-local");
        install(&dir, pack_with("demo", "d"), "file:///tmp/x.json", &no_embedded()).unwrap();
        let report = update(&dir, Some("demo"), &no_embedded()).unwrap();
        assert!(report.contains("skipped"), "local-origin pack must be skipped: {report}");
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test pack`
Expected: FAIL — `Registry`, `filter_registry`, `lookup_registry`, and `update` are not defined.

- [ ] **Step 3: Implement the registry and update**

In `src/pack.rs`, replace the `registry_url_for` stub with:

```rust
pub const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/xooxoxxo/collective-registry/HEAD/registry.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryPack {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub count: usize,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Registry {
    pub packs: Vec<RegistryPack>,
}

fn filter_registry<'a>(reg: &'a Registry, query: &str) -> Vec<&'a RegistryPack> {
    let q = query.trim().to_lowercase();
    reg.packs
        .iter()
        .filter(|p| {
            q.is_empty()
                || p.name.to_lowercase().contains(&q)
                || p.description.to_lowercase().contains(&q)
        })
        .collect()
}

fn lookup_registry(reg: &Registry, name: &str) -> Result<String, String> {
    let hit = reg
        .packs
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("no pack named {name:?} in the registry"))?;
    if !hit.url.starts_with("https://") {
        return Err(format!("registry url for {name:?} is not https"));
    }
    Ok(hit.url.clone())
}

fn registry() -> Result<Registry, String> {
    serde_json::from_str(&fetch(REGISTRY_URL)?).map_err(|e| format!("bad registry: {e}"))
}

fn registry_url_for(name: &str) -> Result<String, String> {
    lookup_registry(&registry()?, name)
}

pub fn search_registry(query: &str) -> Result<Vec<RegistryPack>, String> {
    Ok(filter_registry(&registry()?, query)
        .into_iter()
        .cloned()
        .collect())
}

/// Refetch installed packs from their recorded origin. There is no version
/// comparison: the <owner>/<repo> form has no registry entry to compare
/// against, so refetching is both simpler and the only rule that works for
/// every source type.
pub fn update(
    dir: &std::path::Path,
    name: Option<&str>,
    embedded: &std::collections::HashSet<String>,
) -> Result<String, String> {
    let targets: Vec<Manifest> = match name {
        Some(n) => {
            validate_pack_name(n)?;
            installed(dir)
                .into_iter()
                .filter(|m| m.name == n)
                .collect()
        }
        None => installed(dir),
    };
    if targets.is_empty() {
        return Ok("no packs to update".to_string());
    }
    let mut lines = Vec::new();
    for m in targets {
        if !m.origin.starts_with("https://") {
            lines.push(format!("skipped {}: installed from {}", m.name, m.origin));
            continue;
        }
        match fetch(&m.origin).and_then(|text| parse(&text, Some(&m.name))) {
            Ok(pack) => match install(dir, pack, &m.origin, embedded) {
                Ok(report) => lines.push(report),
                Err(e) => lines.push(format!("failed {}: {e}", m.name)),
            },
            Err(e) => lines.push(format!("failed {}: {e}", m.name)),
        }
    }
    Ok(lines.join("\n"))
}
```

`parse(&text, Some(&m.name))` is why `expected_name` exists: on update, a refetched document whose manifest now claims a different name is rejected rather than silently installed under the old one.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test pack`
Expected: PASS.

- [ ] **Step 5: Wire `Update` and `Search` into `cmd_pack`**

In `src/main.rs`, replace the remaining not-implemented arm:

```rust
        PackCmd::Update { name } => {
            pack::update(&dir, name.as_deref(), &corpus::embedded_ids())
                .map(|report| println!("{report}"))
        }
        PackCmd::Search { query } => pack::search_registry(&query.join(" ")).map(|hits| {
            if hits.is_empty() {
                println!("no matching packs");
            }
            for p in hits {
                println!("{:<20} {:>5} entries  {}", p.name, p.count, p.description);
            }
        }),
```

- [ ] **Step 6: Run the suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/pack.rs src/main.rs
git commit -m "feat: pack update and pack search

update always refetches from the recorded origin rather than comparing
versions: the owner/repo form has no registry entry to compare against, so
refetching is the only rule that works for both source types. A refetched
document whose manifest changed name is rejected instead of being installed
under the old one. Packs installed from a local file are skipped, not failed."
```

---

### Task 8: Pack generator, registry, and release wiring

**Files:**
- Create: `src/bin/build-pack.rs`
- Modify: `.github/workflows/release.yml`
- Modify: `README.md`
- Modify: `Cargo.toml` (version bump)

**Interfaces:**
- Consumes: `packs/tldr/*.yaml`, `src/entry.rs`
- Produces: `tldr.json` release asset; `registry.json` in `xooxoxxo/collective-registry`

- [ ] **Step 1: Write the generator**

Create `src/bin/build-pack.rs`. It shares `Entry` via the `#[path]` trick `build.rs:2` already uses, so no lib target is needed:

```rust
//! Build a distributable pack from a directory of corpus YAML.
//!
//! Usage: build-pack <dir> <name> <version> <license> <description> > pack.json
//!
//! Reuses the same Entry type the build gate uses, so a pack that would fail
//! `cargo build` cannot be published either.

#[path = "../entry.rs"]
mod entry;

use std::{collections::HashSet, fs, path::Path};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, dir, name, version, license, description] = args.as_slice() else {
        eprintln!("usage: build-pack <dir> <name> <version> <license> <description>");
        std::process::exit(1);
    };

    let mut entries = Vec::new();
    let mut ids = HashSet::new();
    let mut stack = vec![Path::new(dir).to_path_buf()];
    while let Some(d) = stack.pop() {
        for f in fs::read_dir(&d).unwrap_or_else(|e| panic!("{dir}: {e}")) {
            let p = f.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "yaml") {
                let text = fs::read_to_string(&p).unwrap();
                let e: entry::Entry = serde_yaml::from_str(&text)
                    .unwrap_or_else(|err| panic!("{}: {err}", p.display()));
                e.validate()
                    .unwrap_or_else(|err| panic!("{}: {err}", p.display()));
                assert!(ids.insert(e.id.clone()), "duplicate id: {}", e.id);
                entries.push(e);
            }
        }
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));

    let pack = serde_json::json!({
        "manifest": {
            "name": name,
            "version": version,
            "description": description,
            "source": "https://github.com/xooxoxxo/collective",
            "license": license,
            "count": entries.len(),
            "origin": ""
        },
        "entries": entries
    });
    println!("{}", serde_json::to_string(&pack).unwrap());
    eprintln!("built pack {name} with {} entries", entries.len());
}
```

- [ ] **Step 2: Generate the tldr pack and verify it round-trips**

```bash
cargo run --bin build-pack -- packs/tldr tldr 1.0.0 CC-BY-4.0 "tldr-pages bulk import" > /tmp/tldr.json
ls -lh /tmp/tldr.json
cargo run -- pack add /tmp/tldr.json
cargo run -- pack list
cargo run -- search "port"
```

Expected: ~1459 entries reported on stderr; `pack list` shows tldr; `search port` shows curated results, the `── tldr imports ──` separator, then imports — matching v4 behavior with the pack installed. Then confirm removal is clean:

```bash
cargo run -- pack remove tldr
cargo run -- search "port"
```

Expected: separator gone, curated results only.

- [ ] **Step 3: Stop the build matrix from cross-compiling the generator**

Packaging is already safe: `.github/workflows/release.yml:35` copies `target/${{ matrix.target }}/release/collective` into `dist/` by name, so the new second executable cannot leak into a release tarball. No packaging change is needed.

The build line does need one: `.github/workflows/release.yml:30` is

```yaml
      - run: cargo build --release --target ${{ matrix.target }}
```

which would now build `build-pack` on all four targets, including cross-compiling a host-only tool to aarch64 for nothing. Scope it to the shipped binary:

```yaml
      - run: cargo build --release --target ${{ matrix.target }} --bin collective
```

`ci.yml` is left alone — building everything there is what catches a broken generator.

- [ ] **Step 4: Add the pack artifact to the release job**

Add a step to the release job that builds and attaches the pack, so every release publishes a matching `tldr.json`:

```yaml
      - name: Build tldr pack
        run: |
          cargo run --release --bin build-pack -- \
            packs/tldr tldr "${GITHUB_REF_NAME#v}" CC-BY-4.0 "tldr-pages bulk import" \
            > tldr.json
```

Attach `tldr.json` alongside the binary tarballs in the release-creation step. Build it once, in the job that creates the release, not per target — it is platform-independent.

- [ ] **Step 5: Create the registry repo**

Per the repo's standing gh-account note, switch accounts first — `gh` drifts back to the work account:

```bash
gh auth switch --user oytuneyucel
gh auth status
gh repo create xooxoxxo/collective-registry --public \
  --description "Pack registry for the collective CLI"
```

Seed `registry.json` at the repo root. The `url` points at the release asset from Step 4:

```json
{
  "packs": [
    {
      "name": "tldr",
      "description": "tldr-pages bulk import",
      "license": "CC-BY-4.0",
      "count": 1459,
      "url": "https://github.com/xooxoxxo/collective/releases/latest/download/tldr.json"
    }
  ]
}
```

Then verify both resolution paths end to end:

```bash
cargo run -- pack search tldr
cargo run -- pack add tldr
cargo run -- pack list
```

Expected: `pack search` lists tldr from the registry; `pack add tldr` installs via the registry URL and `pack list` shows that URL as the origin.

- [ ] **Step 6: Update the README**

Document the split and the new subcommand. Cover: the binary now ships a ~152-entry curated starter; `collective pack add tldr` restores the full corpus; the `pack` command table from spec §4; and that a pack is publishable by pushing a `pack.json` to any GitHub repo, installable with `collective pack add <owner>/<repo>`.

- [ ] **Step 7: Bump the version and run the full suite**

Set `version = "0.3.0"` in `Cargo.toml` — packs are a feature addition and the corpus split changes what a fresh install contains.

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS, zero warnings.

- [ ] **Step 8: Commit**

```bash
cargo fmt
git add -A
git commit -m "feat: pack generator, registry, and release wiring

build-pack turns a corpus YAML directory into a distributable pack.json,
reusing the same Entry type as the build gate so a pack that would fail the
build cannot be published. The release job builds and attaches tldr.json, and
registry.json resolves the tldr short name to that asset.

Version 0.3.0: a fresh install now ships the curated starter, and the full
corpus becomes an opt-in pack install."
```

- [ ] **Step 9: Tag the release**

```bash
git tag v0.3.0 && git push origin main && git push origin v0.3.0
```

Then confirm the release published four binaries plus `tldr.json`, and that a `brew upgrade` leaves an existing `~/.collective/` intact — installed packs, overlay, and favorites all survive because nothing in this plan writes outside `~/.collective/packs/`.

---

## Self-Review

**Spec coverage:**

| spec section | task |
|---|---|
| §1 corpus split, build.rs both trees | Task 2 |
| §2 pack JSON format | Task 3 |
| §3 registry + owner/repo resolution, no sha256 | Tasks 3, 7 |
| §4 command table, update always refetches | Tasks 5, 6, 7 |
| §5.1 pack name validation | Tasks 3, 5, 6 |
| §5.2 URL resolution, https-only | Tasks 3, 6, 7 |
| §5.3 configured agent, redirects kept | Task 6 |
| §5.4 32 MB bound via `take` | Task 6 |
| §5.5 deserialize + validate every entry | Task 4 |
| §5.6 shadowing warning | Task 6 |
| §5.7 cross-origin overwrite refused | Task 6 |
| §5.8 atomic temp + rename | Task 6 |
| §6 three-layer merge, sorted pack order, `is_bulk_import` unchanged | Task 4 |
| §7 six broken tests + new tests | Tasks 1, 3–7 |
| §8 generator + release pipeline | Task 8 |

No gaps. Every "deliberately not doing" item stays absent: no sha256, no proxy tampering, no symlink checks, no `pack inspect`, no provenance display.

**Type consistency:** `Manifest`/`Pack` defined in Task 3 and used unchanged in 4–7. `parse(&str, Option<&str>)` defined in Task 4, called with `None` in 4–6 and `Some(&name)` in 7. `install(dir, pack, origin, embedded)` and `add(dir, source, embedded)` are consistent between Task 6's definition and Task 7's `update`. `corpus::packs_dir` and `corpus::embedded_ids` are defined in Task 4 and consumed in 5–7. `registry_url_for` is stubbed in Task 6 and replaced in Task 7 with the same signature.

**Sequencing:** every task ends with a green suite. Task 1 precedes the corpus move so the suite never goes red. Task 6's `registry_url_for` stub keeps Task 6 green before Task 7 lands. The only network-dependent steps are Task 6 Step 8 and Task 8 Steps 2–5, all manual.
