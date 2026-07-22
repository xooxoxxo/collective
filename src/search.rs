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

    #[test]
    fn curated_hits_all_precede_imports() {
        let entries = corpus::load();
        // "git log" matches curated gems and many tldr imports.
        let hits = search(&entries, "git log");
        assert!(!hits.is_empty());
        if let Some(first_import) = hits.iter().position(|(e, _)| is_bulk_import(e)) {
            assert!(
                hits[first_import..].iter().all(|(e, _)| is_bulk_import(e)),
                "found a curated hit after an import hit"
            );
            assert!(first_import > 0, "expected at least one curated hit first");
        }
    }

    #[test]
    fn both_groups_share_the_cap() {
        let entries = corpus::load();
        // "git" is a broad query with both curated and import hits.
        let hits = search(&entries, "git");
        assert!(hits.len() <= 10, "result exceeds 10 row cap");
        let has_curated = hits.iter().any(|(e, _)| !is_bulk_import(e));
        let has_imports = hits.iter().any(|(e, _)| is_bulk_import(e));
        if has_curated && has_imports {
            // Both groups present: verify grouping and cap enforcement
            if let Some(first_import) = hits.iter().position(|(e, _)| is_bulk_import(e)) {
                assert!(first_import > 0, "expected at least one curated hit first");
                assert!(first_import <= 6, "curated group exceeded 6-slot cap");
                let import_count = hits[first_import..].len();
                assert!(import_count <= 4, "import group exceeded 4-slot cap");
            }
        }
    }
}
