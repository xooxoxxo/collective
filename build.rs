// Validates every corpus/*.yaml at build time. Bad entry = no binary.
#[path = "src/entry.rs"]
#[allow(dead_code)]
// build script reads only `id`; the binary uses every field
mod entry;

use std::{collections::HashSet, fs, path::Path};

fn main() {
    // corpus/ is embedded in the binary; packs/ is published as fetchable
    // packs. Both are schema-validated here and share one id namespace, so a
    // pack entry that would break the corpus cannot be published either.
    println!("cargo:rerun-if-changed=corpus");
    println!("cargo:rerun-if-changed=packs");
    let mut ids = HashSet::new();
    let mut stack = vec![
        Path::new("corpus").to_path_buf(),
        Path::new("packs").to_path_buf(),
    ];
    while let Some(dir) = stack.pop() {
        for f in fs::read_dir(&dir).expect("corpus/ dir missing") {
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
            }
        }
    }
}
