mod fetch;
mod parse;
mod registry;
mod store;
mod types;

pub use fetch::add;
pub use parse::parse;
#[allow(unused_imports)]
pub use registry::{search_registry, update, Registry, RegistryPack, REGISTRY_URL};
pub use store::{installed, remove};
#[allow(unused_imports)]
pub use types::{classify, owner_repo_url, validate_pack_name, Arg, Manifest, Pack};

#[cfg(test)]
pub(in crate::pack) mod testutil {
    use super::parse::parse;
    use super::types::Pack;
    use std::collections::HashSet;
    use std::path::PathBuf;

    pub(crate) fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("col-pk-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    pub(crate) fn seed(dir: &std::path::Path, name: &str) {
        let json = format!(
            r#"{{"manifest":{{"name":"{name}","version":"1.0.0","count":0}},"entries":[]}}"#
        );
        std::fs::write(dir.join(format!("{name}.json")), json).unwrap();
    }

    pub(crate) fn no_embedded() -> HashSet<String> {
        HashSet::new()
    }

    pub(crate) fn pack_with(name: &str, id: &str) -> Pack {
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
}
