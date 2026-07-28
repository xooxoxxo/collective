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

pub fn load() -> Vec<Entry> {
    merge(embedded(), overlay())
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
}
