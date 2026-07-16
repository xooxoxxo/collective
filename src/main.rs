mod entry;
mod corpus;
mod search;
mod sm2;
mod drill;
mod favorites;
mod tui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "collective", about = "hacky script directory + console drills")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fuzzy-search the corpus
    Search { query: Vec<String> },
    /// Show full entry: cmd, explanation, undo, danger, source
    Show { id: String },
    /// Copy the entry's command to the clipboard
    Copy { id: String },
    /// Print one random gem
    Random,
    /// Flashcard drill session (SM-2 spaced repetition)
    Drill {
        #[arg(long)]
        domain: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    let entries = corpus::load();
    match cli.cmd {
        None => {
            if let Err(e) = tui::run() {
                eprintln!("tui error: {e}");
                std::process::exit(1);
            }
        }
        Some(Cmd::Search { query }) => cmd_search(&entries, &query.join(" ")),
        Some(Cmd::Show { id }) => cmd_show(&entries, &id),
        Some(Cmd::Copy { id }) => cmd_copy(&entries, &id),
        Some(Cmd::Random) => cmd_show(&entries, &random_id(&entries)),
        Some(Cmd::Drill { domain }) => drill::run(&entries, domain.as_deref()),
    }
}

fn cmd_search(entries: &[entry::Entry], query: &str) {
    let hits = search::search(entries, query);
    if hits.is_empty() {
        eprintln!("no matches for '{query}'");
        std::process::exit(1);
    }
    for (e, _) in hits {
        let preview: String = e.cmd.chars().take(48).collect();
        println!("{:<28} {:<44} {}", e.id, truncate(&e.title, 44), preview);
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "…"
    }
}

fn find<'a>(entries: &'a [entry::Entry], id: &str) -> &'a entry::Entry {
    entries.iter().find(|e| e.id == id).unwrap_or_else(|| {
        eprintln!("no entry '{id}' — try: collective search {id}");
        std::process::exit(1);
    })
}

fn cmd_show(entries: &[entry::Entry], id: &str) {
    use entry::Danger;
    let e = find(entries, id);
    println!("{}  [{}]", e.title, e.domains.join(", "));
    if e.danger == Danger::High {
        println!("\x1b[1;31m⚠ DANGER: high — know your exit before you run this.\x1b[0m");
        if let Some(u) = e.undo.as_deref().filter(|u| !u.is_empty()) {
            println!("\x1b[31m  undo: {u}\x1b[0m");
        }
    }
    println!("\n  {}\n", e.cmd);
    if e.danger != Danger::High {
        if let Some(u) = e.undo.as_deref().filter(|u| !u.is_empty()) {
            println!("undo: {u}");
        }
    }
    println!("{}", e.explanation.trim());
    println!("source: {}", e.source);
}

fn cmd_copy(entries: &[entry::Entry], id: &str) {
    let e = find(entries, id);
    match arboard::Clipboard::new().and_then(|mut c| c.set_text(e.cmd.clone())) {
        Ok(()) => println!("copied: {}", e.cmd),
        Err(err) => {
            eprintln!("clipboard failed ({err}); here it is:\n{}", e.cmd);
            std::process::exit(1);
        }
    }
}

fn random_id(entries: &[entry::Entry]) -> String {
    use rand::seq::SliceRandom;
    entries
        .choose(&mut rand::thread_rng())
        .expect("corpus is never empty")
        .id
        .clone()
}
