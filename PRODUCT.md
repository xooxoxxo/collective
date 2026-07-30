# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Primary: developers who deliberately practice shell-command recall — people
who choose memorization as a discipline rather than re-googling or asking an
LLM every time. Secondary (confirmed, not courted with dedicated pitch):
DevOps/SRE teams in air-gapped or restricted networks, served by the
offline-first design as a feature line.

## Product Purpose

collective is a Rust CLI: a searchable, offline corpus of shell commands with
enforced safety metadata and SM-2 spaced-repetition typing drills. It exists
so the commands you look up become commands you know.

Success is real adoption: installs, stars, and above all drill usage. The
category audit's test is on record — if >50% of active users drill regularly,
the learning positioning is validated; if <10%, the tool is a lookup
alternative in people's heads, which is a losing position.

## Positioning

Decided 2026-07-29 (category audit, Option 1 amended): **learning + enforced
safety schema + offline-first**. The claim a neighbour cannot truthfully copy:
no maintained tool combines a curated offline corpus, a build-gated
danger/undo schema on every entry, and spaced-repetition drilling.

navi (17.4k stars) is the nearest neighbour and wins on lookup — interactive
fuzzy search, live variable sourcing, repo ecosystem. collective does not
compete on lookup features and public copy does not name competitors; it draws
the lookup-vs-learning category contrast instead. The audit's tldr-pack cut
was rejected: core stays 152 curated entries, packs stay optional bulk.

## Operating Context

Installed via `brew install xooxoxxo/tap/collective` or cargo; a shell wrapper
(`collective --print-shell zsh`) enables prefill. Daily use is terminal-only:
ratatui TUI search with `^Y` copy, `collective drill` typed practice,
`collective collect` into a personal overlay (`~/.collective/corpus/`) that
always wins over shipped entries, `collective pack add` for opt-in packs from
the JSON registry (xooxoxxo/collective-registry).

Corpus: 152 curated entries ship in the binary (10 root + 142 gems); the tldr
pack adds 1,459 for 1,611 total.

## Capabilities and Constraints

- v0.4.0 released 2026-07-30: typed drill (normalised matching, first-wrong-
  token marker, SM-2 grade derived from the result), explanation-indexed
  search, pack module split.
- Every entry carries a danger rating (4-level), an explanation, and an
  optional undo — enforced at build time, not by convention.
- Platform filtering: macOS/Linux/BSD per entry.
- Placeholders are static prompts only. Live variable sourcing (navi's moat)
  is deliberately out of scope.
- Fully offline: no network, no API key, no telemetry, no account.
- Landing page constraint (from its spec): hand-written HTML/CSS, no build
  step, vendored assets, zero external requests — the page must not
  contradict the offline pitch.
- Known ceilings, tracked in Linear: natural-language search ranking (XO-170),
  `-n 5`/`-n5` drill equivalence (XO-171).

## Brand Commitments

- Name is lowercase **collective**; binary `collective`.
- Voice: plain and technical — terse, factual, real numbers, no hype words.
  Reference copy: "stop re-googling the same commands. Drill them until they
  stick."
- Terminal-native visual identity, made binding by the landing page spec:
  monospace, near-black background, accent colours lifted from the TUI (red =
  high danger, dark gray help bar), one accent used sparingly.
- MIT licensed, public at github.com/xooxoxxo/collective.

## Evidence on Hand

Real numbers only: 1,611 entries (152 curated + 1,459 tldr pack), 129 tests,
~4.56 MB binary, four release targets, v0.4.0 on the Homebrew tap. Demo
material must be real binary output — the landing spec forbids invented
terminal frames.

Absent, and must not be fabricated: testimonials, named users, case studies,
star counts, benchmarks, usage statistics.

## Product Principles

1. **Works the moment it installs.** No setup, no network, no empty state.
2. **Learning over lookup.** Features are judged by whether they build recall,
   not by lookup convenience.
3. **Safety is schema, not convention.** Danger, explanation, undo — enforced
   at build, on every entry.
4. **Show only real output.** Marketing surfaces reconstruct sessions the
   binary actually produces.
5. **Curated core, optional bulk.** The 152 stay hand-picked; scale lives in
   opt-in packs.

## Accessibility & Inclusion

Committed by the landing page spec: 4.5:1 minimum body contrast, visible focus
styles, screen-reader-labelled controls, `prefers-reduced-motion` honoured
(disables cast autoplay), page usable with JavaScript disabled, light mode via
`prefers-color-scheme`. No further product-specific standard established.
