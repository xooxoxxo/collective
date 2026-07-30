use crate::entry::{Danger, Entry};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut prev_hyphen = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_hyphen = false;
        } else if !prev_hyphen {
            out.push('-');
            prev_hyphen = true;
        }
    }
    out.trim_matches('-').to_string()
}

pub fn uniquify(base: &str, existing: &HashSet<String>) -> String {
    if !existing.contains(base) {
        return base.to_string();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !existing.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

pub fn write_entry(dir: &Path, e: &Entry) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.yaml", e.id));
    fs::write(
        &path,
        serde_yaml_bw::to_string(e).expect("entry serializes"),
    )?;
    Ok(path)
}

use crate::ai;
use std::io::{self, Write};

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn ask(prompt: &str, default: &str) -> String {
    print!("{prompt}");
    if !default.is_empty() {
        print!(" [{default}]");
    }
    print!(": ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
        return default.to_string();
    }
    let t = line.trim();
    if t.is_empty() {
        default.to_string()
    } else {
        t.to_string()
    }
}

fn csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Build an Entry from AI-populated fields.
fn from_ai(cmd: &str, f: ai::AiFields) -> Entry {
    Entry {
        id: String::new(), // filled by caller after uniquify
        title: f.title,
        cmd: cmd.to_string(),
        undo: (!f.undo.is_empty()).then_some(f.undo),
        app: None,
        platform: if f.platform.is_empty() {
            vec!["macos".into()]
        } else {
            f.platform
        },
        domains: if f.domains.is_empty() {
            vec!["shell".into()]
        } else {
            f.domains
        },
        danger: Danger::parse(&f.danger).unwrap_or(Danger::Low),
        explanation: f.explanation,
        source: format!("collect:{}", hostname()),
        tags: f.tags,
    }
}

fn from_manual(cmd: &str) -> Entry {
    let title = ask("title", "");
    let explanation = ask("explanation", "");
    let domains = csv(&ask("domains (comma-sep)", "shell"));
    let danger = loop {
        match Danger::parse(&ask("danger (low/medium/high)", "low")) {
            Some(d) => break d,
            None => println!("  must be low, medium, or high"),
        }
    };
    let tags = csv(&ask("tags (comma-sep)", ""));
    let undo = ask("undo command", "");
    let platform = csv(&ask("platform (comma-sep)", "macos"));
    Entry {
        id: String::new(),
        title,
        cmd: cmd.to_string(),
        undo: (!undo.is_empty()).then_some(undo),
        app: None,
        platform,
        domains,
        danger,
        explanation,
        source: format!("collect:{}", hostname()),
        tags,
    }
}

pub fn run(command: Option<String>, manual: bool, last: bool) {
    let cmd: String = if last {
        match std::env::var("COLLECTIVE_LAST_CMD") {
            Ok(c) if !c.trim().is_empty() => c.trim().to_string(),
            _ => {
                eprintln!("--last needs the shell wrapper — run 'collective --print-shell <shell>' and reload, or pass the command explicitly");
                std::process::exit(1);
            }
        }
    } else {
        match command {
            Some(c) if !c.trim().is_empty() => c,
            _ => {
                eprintln!("nothing to collect — pass a command or use --last");
                std::process::exit(1);
            }
        }
    };
    let cmd = cmd.as_str();
    let use_ai = if manual {
        false
    } else {
        matches!(
            ask("Populate with AI, or fill in manually? [a/m]", "a")
                .to_lowercase()
                .as_str(),
            "a" | "ai" | ""
        )
    };

    let mut entry = if use_ai {
        match ai::populate(cmd) {
            Ok(fields) => from_ai(cmd, fields),
            Err(e) => {
                eprintln!("AI populate failed ({e}); falling back to manual.");
                from_manual(cmd)
            }
        }
    } else {
        from_manual(cmd)
    };

    // assign a unique id from existing corpus + overlay
    let existing: HashSet<String> = crate::corpus::load().into_iter().map(|e| e.id).collect();
    entry.id = uniquify(&slug(&entry.title), &existing);

    // Known limitation: an empty or punctuation-only title slugs to "", which
    // validate() rejects below only after the user has entered every field.
    // A future improvement is to re-prompt for the title interactively instead
    // of failing at the end.
    if let Err(e) = entry.validate() {
        eprintln!("cannot save: {e}");
        std::process::exit(1);
    }
    let dir = crate::favorites::default_path()
        .parent()
        .expect("home has parent")
        .join("corpus");
    match write_entry(&dir, &entry) {
        Ok(path) => println!("saved {} -> {}", entry.id, path.display()),
        Err(e) => {
            eprintln!("write failed: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_kebab() {
        assert_eq!(slug("Disable Sleep, Entirely!"), "disable-sleep-entirely");
        assert_eq!(slug("  git   reflog  "), "git-reflog");
    }

    #[test]
    fn uniquify_appends_suffix() {
        let mut existing = HashSet::new();
        existing.insert("git-reflog".to_string());
        existing.insert("git-reflog-2".to_string());
        assert_eq!(uniquify("git-reflog", &existing), "git-reflog-3");
        assert_eq!(uniquify("fresh", &existing), "fresh");
    }

    #[test]
    fn assembled_entry_validates_and_roundtrips() {
        let e = Entry {
            id: "test-entry".into(),
            title: "Test entry".into(),
            cmd: "echo hi".into(),
            undo: None,
            app: None,
            platform: vec!["macos".into()],
            domains: vec!["shell".into()],
            danger: Danger::Low,
            explanation: "Prints hi.".into(),
            source: "collect:testhost".into(),
            tags: vec!["echo".into()],
        };
        assert!(e.validate().is_ok());
        let dir = std::env::temp_dir().join("col-collect-test");
        let _ = fs::remove_dir_all(&dir);
        let path = write_entry(&dir, &e).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let back: Entry = serde_yaml_bw::from_str(&text).unwrap();
        assert_eq!(back.id, e.id);
        assert_eq!(back.danger, Danger::Low);
        assert_eq!(back.cmd, "echo hi");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn danger_parses() {
        assert_eq!(Danger::parse("high"), Some(Danger::High));
        assert_eq!(Danger::parse("bogus"), None);
    }
}
