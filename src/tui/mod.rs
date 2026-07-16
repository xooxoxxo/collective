use crate::entry::Entry;
use crate::search;
use std::collections::HashSet;

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
