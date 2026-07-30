# Collective landing page v2 — Design Spec

Date: 2026-07-30
Status: Approved (brainstorming session)
Supersedes: `2026-07-29-collective-landing-page-design.md`
Relates to: the `collective` CLI at v0.4.0 · Linear XO-169

## Why a v2

The v1 spec was approved and then immediately invalidated by the category
audit (`2026-07-29-category-audit.md`): it named `tldr` and `cheat.sh` as the
competition, but `navi` (17.4k stars) is the real neighbour and already does
interactive search, shell prefill, placeholders, and third-party repos. The
positioning decision recorded on the audit (2026-07-29) adopted its Option 1 —
learning + safety schema + offline-first — while rejecting the tldr-pack cut.
This spec rewrites the pitch under that positioning and updates the demo for
the v0.4.0 typed drill. Everything structural from v1 that survived is
restated here so this document stands alone.

## What

A single static page whose one job is turning a curious visitor into someone
who runs `brew install`. Hosted on GitHub Pages from the project repo.

Not a docs site. The README stays the reference; the page is the pitch.

## Goal and non-goals

**Goal:** installs. Every section either builds the case for installing or
gets out of the way. The install command appears twice — hero and bottom — so
a visitor convinced at either point does not scroll to act.

**Non-goals:** analytics, custom domain, docs, blog, changelog, newsletter,
search, or any JavaScript beyond the cast player. Listed to be refused, not
deferred.

## 1. Technology

Hand-written HTML and CSS. No framework, no bundler, no build step.

The product's pitch is one offline binary with no runtime; a landing page that
needed `npm install` would contradict what it sells. Five files ship — three
authored, two vendored:

```
site/
  index.html
  style.css
  demo.cast              # asciinema recording
  vendor/
    asciinema-player.min.js
    asciinema-player.css
```

The player is vendored, not CDN-loaded: no third-party request, nothing that
can break or track visitors.

## 2. Hosting

GitHub Pages, published by a GitHub Actions workflow from `site/`.

**Not the `docs/` folder option** — `docs/` holds the superpowers specs and
plans; pointing Pages at it would publish every design document.

`.github/workflows/pages.yml` triggers on pushes to `main` touching `site/**`,
uses `actions/configure-pages`, `actions/upload-pages-artifact`,
`actions/deploy-pages`, with `permissions: pages: write, id-token: write`.
Pages must be switched to "GitHub Actions" as source in repo settings — a
one-time manual step; the API cannot enable Pages on a repo that never had it.

Repo metadata set as part of this work: `homepage` to the Pages URL, `topics`
to `cli`, `rust`, `developer-tools`, `tui`, `spaced-repetition`.

## 3. Page structure

Roughly one screen, then scroll. Order: what it is, proof it works, why it is
different, how to get it.

| # | Section | Content |
|---|---|---|
| 1 | Hero | Name, learning-first pitch, install command with copy button, GitHub link |
| 2 | Demo | The asciinema cast |
| 3 | Why | Three beats: lookup-vs-learning, safety schema, works-on-install |
| 4 | Packs | `pack add tldr` and `pack add <owner>/<repo>` |
| 5 | Install | brew, cargo, and the shell prefill wrapper |
| 6 | Footer | GitHub, MIT, registry |

### The hero leads with learning

v1 led with the familiar category (searchable offline directory) and made
drilling the second beat. Under the decided positioning that is inverted: the
differentiator is the headline, the category is the sub-line.

> **collective** — stop re-googling the same commands. Drill them until they
> stick.
>
> A curated, offline command corpus with spaced-repetition practice built in.

### Section 3, "Why" — category contrast, no competitor names

v1 named `tldr` and `cheat.sh`; the audit showed those are the wrong
neighbours, and naming the right one (`navi`) invites comparison-shopping on
lookup features it wins. The v2 copy draws the lookup-vs-learning axis without
naming anyone — nothing to go stale when the landscape shifts:

1. **Lookup tools answer the question; they don't stop you asking it again.**
   collective drills what you look up until recall beats re-googling.
2. **Every entry ships with a danger rating, an explanation, and an undo** —
   enforced by schema, not convention.
3. **Works the moment it installs.** 152 curated commands offline, the
   1459-entry tldr pack one command away. No network, no API key.

The offline/air-gapped audience is served by beat 3 as a feature line — no
dedicated air-gapped section, no anti-LLM framing. One page, one story.

## 4. The demo cast

Hand-authored `.cast` v2 (JSON header line + `[time, "o", "text"]` events),
not captured — deterministic, re-editable, paced for a viewer.

**Every command and every line of output must be real** — taken from the
actual v0.4.0 binary. The cast reconstructs a session that could happen; it
must never show output the tool would not produce. Verify each frame against
real command output before shipping.

Workflow order, with the typed drill as the longest segment — it is the
feature the hero just claimed:

1. `collective search "prevent sleep"` → grouped results
2. `collective` → TUI opens, type `git`, list filters live, `^Y` copies
3. `collective drill --domain git` → the typed drill:
   - type a plausible wrong answer → first differing token marked, expected
     answer shown
   - retype correct → accepted, SM-2 grade and next-review interval shown
4. `collective pack add tldr` → `installed tldr (1459 entries)`

About 35 seconds. Autoplay, loop, no audio, controls available;
`idle_time_limit` caps dead air. The exact drill exchange (prompt, wrong
answer, marker output, grade line) is taken verbatim from a real drill session
at implementation time, not invented in this spec.

## 5. Visual design

Terminal-native dark: monospace throughout, near-black background, accent
colours lifted from the TUI (red for high danger, dark gray help bar) so page
and product read as one thing. One accent, used sparingly.

Light mode supported via `prefers-color-scheme: light` — about fifteen lines
of CSS; dark remains the designed-for case.

Single centred column, max width ~720px, generous vertical rhythm.

Accessibility is not optional: 4.5:1 minimum body contrast, visible focus
styles, the copy button reachable and labelled for screen readers,
`prefers-reduced-motion` disables cast autoplay.

## 6. The copy button

The single piece of bespoke JavaScript: click the install command, it copies,
the button confirms for two seconds. `navigator.clipboard.writeText` with a
`document.execCommand('copy')` fallback; if both fail the text stays
selectable. ~15 lines, inline, no dependency.

## 7. Testing

Verified in a real browser before this is called done:

- Page loads with an empty console — no errors, no warnings.
- **Zero external network requests** — checked in the network panel; this is
  the claim the vendored player exists to make good on.
- Renders at 320px with no horizontal scroll, and at desktop width.
- The cast plays, loops, and its content matches real v0.4.0 output.
- Copy button works; page still usable with JavaScript disabled.
- Dark and light both legible; contrast checked, not eyeballed.
- Deployed Pages URL serves the page and the workflow is green.

## Rollout order

1. `site/index.html` and `style.css` with a placeholder demo block.
2. The hand-authored `demo.cast`, verified against real v0.4.0 output, and the
   vendored player wired in.
3. `.github/workflows/pages.yml`, then enable Pages in settings.
4. Repo `homepage` and `topics`.
5. Browser verification pass against section 7.

## Done criteria

- The Pages URL serves the page, and the deploy workflow is green.
- A visitor can copy a working install command without scrolling.
- The cast plays and shows only real output, including the typed drill.
- No external requests, no console errors, legible at 320px, both themes.
- Repo homepage and topics set.
