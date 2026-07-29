use crate::entry::Entry;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// A scored search hit: the entry and its weighted fuzzy score.
type Scored<'a> = (&'a Entry, u32);

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
            // Explanation is scored so you can find an entry by describing what
            // it does, not only by recalling its title or the command itself.
            // Weighted lowest: it is the longest field, and fuzzy matchers
            // reward long haystacks, so an equal weight would let prose drown
            // out titles.
            let explanation = score_of(&e.explanation, &mut matcher);
            let raw = 3 * title + 2 * tag + cmd + explanation;
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
    let (curated, imports): (Vec<Scored>, Vec<Scored>) =
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
    fn finds_an_entry_by_describing_what_it_does() {
        // The query words appear ONLY in the explanation — not in the title,
        // the command, or the tags. Without explanation scoring this entry is
        // unreachable, which was the gap: you had to already know the title or
        // the command to find the command.
        let mut e = fixture("quarantine-strip", "Remove the quarantine flag", "macos");
        e.cmd = "xattr -d com.apple.quarantine".into();
        e.tags = vec!["xattr".into()];
        e.explanation = "Stops Gatekeeper nagging about an unidentified developer.".into();
        let entries = vec![e];

        let hits = search(&entries, "gatekeeper nagging");
        assert_eq!(hits.len(), 1, "explanation-only match must be findable");
        assert_eq!(hits[0].0.id, "quarantine-strip");
    }

    #[test]
    fn title_still_outranks_explanation() {
        // Explanation is searchable but must not drown out titles: it is the
        // longest field, and fuzzy matchers reward long haystacks.
        let mut on_title = fixture("by-title", "flush dns cache", "network");
        on_title.explanation = "Unrelated prose.".into();
        let mut in_prose = fixture("by-prose", "Reset resolver state", "network");
        in_prose.explanation = "Use when you need to flush dns cache after edits.".into();

        let entries = vec![in_prose, on_title];
        let hits = search(&entries, "flush dns cache");
        assert_eq!(
            hits[0].0.id, "by-title",
            "prose match outranked a title match"
        );
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
        // IDs flipped so alphabetic order would put import first without grouping.
        let entries = vec![
            fixture("a-import", "git log graph", "tldr-import"),
            fixture("z-curated", "git log graph display", "vcs"),
        ];
        let hits = search(&entries, "git log graph");
        assert_eq!(hits.len(), 2);
        // Both score 948; grouping tier sorts curated (false) before import (true).
        // If grouping were removed, "a-import" would win on id tie-break.
        assert!(
            hits[0].1 >= hits[1].1,
            "import scored higher: {} >= {}",
            hits[0].1,
            hits[1].1
        );
        assert_eq!(
            hits[0].0.id, "z-curated",
            "grouping must sort curated before import despite lower id"
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
