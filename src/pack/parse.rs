use super::types::{validate_pack_name, Pack};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
