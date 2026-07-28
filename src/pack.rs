use crate::entry::Entry;
use serde::{Deserialize, Serialize};
use std::io::Read;
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

/// Packs are data, not archives: 32 MB is far above any plausible corpus and
/// far below anything that would exhaust memory.
const MAX_PACK_BYTES: u64 = 32 * 1024 * 1024;

/// One HTTPS GET of one JSON document. Redirects are capped at 5 and must stay
/// enabled: GitHub release assets 302 to objects.githubusercontent.com, so
/// disabling them would break `pack add` outright. The cap is set explicitly
/// rather than inherited, so the behaviour cannot drift with a library default.
/// The body is bounded by `take` rather than by trusting `content-length`,
/// which a hostile server can understate.
fn fetch(url: &str) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err(format!("refusing non-https url: {url}"));
    }
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(std::time::Duration::from_secs(10)))
        .timeout_recv_body(Some(std::time::Duration::from_secs(60)))
        .max_redirects(5)
        .build();
    let agent: ureq::Agent = config.into();
    // ureq 3's error text omits the URL, so name it here: "http status: 404"
    // alone tells the user nothing about which pack or registry failed.
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("fetch failed for {url}: {e}"))?;
    let mut buf = String::new();
    resp.into_body()
        .into_reader()
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
            installed(dir).into_iter().filter(|m| m.name == n).collect()
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

        // Test multiple escape payloads, verifying both error and filesystem outcome.
        for escape_name in ["../../pwned", "..", "a/b"] {
            let src = dir.join("evil.json");
            std::fs::write(
                &src,
                format!(r#"{{"manifest":{{"name":"{escape_name}","count":0}},"entries":[]}}"#),
            )
            .unwrap();

            // add() must reject the name and return an error.
            assert!(
                add(&dir, src.to_str().unwrap(), &no_embedded()).is_err(),
                "failed to reject escape payload {escape_name:?}"
            );

            // Verify the exact path a missing-guard implementation would write to does not exist.
            let escaped = dir.join(format!("{escape_name}.json"));
            assert!(
                !escaped.exists(),
                "escape payload {escape_name:?} wrote a file to {}",
                escaped.display()
            );

            let _ = std::fs::remove_file(&src);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_filters_by_name_and_description() {
        let json = r#"{"packs":[
            {"name":"tldr","description":"tldr-pages bulk import","url":"https://x.test/t.json"},
            {"name":"kube","description":"kubernetes recipes","url":"https://x.test/k.json"}]}"#;
        let reg: Registry = serde_json::from_str(json).unwrap();
        assert_eq!(filter_registry(&reg, "tldr").len(), 1);
        assert_eq!(
            filter_registry(&reg, "kubernetes").len(),
            1,
            "must match description"
        );
        assert_eq!(
            filter_registry(&reg, "").len(),
            2,
            "empty query lists everything"
        );
        assert_eq!(filter_registry(&reg, "nothing").len(), 0);
    }

    #[test]
    fn registry_lookup_rejects_a_non_https_url() {
        let reg: Registry = serde_json::from_str(
            r#"{"packs":[{"name":"evil","description":"","url":"http://x.test/e.json"}]}"#,
        )
        .unwrap();
        assert!(
            lookup_registry(&reg, "evil").is_err(),
            "non-https registry url accepted"
        );
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
        install(
            &dir,
            pack_with("demo", "d"),
            "file:///tmp/x.json",
            &no_embedded(),
        )
        .unwrap();
        let report = update(&dir, Some("demo"), &no_embedded()).unwrap();
        assert!(
            report.contains("skipped"),
            "local-origin pack must be skipped: {report}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
