use crate::entry::Entry;
use include_dir::{include_dir, Dir};
use std::fs;

static EMBEDDED: Dir = include_dir!("$CARGO_MANIFEST_DIR/corpus");

/// All embedded entries. Panics never: build.rs already validated them.
fn embedded() -> Vec<Entry> {
    fn walk(dir: &Dir, out: &mut Vec<Entry>) {
        for f in dir.files() {
            if f.path().extension().is_some_and(|e| e == "yaml") {
                let text = f.contents_utf8().expect("corpus yaml is utf8");
                out.push(serde_yaml::from_str(text).expect("validated at build time"));
            }
        }
        for d in dir.dirs() {
            walk(d, out);
        }
    }
    let mut out = Vec::new();
    walk(&EMBEDDED, &mut out);
    out
}

/// User overlay: ~/.collective/corpus/*.yaml. Invalid entries warn + skip.
fn overlay() -> Vec<Entry> {
    let Some(base) = directories::BaseDirs::new() else {
        return vec![];
    };
    let dir = base.home_dir().join(".collective/corpus");
    let Ok(read) = fs::read_dir(&dir) else {
        return vec![];
    };
    read.filter_map(|f| f.ok())
        .map(|f| f.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .filter_map(|p| {
            let text = fs::read_to_string(&p).ok()?;
            match serde_yaml::from_str::<Entry>(&text)
                .map_err(|e| e.to_string())
                .and_then(|e| e.validate().map(|_| e))
            {
                Ok(e) => Some(e),
                Err(err) => {
                    eprintln!("warning: skipping {}: {err}", p.display());
                    None
                }
            }
        })
        .collect()
}

fn merge(base: Vec<Entry>, over: Vec<Entry>) -> Vec<Entry> {
    let mut entries = base;
    for e in over {
        entries.retain(|x| x.id != e.id); // overlay wins
        entries.push(e);
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    entries
}

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
    let Ok(read) = fs::read_dir(dir) else {
        return vec![];
    };
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
#[allow(dead_code)]
pub fn embedded_ids() -> std::collections::HashSet<String> {
    embedded().into_iter().map(|e| e.id).collect()
}

pub fn load() -> Vec<Entry> {
    merge(merge(embedded(), packs()), overlay())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_corpus_is_the_starter_only() {
        let entries = embedded();
        assert!(
            entries.len() >= 100,
            "starter shrank unexpectedly: {}",
            entries.len()
        );
        assert!(
            entries.len() < 500,
            "bulk imports leaked back into the binary"
        );
        assert!(entries.iter().any(|e| e.id == "pmset-disable-sleep"));
        assert!(
            !entries
                .iter()
                .any(|e| e.domains.iter().any(|d| d == "tldr-import")),
            "embedded corpus must contain no bulk imports"
        );
    }

    #[test]
    fn overlay_overrides_by_id() {
        let base = embedded();
        let n = base.len();
        let mut clone = base[0].clone();
        clone.title = "OVERRIDDEN".into();
        let merged = merge(base, vec![clone.clone()]);
        assert_eq!(merged.len(), n);
        assert_eq!(
            merged.iter().find(|e| e.id == clone.id).unwrap().title,
            "OVERRIDDEN"
        );
    }

    #[test]
    fn merged_is_sorted_by_id() {
        let merged = merge(embedded(), vec![]);
        let mut sorted = merged.clone();
        sorted.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(
            merged.iter().map(|e| &e.id).collect::<Vec<_>>(),
            sorted.iter().map(|e| &e.id).collect::<Vec<_>>()
        );
    }

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
        assert_eq!(
            hit[0].title, "FROM B",
            "sorted-filename precedence not applied"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_pack_warns_and_skips_without_aborting() {
        let dir = temp_packs_dir("bad");
        std::fs::write(dir.join("broken.json"), "{ not json").unwrap();
        write_pack(&dir, "good.json", "good", "good-id", "GOOD");
        let entries = read_packs(&dir);
        assert_eq!(
            entries.len(),
            1,
            "a corrupt pack must not take the good one down"
        );
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
}
