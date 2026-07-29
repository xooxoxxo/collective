use super::parse::parse;
use super::store::install;
use super::types::{classify, owner_repo_url, Arg};
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

/// Packs are data, not archives: 32 MB is far above any plausible corpus and
/// far below anything that would exhaust memory.
const MAX_PACK_BYTES: u64 = 32 * 1024 * 1024;

/// One HTTPS GET of one JSON document. Redirects are capped at 5 and must stay
/// enabled: GitHub release assets 302 to objects.githubusercontent.com, so
/// disabling them would break `pack add` outright. The cap is set explicitly
/// rather than inherited, so the behaviour cannot drift with a library default.
/// The body is bounded by `take` rather than by trusting `content-length`,
/// which a hostile server can understate.
pub(super) fn fetch(url: &str) -> Result<String, String> {
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
pub fn add(dir: &Path, source: &str, embedded: &HashSet<String>) -> Result<String, String> {
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
            let url = super::rest::registry_url_for(&name)?;
            (fetch(&url)?, url)
        }
    };
    let pack = parse(&text, None)?;
    install(dir, pack, &origin, embedded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::testutil::{no_embedded, temp_dir};

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
}
