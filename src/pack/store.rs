use super::parse::parse;
use super::types::{validate_pack_name, Manifest, Pack};
use std::path::PathBuf;

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

/// Write a validated pack to `<dir>/<name>.json`, refusing to land on a pack
/// installed from a different origin. Returns the human-facing report.
pub(super) fn install(
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
    // If the installed pack cannot be parsed, its origin cannot be established, so we
    // allow the install rather than permanently stranding the name. The origin is always
    // written by install() itself, so an empty origin is never a valid case to check.
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
    // Same-directory temp file plus atomic rename: the rename publishes atomically,
    // and a per-process temp name prevents concurrent writers from colliding.
    let tmp = dir.join(format!(".{name}.{}.json.tmp", std::process::id()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::testutil::{no_embedded, pack_with, seed, temp_dir};

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

    #[test]
    fn install_writes_the_pack_and_records_origin() {
        let dir = temp_dir("install");
        install(
            &dir,
            pack_with("demo", "demo-id"),
            "https://example.test/p.json",
            &no_embedded(),
        )
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
        install(
            &dir,
            pack_with("tldr", "official"),
            "https://official.test/p.json",
            &no_embedded(),
        )
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
        assert!(
            text.contains("official"),
            "hostile pack clobbered the official one"
        );
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
        assert!(
            report.contains("flush-dns-cache"),
            "shadowing not reported: {report}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
