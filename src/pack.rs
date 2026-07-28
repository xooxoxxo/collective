#![allow(dead_code)]
// Pack types and validators are consumed by Tasks 6-7; suppress dead-code warnings
// until then. This allow will be removed when those consumers land.

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
        Err(format!(
            "bad pack name {name:?}: use lowercase/digits/hyphens"
        ))
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
            return Err(format!(
                "bad source address {arg:?}: expected <owner>/<repo>"
            ));
        }
        return Ok(Arg::OwnerRepo(owner.into(), repo.into()));
    }
    validate_pack_name(arg)?;
    Ok(Arg::Name(arg.into()))
}

pub fn owner_repo_url(owner: &str, repo: &str) -> String {
    format!("https://raw.githubusercontent.com/{owner}/{repo}/HEAD/pack.json")
}

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
            eprintln!(
                "warning: skipping entry in pack {}: {err}",
                pack.manifest.name
            );
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

/// Manifests of every installed pack, sorted by filename. A pack that fails to
/// parse is skipped so one bad file cannot break `pack list`.
pub fn installed(dir: &std::path::Path) -> Vec<Manifest> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return vec![];
    };
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn accepts_plain_pack_names() {
        assert!(validate_pack_name("tldr").is_ok());
        assert!(validate_pack_name("my-pack-2").is_ok());
    }

    #[test]
    fn rejects_names_that_could_escape_the_packs_dir() {
        for bad in [
            "..",
            ".",
            "../../.zshrc",
            "a/b",
            "",
            "Tldr",
            "a b",
            "a.json",
        ] {
            assert!(
                validate_pack_name(bad).is_err(),
                "accepted bad name {bad:?}"
            );
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
            "../../etc", // traversal via the owner segment
            "owner/..",  // traversal via the repo segment
            "../repo",
            "owner/re/po", // extra segment
            "/repo",       // empty owner
            "owner/",      // empty repo
            "own er/repo", // space
            "owner/re?po", // query injection into the URL
            "owner/re#po", // fragment injection
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
        assert_eq!(
            pack.manifest.origin, "",
            "origin defaults empty when absent"
        );
        assert_eq!(pack.entries.len(), 1);
        assert!(pack.entries[0].validate().is_ok());
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
}
