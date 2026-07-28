use crate::entry::Entry;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// Bulk tldr-pages imports live in this domain; they are ranked below
/// hand-curated entries so the good stuff surfaces first.
pub(crate) fn is_bulk_import(e: &Entry) -> bool {
    e.domains.iter().any(|d| d == "tldr-import")
}

/// Weighted fuzzy search: title 3x, best tag 2x, cmd 1x. Curated matches are
/// always grouped before bulk tldr imports. Top 10: curated first (max 6 when
/// imports also match), imports fill the rest.
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
            (raw > 0).then_some((e, raw))
        })
        .collect();
    // Curated entries always precede bulk imports; within a group, best
    // score first, id as the tie-break. Deterministic — replaces the old
    // probabilistic x2 score boost.
    scored.sort_by(|a, b| {
        is_bulk_import(a.0)
            .cmp(&is_bulk_import(b.0))
            .then(b.1.cmp(&a.1))
            .then(a.0.id.cmp(&b.0.id))
    });
    // Per-group cap: 10 rows total, imports guaranteed up to 4 slots when
    // both groups match, either group backfills when the other runs short.
    let (curated, imports): (Vec<(&Entry, u32)>, Vec<(&Entry, u32)>) =
        scored.into_iter().partition(|(e, _)| !is_bulk_import(e));
    let import_take = imports.len().min(4);
    let curated_take = curated.len().min(10 - import_take);
    let import_take = imports.len().min(10 - curated_take);
    let mut out = curated;
    out.truncate(curated_take);
    out.extend(imports.into_iter().take(import_take));
    out
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

    /// Throwaway entry for ranking tests. `domain` decides curated vs import:
    /// "tldr-import" makes it a bulk import, anything else makes it curated.
    fn fixture(id: &str, title: &str, domain: &str) -> Entry {
        Entry {
            id: id.into(),
            title: title.into(),
            cmd: format!("run {id}"),
            undo: None,
            platform: vec!["macos".into()],
            domains: vec![domain.into()],
            danger: crate::entry::Danger::Low,
            explanation: "fixture".into(),
            source: "fixture".into(),
            tags: vec![],
        }
    }

    #[test]
    fn curated_outranks_bulk_import() {
        // The import is the better textual match; grouping must still win.
        let entries = vec![
            fixture("import-exact", "git log graph", "tldr-import"),
            fixture("curated-weak", "git log graph display", "vcs"),
        ];
        let hits = search(&entries, "git log graph");
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].0.id, "curated-weak",
            "import outranked a curated entry"
        );
    }

    #[test]
    fn curated_hits_all_precede_imports() {
        let mut entries = vec![];
        for i in 0..3 {
            entries.push(fixture(&format!("import-{i}"), "git log", "tldr-import"));
            entries.push(fixture(&format!("curated-{i}"), "git log", "vcs"));
        }
        let hits = search(&entries, "git log");
        let first_import = hits
            .iter()
            .position(|(e, _)| is_bulk_import(e))
            .expect("fixture guarantees at least one import hit");
        assert_eq!(first_import, 3, "expected all 3 curated hits first");
        assert!(
            hits[first_import..].iter().all(|(e, _)| is_bulk_import(e)),
            "found a curated hit after an import hit"
        );
    }

    #[test]
    fn both_groups_share_the_cap() {
        // 8 of each: enough that the 6/4 split is forced rather than incidental.
        let mut entries = vec![];
        for i in 0..8 {
            entries.push(fixture(&format!("curated-{i}"), "git log", "vcs"));
            entries.push(fixture(&format!("import-{i}"), "git log", "tldr-import"));
        }
        let hits = search(&entries, "git log");
        assert_eq!(hits.len(), 10, "result must fill the 10 row cap");
        let curated = hits.iter().filter(|(e, _)| !is_bulk_import(e)).count();
        let imports = hits.iter().filter(|(e, _)| is_bulk_import(e)).count();
        assert_eq!(
            curated, 6,
            "curated group must cap at 6 when imports compete"
        );
        assert_eq!(imports, 4, "imports must be guaranteed 4 slots");
    }

    #[test]
    fn one_group_backfills_the_whole_cap() {
        // No imports competing: curated takes all 10, no 6-slot cap applied.
        let entries: Vec<Entry> = (0..12)
            .map(|i| fixture(&format!("curated-{i}"), "git log", "vcs"))
            .collect();
        assert_eq!(search(&entries, "git log").len(), 10);
    }
}
