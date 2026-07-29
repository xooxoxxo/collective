use crate::entry::Entry;
use crate::sm2::{self, Card};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn default_state_path() -> PathBuf {
    directories::BaseDirs::new()
        .expect("cannot locate home directory")
        .home_dir()
        .join(".collective/drill.json")
}

pub fn load_state(path: &Path) -> HashMap<String, Card> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| {
            eprintln!(
                "warning: drill state corrupt at {}, resetting",
                path.display()
            );
            HashMap::new()
        }),
        Err(_) => HashMap::new(),
    }
}

pub fn save_state(path: &Path, state: &HashMap<String, Card>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(state).expect("state serializes"),
    )
}

pub fn pick_due<'a>(
    entries: &'a [Entry],
    state: &HashMap<String, Card>,
    domain: Option<&str>,
    now: u64,
) -> Vec<&'a Entry> {
    use rand::seq::SliceRandom;
    let mut due: Vec<&Entry> = entries
        .iter()
        .filter(|e| domain.is_none_or(|d| e.domains.iter().any(|x| x == d)))
        .filter(|e| state.get(&e.id).is_none_or(|c| c.due <= now))
        .collect();
    // Shuffle first, then stable-sort by due date: most-overdue cards come
    // first, ties (e.g. all-unseen due=0) resolve randomly so a large corpus
    // does not always drill the same alphabetically-first 20.
    due.shuffle(&mut rand::rng());
    due.sort_by_key(|e| state.get(&e.id).map_or(0, |c| c.due));
    due.truncate(20);
    due
}

pub fn run(entries: &[Entry], domain: Option<&str>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before 1970")
        .as_secs();
    let path = default_state_path();
    let mut state = load_state(&path);
    let due = pick_due(entries, &state, domain, now);
    if due.is_empty() {
        println!("nothing due. come back later.");
        return;
    }
    println!(
        "{} card(s) due. recall the command, Enter reveals.\n",
        due.len()
    );
    let stdin = io::stdin();
    for e in due {
        println!("── {}", e.title);
        print!("your answer (or Enter to reveal): ");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        if stdin.read_line(&mut buf).unwrap() == 0 {
            println!();
            return;
        }
        let typed = buf.trim();
        let correct = !typed.is_empty() && crate::answer::matches(&e.cmd, typed);
        println!("  {}", e.cmd);
        if !typed.is_empty() {
            let mark = if correct {
                "✓ correct"
            } else {
                "✗ not quite"
            };
            println!("  you typed: {typed}  {mark}");
        }
        let outcome = crate::answer::outcome_for(typed.is_empty(), correct);
        let proposed = crate::answer::derived_grade(outcome);
        let label = match proposed {
            1 => "again",
            2 => "hard",
            3 => "good",
            _ => "easy",
        };
        let grade = loop {
            print!("graded: {label}   [Enter accepts · 1-4 overrides]: ");
            io::stdout().flush().unwrap();
            let mut g = String::new();
            match stdin.read_line(&mut g) {
                Ok(0) => {
                    println!("\nsession ended.");
                    return;
                }
                Ok(_) => {
                    let g = g.trim();
                    if g.is_empty() {
                        break proposed;
                    }
                    match g.parse::<u8>() {
                        Ok(n @ 1..=4) => break n,
                        _ => continue,
                    }
                }
                Err(err) => {
                    eprintln!("input error: {err}");
                    return;
                }
            }
        };
        let card = state.get(&e.id).copied().unwrap_or_default();
        state.insert(e.id.clone(), sm2::review(card, grade, now));
        if let Err(err) = save_state(&path, &state) {
            eprintln!("warning: could not save drill state: {err}");
        }
        println!();
    }
    println!("session done.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus;

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

    #[test]
    fn pick_due_includes_unseen_and_excludes_future() {
        let entries = corpus::load();
        let now = 1_800_000_000u64;
        let mut state = std::collections::HashMap::new();
        // one card scheduled far in the future -> excluded
        let future = sm2::review(Card::default(), 4, now);
        state.insert("pmset-disable-sleep".to_string(), future);
        let due = pick_due(&entries, &state, None, now);
        assert!(!due.is_empty());
        assert!(due.len() <= 20);
        // the future-scheduled card is excluded
        assert!(due.iter().all(|e| e.id != "pmset-disable-sleep"));
        // everything returned is genuinely due: unseen, or due at/before now
        assert!(due
            .iter()
            .all(|e| state.get(&e.id).is_none_or(|c| c.due <= now)));
    }

    #[test]
    fn pick_due_filters_by_domain() {
        let entries = corpus::load();
        let state = std::collections::HashMap::new();
        let due = pick_due(&entries, &state, Some("git"), 0);
        assert!(!due.is_empty());
        assert!(due.iter().all(|e| e.domains.iter().any(|d| d == "git")));
    }
}
