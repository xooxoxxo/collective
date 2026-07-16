mod ui;

use crate::entry::Entry;
use crate::favorites;
use crate::search;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::collections::HashSet;
use std::io::{self, Write};

pub struct App {
    pub all: Vec<Entry>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filter: String,
    pub favorites: HashSet<String>,
    pub fav_only: bool,
}

impl App {
    pub fn new(all: Vec<Entry>, favorites: HashSet<String>) -> App {
        let mut app = App {
            all,
            filtered: Vec::new(),
            selected: 0,
            filter: String::new(),
            favorites,
            fav_only: false,
        };
        app.recompute();
        app
    }

    fn recompute(&mut self) {
        let mut idx: Vec<usize> = if self.filter.trim().is_empty() {
            (0..self.all.len()).collect()
        } else {
            // map search results back to indices in `all`
            let hits = search::search(&self.all, &self.filter);
            hits.iter()
                .filter_map(|(e, _)| self.all.iter().position(|x| x.id == e.id))
                .collect()
        };
        if self.fav_only {
            idx.retain(|&i| self.favorites.contains(&self.all[i].id));
        }
        self.filtered = idx;
        self.selected = 0;
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        self.recompute();
    }

    pub fn move_down(&mut self) {
        if !self.filtered.is_empty() && self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn toggle_fav_only(&mut self) {
        self.fav_only = !self.fav_only;
        self.recompute();
    }

    pub fn toggle_star(&mut self) -> Option<String> {
        let id = self.selected_entry()?.id.clone();
        if !self.favorites.remove(&id) {
            self.favorites.insert(id.clone());
        }
        if self.fav_only {
            self.recompute();
        }
        Some(id)
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.filtered.get(self.selected).map(|&i| &self.all[i])
    }

    pub fn visible(&self) -> Vec<&Entry> {
        self.filtered.iter().map(|&i| &self.all[i]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus;

    fn app() -> App {
        App::new(corpus::load(), HashSet::new())
    }

    #[test]
    fn new_shows_all_sorted() {
        let a = app();
        assert_eq!(a.filtered.len(), a.all.len());
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn filter_narrows_and_resets_selection() {
        let mut a = app();
        a.move_down();
        a.set_filter("disable sleep");
        assert!(a.visible().len() < a.all.len());
        assert_eq!(a.selected, 0);
        assert_eq!(a.selected_entry().unwrap().id, "pmset-disable-sleep");
    }

    #[test]
    fn move_clamps() {
        let mut a = app();
        a.set_filter("zzqqxxnothing"); // empty result
        a.move_down();
        assert_eq!(a.selected, 0);
        assert!(a.selected_entry().is_none());
    }

    #[test]
    fn toggle_star_adds_then_removes() {
        let mut a = app();
        a.set_filter("disable sleep");
        let id = a.toggle_star().unwrap();
        assert_eq!(id, "pmset-disable-sleep");
        assert!(a.favorites.contains("pmset-disable-sleep"));
        a.toggle_star();
        assert!(!a.favorites.contains("pmset-disable-sleep"));
    }

    #[test]
    fn fav_only_filters_to_favorites() {
        let mut a = app();
        a.set_filter("disable sleep");
        a.toggle_star(); // star pmset-disable-sleep
        a.set_filter("");
        a.toggle_fav_only();
        assert_eq!(a.visible().len(), 1);
        assert_eq!(a.visible()[0].id, "pmset-disable-sleep");
    }
}

fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}

pub fn run() -> io::Result<()> {
    // Ensure the terminal is restored even on panic.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        prev(info);
    }));

    let fav_path = favorites::default_path();
    let mut app = App::new(crate::corpus::load(), favorites::load(&fav_path));
    let mut picked: Option<String> = None;

    let result = (|| -> io::Result<()> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

        loop {
            terminal.draw(|f| ui::draw(f, &app))?;
            let Event::Key(key) = event::read()? else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Char('q') if app.filter.is_empty() => break,
                KeyCode::Up => app.move_up(),
                KeyCode::Down => app.move_down(),
                KeyCode::Enter => {
                    if let Some(e) = app.selected_entry() {
                        picked = Some(e.cmd.clone());
                    }
                    break;
                }
                KeyCode::Char('\n') => {}
                KeyCode::Backspace => {
                    let mut f = app.filter.clone();
                    f.pop();
                    app.set_filter(&f);
                }
                // Ctrl-less bare keys drive both filter text AND actions:
                // reserve f/F/y for actions, everything else types into filter.
                KeyCode::Char('y') => {
                    if let Some(e) = app.selected_entry() {
                        let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text(e.cmd.clone()));
                    }
                }
                KeyCode::Char('f') => {
                    if let Some(_id) = app.toggle_star() {
                        let _ = favorites::save(&fav_path, &app.favorites);
                    }
                }
                KeyCode::Char('F') => app.toggle_fav_only(),
                KeyCode::Char(c) => {
                    let mut f = app.filter.clone();
                    f.push(c);
                    app.set_filter(&f);
                }
                _ => {}
            }
        }
        Ok(())
    })();

    restore();
    let _ = std::panic::take_hook();
    result?;

    if let Some(cmd) = picked {
        deliver(&cmd);
    }
    Ok(())
}

fn deliver(cmd: &str) {
    let _ = arboard::Clipboard::new().and_then(|mut c| c.set_text(cmd.to_string()));
    match std::env::var("COLLECTIVE_PICK") {
        Ok(path) if !path.is_empty() => {
            // Wrapper reads this file and places the command on the prompt.
            let _ = std::fs::write(path, cmd);
        }
        _ => {
            // No wrapper: print so the user can copy/paste.
            println!("{cmd}");
            let _ = io::stdout().flush();
        }
    }
}
