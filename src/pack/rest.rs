use super::parse::parse;
use super::store::{install, installed};
use super::types::{classify, owner_repo_url, validate_pack_name, Arg, Manifest};
use serde::{Deserialize, Serialize};
use std::io::Read;

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
    use crate::pack::testutil::{no_embedded, pack_with, temp_dir};

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
