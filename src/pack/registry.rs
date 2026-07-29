use super::fetch::fetch;
use super::parse::parse;
use super::store::{install, installed};
use super::types::{validate_pack_name, Manifest};
use serde::{Deserialize, Serialize};

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

pub(super) fn registry_url_for(name: &str) -> Result<String, String> {
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
