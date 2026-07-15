use crate::entry::Entry;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Bulk tldr-pages imports live in this domain; they are ranked below
/// hand-curated entries so the good stuff surfaces first.
fn is_bulk_import(e: &Entry) -> bool {
    e.domains.iter().any(|d| d == "tldr-import")
}

/// Weighted fuzzy search: title 3x, best tag 2x, cmd 1x. Curated entries score
/// at full weight; bulk tldr imports at half so they don't drown the gems.
/// Top 10, best first.
pub fn search<'a>(entries: &'a [Entry], query: &str) -> Vec<(&'a Entry, u32)> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buf = Vec::new();
    let mut score_of = |text: &str, matcher: &mut Matcher| -> u32 {
        buf.clear();
        let hay = Utf32Str::new(text, &mut buf);
        pattern.score(hay, matcher).unwrap_or(0)
    };
    let mut scored: Vec<(&Entry, u32)> = entries
        .iter()
        .filter_map(|e| {
            let title = score_of(&e.title, &mut matcher);
            let tag = e
                .tags
                .iter()
                .map(|t| score_of(t, &mut matcher))
                .max()
                .unwrap_or(0);
            let cmd = score_of(&e.cmd, &mut matcher);
            let raw = 3 * title + 2 * tag + cmd;
            // Curated ×2, bulk imports ×1 — a stable integer boost that keeps
            // curated entries ahead of imports at equal match strength.
            let s = raw * if is_bulk_import(e) { 1 } else { 2 };
            (raw > 0).then_some((e, s))
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.id.cmp(&b.0.id)));
    scored.truncate(10);
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus;

    #[test]
    fn finds_pmset_for_sleep_query() {
        let entries = corpus::load();
        let hits = search(&entries, "disable sleep");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].0.id, "pmset-disable-sleep");
    }

    #[test]
    fn title_outranks_cmd_only_match() {
        let entries = corpus::load();
        let hits = search(&entries, "screenshot");
        assert_eq!(hits[0].0.id, "screenshot-location");
    }

    #[test]
    fn caps_at_ten_results() {
        let entries = corpus::load();
        assert!(search(&entries, "a").len() <= 10);
    }

    #[test]
    fn no_match_returns_empty() {
        let entries = corpus::load();
        assert!(search(&entries, "zzqqxxnothing").is_empty());
    }

    #[test]
    fn curated_outranks_bulk_import() {
        // "port" matches curated gems (e.g. mac-find-process-by-port) and many
        // tldr imports. The first result must be curated, not a bulk import.
        let entries = corpus::load();
        let hits = search(&entries, "process listening port");
        assert!(!hits.is_empty());
        assert!(
            !hits[0].0.domains.iter().any(|d| d == "tldr-import"),
            "top hit was a bulk import: {}",
            hits[0].0.id
        );
    }
}
