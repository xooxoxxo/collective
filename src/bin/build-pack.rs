//! Build a distributable pack from a directory of corpus YAML.
//!
//! Usage: build-pack <dir> <name> <version> <license> <description> > pack.json
//!
//! Reuses the same Entry type the build gate uses, so a pack that would fail
//! `cargo build` cannot be published either.

#[path = "../entry.rs"]
mod entry;

use std::{collections::HashSet, fs, path::Path};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, dir, name, version, license, description] = args.as_slice() else {
        eprintln!("usage: build-pack <dir> <name> <version> <license> <description>");
        std::process::exit(1);
    };

    let mut entries = Vec::new();
    let mut ids = HashSet::new();
    let mut stack = vec![Path::new(dir).to_path_buf()];
    while let Some(d) = stack.pop() {
        for f in fs::read_dir(&d).unwrap_or_else(|e| panic!("{dir}: {e}")) {
            let p = f.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "yaml") {
                let text = fs::read_to_string(&p).unwrap();
                let e: entry::Entry = serde_yaml_bw::from_str(&text)
                    .unwrap_or_else(|err| panic!("{}: {err}", p.display()));
                e.validate()
                    .unwrap_or_else(|err| panic!("{}: {err}", p.display()));
                assert!(ids.insert(e.id.clone()), "duplicate id: {}", e.id);
                entries.push(e);
            }
        }
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));

    let pack = serde_json::json!({
        "manifest": {
            "name": name,
            "version": version,
            "description": description,
            "source": "https://github.com/xooxoxxo/collective",
            "license": license,
            "count": entries.len(),
            "origin": ""
        },
        "entries": entries
    });
    println!("{}", serde_json::to_string(&pack).unwrap());
    eprintln!("built pack {name} with {} entries", entries.len());
}
