mod entry;
mod corpus;
mod search;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "col", about = "hacky script directory + console drills")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fuzzy-search the corpus
    Search { query: Vec<String> },
}

fn main() {
    let cli = Cli::parse();
    let entries = corpus::load();
    match cli.cmd {
        Cmd::Search { query } => cmd_search(&entries, &query.join(" ")),
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
