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
}
