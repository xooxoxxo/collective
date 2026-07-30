# Collective landing page — Design Spec

Date: 2026-07-29
Status: **Superseded by `2026-07-30-landing-page-v2-design.md`** — the
positioning this spec's pitch rests on was invalidated by the category audit
(navi, not tldr/cheat.sh, is the real competitor), and the demo predates the
v0.4.0 typed drill.
Relates to: the `collective` CLI, released at v0.3.1

## What

A single static page whose one job is turning a curious visitor into someone who
runs `brew install`. Hosted on GitHub Pages from the project repo.

Not a docs site. The README stays the reference; the page is the pitch.

## Goal and non-goals

**Goal:** installs. Every section either builds the case for installing or gets
out of the way. The install command appears twice — once in the hero, once at
the bottom — because a visitor convinced at either point should not have to
scroll to act.

**Non-goals:** analytics, custom domain, docs, blog, changelog, newsletter,
search, or any JavaScript beyond the cast player. These are listed to be
refused, not deferred.

## 1. Technology

Hand-written HTML and CSS. No framework, no bundler, no build step.

The product's entire pitch is one offline binary with no runtime. A landing page
that needed `npm install` to produce would contradict the thing it is selling.
Five files ship — three authored, two vendored:

```
site/
  index.html
  style.css
  demo.cast              # asciinema recording
  vendor/
    asciinema-player.min.js
    asciinema-player.css
```

The player is vendored rather than pulled from a CDN. Same reasoning as the
binary being offline-first: no third-party request, no external dependency that
can break or track visitors.

## 2. Hosting

GitHub Pages, published by a GitHub Actions workflow from `site/`.

**Not the `docs/` folder option.** `docs/` already holds this project's
superpowers specs and plans. Pointing Pages at it would publish every design
document as a web page. `site/` keeps the served surface deliberate.

The workflow (`.github/workflows/pages.yml`) triggers on pushes to `main` that
touch `site/**`, uses `actions/configure-pages`, `actions/upload-pages-artifact`,
and `actions/deploy-pages`, and needs `permissions: pages: write, id-token: write`.
Pages must also be switched to "GitHub Actions" as its source in repo settings —
a one-time manual step, since the API cannot enable Pages on a repo that has
never had it.

Two repo metadata fields are currently empty and get set as part of this work:
`homepage` to the Pages URL, and `topics` to `cli`, `rust`, `developer-tools`,
`tui`, `spaced-repetition`.

## 3. Page structure

Roughly one screen, then scroll. Order is deliberate: what it is, proof it
works, why it is different, how to get it.

| # | Section | Content |
|---|---|---|
| 1 | Hero | Name, one-line pitch, primary install command with a copy button, GitHub link |
| 2 | Demo | The asciinema cast |
| 3 | Why | Three reasons, including the comparison to existing tools |
| 4 | Packs | `pack add tldr` and `pack add <owner>/<repo>` |
| 5 | Install | brew, cargo, and the shell prefill wrapper |
| 6 | Footer | GitHub, MIT, registry |

### The pitch, and how the two angles reconcile

Two framings were both chosen, and they are sequenced rather than merged.

**The hero states what it is**, in the familiar terms of a category people
already understand: a searchable offline directory of developer commands. This
is the instantly graspable framing — a visitor who knows `tldr` or `cheat.sh`
places it in one line.

**The next beat states what is different**: you drill the commands with spaced
repetition until you stop re-googling them. This is the genuinely novel claim
and no comparable tool does it, but it lands better as the second beat than the
first — leading with it would ask a visitor to accept an unfamiliar premise
before they know what the thing is.

Hero copy:

> **collective** — a searchable, offline directory of developer commands you'd
> otherwise re-google. Then drill them until you don't have to.

### Section 3, "Why", names the alternatives

Three points, each one sentence, the middle one comparative:

1. **Curated, not scraped.** ~150 hand-picked commands ship in the binary, each
   with an explanation, an undo, and a danger rating.
2. **Unlike `tldr` or `cheat.sh`, it is not just lookup.** Entries are ranked
   curated-first, work with no network, and can be drilled as flashcards.
3. **You keep what you capture.** `collective collect` saves your own commands
   into a personal overlay that always wins over shipped entries.

Naming competitors is a deliberate choice: it is clarifying for the audience
that already knows the space, and this audience does. The comparison stays
factual and avoids disparagement — it states a difference in kind, not a
judgment of quality.

## 4. The demo cast

Hand-authored, not captured. `asciinema` records through a TTY, and a
hand-written cast is deterministic, re-editable without re-recording when the UI
changes, and lets the pacing be set for a viewer rather than a typist.

**Every command and every line of output in it must be real** — taken from the
actual v0.3.1 binary, not invented. The cast reconstructs a session that could
happen; it must never show output the tool would not produce. Verify each frame
against real command output before shipping.

The `.cast` v2 format is a JSON header line followed by `[time, "o", "text"]`
event lines, which is why it can be authored directly.

Sequence, about 30 seconds:

1. `collective search "prevent sleep"` → grouped results
2. `collective` → TUI opens, type `git`, list filters live
3. `^Y` → copies, help bar visible
4. `collective drill --domain git` → one flashcard, reveal, grade
5. `collective pack add tldr` → `installed tldr (1459 entries)`

Playback: autoplay, loop, no audio, with controls available. `idle_time_limit`
caps dead air so typing pauses do not stall the viewer.

## 5. Visual design

Terminal-native dark: monospace throughout, near-black background, accent
colours lifted from the TUI so the page and the product read as one thing. The
TUI uses red for high-danger entries and dark gray for the help bar; the page
borrows that restraint — one accent, used sparingly.

**Light mode is supported anyway**, via `prefers-color-scheme: light`. A
pure-dark page is a genuinely poor experience for some readers, and honouring
the system preference costs about fifteen lines of CSS. The dark presentation
remains the default and the designed-for case.

Layout is a single centred column, max width around 720px, generous vertical
rhythm. No grid framework; the page has one column at every size.

Accessibility is not optional here: real contrast ratios (4.5:1 minimum for body
text), focus styles that are visible, the copy button reachable and labelled for
screen readers, and `prefers-reduced-motion` respected by disabling cast
autoplay.

## 6. The copy button

The single piece of bespoke JavaScript: click the install command, it copies,
the button confirms for two seconds. `navigator.clipboard.writeText` with a
`document.execCommand('copy')` fallback for older browsers, and if both fail the
text remains selectable so nothing is lost. Roughly fifteen lines, inline, no
dependency.

## 7. Testing

Verified in a real browser before this is called done:

- Page loads with an empty console — no errors, no warnings.
- **Zero external network requests.** Checked in the network panel; this is the
  claim the vendored player exists to make good on, so it gets verified rather
  than assumed.
- Renders correctly at 320px with no horizontal scroll, and at desktop width.
- The cast plays, loops, and its content matches real command output.
- Copy button works, and the page is still usable with JavaScript disabled.
- Dark and light both legible; contrast checked, not eyeballed.
- Deployed Pages URL serves the page and the workflow is green.

## Rollout order

1. `site/index.html` and `style.css` with a placeholder demo block.
2. The hand-authored `demo.cast`, verified against real output, and the vendored
   player wired in.
3. `.github/workflows/pages.yml`, then enable Pages in settings.
4. Repo `homepage` and `topics`.
5. Browser verification pass against section 7.

## Done criteria

- The Pages URL serves the page, and the deploy workflow is green.
- A visitor can copy a working install command without scrolling.
- The cast plays and shows only real output.
- No external requests, no console errors, legible at 320px, both themes.
- Repo homepage and topics set.
