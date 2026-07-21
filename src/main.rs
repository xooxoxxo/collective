mod entry;
mod corpus;
mod search;
mod sm2;
mod drill;
mod favorites;
mod tui;
mod ai; // consumed by collect (Task 6)
mod collect;
mod placeholder;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};

#[derive(Parser)]
#[command(name = "collective", about = "hacky script directory + console drills")]
struct Cli {
    /// Print the shell wrapper for zsh or bash, then exit
    #[arg(long, value_name = "SHELL")]
    print_shell: Option<String>,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fuzzy-search the corpus
    Search {
        query: Vec<String>,
        /// Only entries in this domain (e.g. git, network)
        #[arg(long)]
        domain: Option<String>,
        /// Exclude bulk tldr imports; curated entries only
        #[arg(long)]
        curated: bool,
    },
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
    /// Capture a command into your personal corpus (overlay)
    Collect {
        /// The command to save (optional with --last)
        command: Option<String>,
        /// Skip the AI prompt and enter fields manually
        #[arg(long)]
        manual: bool,
        /// Capture the previous shell command (needs the shell wrapper)
        #[arg(long)]
        last: bool,
    },
    /// Print a shell completion script (zsh, bash, fish)
    Completions {
        /// Target shell
        shell: String,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Some(shell) = cli.print_shell.as_deref() {
        match shell {
            "zsh" => print!("{}", include_str!("../shell/collective.zsh")),
            "bash" => print!("{}", include_str!("../shell/collective.bash")),
            other => {
                eprintln!("unknown shell '{other}' (use zsh or bash)");
                std::process::exit(1);
            }
        }
        return;
    }
    // Handle completions before loading corpus (doesn't need entries)
    if let Some(Cmd::Completions { shell }) = &cli.cmd {
        let parsed: Shell = match shell.parse() {
            Ok(s) => s,
            Err(_) => {
                eprintln!("unknown shell '{shell}' (use zsh, bash, or fish)");
                std::process::exit(1);
            }
        };
        generate(parsed, &mut Cli::command(), "collective", &mut std::io::stdout());
        return;
    }

    let entries = corpus::load();
    match cli.cmd {
        None => {
            if let Err(e) = tui::run() {
                eprintln!("tui error: {e}");
                std::process::exit(1);
            }
        }
        Some(Cmd::Search { query, domain, curated }) => {
            cmd_search(&entries, &query.join(" "), domain.as_deref(), curated)
        }
        Some(Cmd::Show { id }) => cmd_show(&entries, &id),
        Some(Cmd::Copy { id }) => cmd_copy(&entries, &id),
        Some(Cmd::Random) => cmd_show(&entries, &random_id(&entries)),
        Some(Cmd::Drill { domain }) => drill::run(&entries, domain.as_deref()),
        Some(Cmd::Collect { command, manual, last }) => collect::run(command, manual, last),
        Some(Cmd::Completions { .. }) => unreachable!("handled before corpus load"),
    }
}

fn cmd_search(entries: &[entry::Entry], query: &str, domain: Option<&str>, curated: bool) {
    let filtered: Vec<entry::Entry> = entries
        .iter()
        .filter(|e| domain.is_none_or(|d| e.domains.iter().any(|x| x == d)))
        .filter(|e| !curated || !search::is_bulk_import(e))
        .cloned()
        .collect();
    let hits = search::search(&filtered, query);
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
    let cmd = placeholder::fill_interactive(&e.cmd);
    match arboard::Clipboard::new().and_then(|mut c| c.set_text(cmd.clone())) {
        Ok(()) => println!("copied: {cmd}"),
        Err(err) => {
            eprintln!("clipboard failed ({err}); here it is:\n{cmd}");
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
