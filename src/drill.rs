use crate::sm2::Card;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_state_path() -> PathBuf {
    directories::BaseDirs::new()
        .expect("cannot locate home directory")
        .home_dir()
        .join(".collective/drill.json")
}

pub fn load_state(path: &Path) -> HashMap<String, Card> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| {
            eprintln!("warning: drill state corrupt at {}, resetting", path.display());
            HashMap::new()
        }),
        Err(_) => HashMap::new(),
    }
}

pub fn save_state(path: &Path, state: &HashMap<String, Card>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state).expect("state serializes"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sm2::Card;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("col-drill-test-{name}.json"))
    }

    #[test]
    fn missing_file_gives_empty_state() {
        let p = tmp("missing");
        let _ = std::fs::remove_file(&p);
        assert!(load_state(&p).is_empty());
    }

    #[test]
    fn roundtrips_state() {
        let p = tmp("roundtrip");
        let mut state = std::collections::HashMap::new();
        state.insert("pmset-disable-sleep".to_string(), Card::default());
        save_state(&p, &state).unwrap();
        let loaded = load_state(&p);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded["pmset-disable-sleep"], Card::default());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn corrupt_file_resets_without_panic() {
        let p = tmp("corrupt");
        std::fs::write(&p, "{ not json !!").unwrap();
        assert!(load_state(&p).is_empty());
        let _ = std::fs::remove_file(&p);
    }
}
