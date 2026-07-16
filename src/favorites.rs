use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_path() -> PathBuf {
    directories::BaseDirs::new()
        .expect("cannot locate home directory")
        .home_dir()
        .join(".collective/favorites.json")
}

pub fn load(path: &Path) -> HashSet<String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| {
            eprintln!("warning: favorites corrupt at {}, resetting", path.display());
            HashSet::new()
        }),
        Err(_) => HashSet::new(),
    }
}

pub fn save(path: &Path, favs: &HashSet<String>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut sorted: Vec<&String> = favs.iter().collect();
    sorted.sort();
    fs::write(path, serde_json::to_string_pretty(&sorted).expect("favorites serialize"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("col-fav-test-{name}.json"))
    }

    #[test]
    fn missing_file_gives_empty() {
        let p = tmp("missing");
        let _ = fs::remove_file(&p);
        assert!(load(&p).is_empty());
    }

    #[test]
    fn roundtrips() {
        let p = tmp("round");
        let mut favs = HashSet::new();
        favs.insert("pmset-disable-sleep".to_string());
        favs.insert("flush-dns-cache".to_string());
        save(&p, &favs).unwrap();
        let loaded = load(&p);
        assert_eq!(loaded, favs);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn corrupt_file_resets_without_panic() {
        let p = tmp("corrupt");
        fs::write(&p, "not json !!").unwrap();
        assert!(load(&p).is_empty());
        let _ = fs::remove_file(&p);
    }
}
