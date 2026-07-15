# Track 2 Research Notes — Gem-Mining Sweep

The Track 2 corpus (`corpus/gems/`) was mined by five parallel research agents,
one per lens, each writing schema-valid YAML directly. 115 gems survived the
schema + danger-sanity + build gate.

## Lenses and yield

| Lens | id prefix | entries | sources |
|---|---|---|---|
| macOS internals | `mac-` | 37 | ss64.com/mac, macos-defaults.com, Apple docs |
| Dotfiles archaeology | `dot-` | 28 | mathiasbynens, paulirish, holman, thoughtbot, skwp dotfiles |
| HN/Reddit threads | `hn-` | 4 | HN & r/commandline "best one-liner" threads |
| Shell wizardry | `shell-` | 25 | ss64, git-scm docs, awk/jq/git plumbing canon |
| Blog canon / modern tools | `blog-` | 21 | fzf/rg/fd/bat/eza/zoxide/entr/tmux/jq canon |

The HN/Reddit lens under-delivered (agent hit an API error mid-run after
writing only 4 entries). Re-mining that thread corpus is the obvious next
harvest — the recurring "share your best CLI trick" threads have far more than
4 gems worth keeping.

## Danger distribution

97 low / 15 medium / 3 high. Destructive-verb audit confirmed nothing
irreversible was mis-rated `low`. High-danger entries: `git push -f`
(`dot-git-undo-push`), an fzf-driven `kill -9` alias (`dot-fzf-kill-process`),
and `git clean` teaching (`shell-git-clean-dryrun`, conservatively high though
the shown form is a dry run).

## Rejected but interesting

Leads the agents surfaced and excluded, worth revisiting:

- Trivial aliases (`..`→`cd ..`, `ll`→`ls -la`, `l`→`ls`) — fail the
  non-obvious quality gate.
- Duplicate `defaults write AppleShowAllExtensions` variants — consolidated to
  one entry.
- Direct `sqlite3` edits of `~/Library/Preferences/*.plist` — kept one as
  `mac-dump-system-database` (medium, read-only query) but rejected the write
  variants as too dangerous for a copy-paste directory.

## Known follow-ups

- **HN/Reddit lens is thin (4 entries).** Re-run that lens.
- **Search ranking:** with ~1470 tldr imports plus 115 gems, fuzzy queries
  sometimes rank a tldr page above a hand-curated gem. A future scoring tweak
  (boost `corpus/gems/` and hand-curated entries over `tldr-import` domain)
  would surface the good stuff first. Tracked as a search-quality task, not a
  corpus task.
- The parallel background research agents proved flaky under load (API drops,
  stream stalls). A foreground or smaller-fan-out sweep was more reliable.
