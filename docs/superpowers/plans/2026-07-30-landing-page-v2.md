# Landing Page v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the collective landing page — a single static GitHub Pages site whose one job is `brew install`.

**Architecture:** Hand-written HTML/CSS in `site/`, no build step, vendored asciinema player, one hand-authored demo cast built from real v0.4.0 output, deployed by a Pages workflow on pushes to `main` touching `site/**`.

**Tech Stack:** Plain HTML/CSS, ~15 lines of inline JS (copy button) + player init, asciinema-player v3.9.0 (vendored), GitHub Actions Pages deploy.

**Spec:** `docs/superpowers/specs/2026-07-30-landing-page-v2-design.md` · Linear XO-169

## Global Constraints

- **Zero external requests.** No CDN, no webfonts, no analytics, no remote images. Vendored assets only. Anchor `href`s to GitHub are fine; loaded resources are not.
- **No build step.** Files in `site/` are served exactly as written.
- **Real output only.** Every terminal frame in the cast and every entry excerpt on the page is verbatim output of the v0.4.0 binary. Never invent output.
- **Copy is fixed by the spec.** Hero: "Stop re-googling the same commands. Drill them until they stick." / "A curated, offline command corpus with spaced-repetition practice built in." Why beats: (1) "Lookup tools answer the question; they don't stop you asking it again. collective drills what you look up until recall beats re-googling." (2) "Every entry ships with a danger rating, an explanation, and an undo — enforced by schema, not convention." (3) "Works the moment it installs. 152 curated commands offline, the 1,459-entry tldr pack one command away. No network, no API key." Do not name competitors anywhere.
- Product name is lowercase `collective` everywhere, including the page `<h1>` and `<title>`.
- Install command: `brew install xooxoxxo/tap/collective` (appears in hero AND install section).
- Accessibility: ≥4.5:1 body contrast, visible focus styles, labelled copy button, `prefers-reduced-motion` disables cast autoplay, page usable with JS disabled, light mode via `prefers-color-scheme`.
- **Environment:** `gh` must be on the `oytuneyucel` account; push with `GH_TOKEN=$(gh auth token --user oytuneyucel) git push origin main`. `rm` is blocked by a shell hook — use `trash` or `mv`. Run `cargo clippy --all-targets -- -D warnings` before every commit (repo rule; it is fast and must exit 0).
- Static HTML has no unit-test framework here; each task's test cycle is the concrete runnable checks given in its steps (grep, local serve, browser).

---

### Task 1: Page structure — `site/index.html` + `site/style.css`

**Files:**
- Create: `site/index.html`
- Create: `site/style.css`

**Interfaces:**
- Produces: `<div id="player">` placeholder (Task 2 replaces its fallback content with the cast player), `<link rel="stylesheet" href="vendor/asciinema-player.css">` and `<script src="vendor/asciinema-player.min.js">` references that 404 until Task 2 vendors them (acceptable within Task 1 — its checks skip vendor files), `#install-cmd` / `.copy` used by the inline copy-button JS below.

- [ ] **Step 1: Capture the real entry excerpt for the Why section**

The safety beat shows one real entry. Capture it:

```bash
cargo run --quiet -- show mac-prevent-sleep-while-charging
```

Copy the output verbatim into a scratch note. If that id prints nothing, pick any gem with a danger rating and an undo via `cargo run --quiet -- search sudo` and `show` it. The excerpt used on the page must be this captured text, character for character (trailing whitespace may be trimmed).

- [ ] **Step 2: Write `site/index.html`**

Replace the `<pre class="entry">` content below with the Step 1 capture (HTML-escape `<` and `>` as `&lt;`/`&gt;`):

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>collective — drill the commands you keep re-googling</title>
<meta name="description" content="A curated, offline command corpus with spaced-repetition practice built in. Danger ratings and undos on every entry, SM-2 typing drills, works without a network.">
<link rel="stylesheet" href="style.css">
<link rel="stylesheet" href="vendor/asciinema-player.css">
</head>
<body>

<header class="hero">
  <p class="prompt" aria-hidden="true">$</p>
  <h1>collective</h1>
  <p class="tagline">Stop re-googling the same commands.<br>Drill them until they stick.</p>
  <p class="sub">A curated, offline command corpus with spaced-repetition practice built in.</p>
  <div class="install-row">
    <code id="install-cmd">brew install xooxoxxo/tap/collective</code>
    <button class="copy" type="button" aria-label="Copy install command">copy</button>
  </div>
  <p class="hero-links"><a href="https://github.com/xooxoxxo/collective">source on GitHub</a></p>
</header>

<main>
  <section class="demo" aria-label="Terminal demo">
    <div id="player">
      <pre class="demo-fallback">$ collective drill --domain git
<!-- JS-disabled fallback: replace this comment with the first two lines of the
     Task 2 Segment C drill capture, verbatim — never invented text. Until Task 2
     runs, leave the comment in place; Task 2 Step 6 fills it. --></pre>
    </div>
  </section>

  <section>
    <h2>Why</h2>
    <div class="beat">
      <p><strong>Lookup tools answer the question; they don't stop you asking it again.</strong>
      collective drills what you look up until recall beats re-googling.</p>
    </div>
    <div class="beat">
      <p><strong>Every entry ships with a danger rating, an explanation, and an undo</strong> —
      enforced by schema, not convention.</p>
      <pre class="entry"><!-- Step 1 capture goes here, verbatim --></pre>
    </div>
    <div class="beat">
      <p><strong>Works the moment it installs.</strong> 152 curated commands offline, the
      1,459-entry tldr pack one command away. No network, no API key.</p>
    </div>
  </section>

  <section>
    <h2>Packs</h2>
    <p>The core stays hand-picked. Bulk is opt-in:</p>
    <pre class="term">$ collective pack add tldr
installed tldr (1459 entries)</pre>
    <p>Anyone can publish a pack — a repo with entry YAML is enough:</p>
    <pre class="term">$ collective pack add owner/repo</pre>
  </section>

  <section>
    <h2>Install</h2>
    <pre class="term">$ brew install xooxoxxo/tap/collective</pre>
    <p>Or with cargo:</p>
    <pre class="term">$ cargo install --git https://github.com/xooxoxxo/collective</pre>
    <p>The shell wrapper puts picked commands on your prompt instead of your clipboard:</p>
    <pre class="term">$ collective --print-shell zsh &gt;&gt; ~/.zshrc</pre>
  </section>
</main>

<footer>
  <p>
    <a href="https://github.com/xooxoxxo/collective">GitHub</a> ·
    <a href="https://github.com/xooxoxxo/collective/blob/main/LICENSE">MIT</a> ·
    <a href="https://github.com/xooxoxxo/collective-registry">pack registry</a>
  </p>
</footer>

<script src="vendor/asciinema-player.min.js"></script>
<script>
(function () {
  var el = document.getElementById('player');
  if (window.AsciinemaPlayer && el) {
    el.textContent = '';
    AsciinemaPlayer.create('demo.cast', el, {
      loop: true,
      autoPlay: !window.matchMedia('(prefers-reduced-motion: reduce)').matches,
      controls: true,
      idleTimeLimit: 2,
      fit: 'width'
    });
  }
  var btn = document.querySelector('.copy');
  var cmd = document.getElementById('install-cmd');
  if (btn && cmd) {
    btn.addEventListener('click', function () {
      function done() {
        btn.textContent = 'copied';
        btn.classList.add('ok');
        setTimeout(function () {
          btn.textContent = 'copy';
          btn.classList.remove('ok');
        }, 2000);
      }
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(cmd.textContent).then(done);
      } else {
        var r = document.createRange();
        r.selectNodeContents(cmd);
        var sel = getSelection();
        sel.removeAllRanges();
        sel.addRange(r);
        if (document.execCommand('copy')) done();
        sel.removeAllRanges();
      }
    });
  }
})();
</script>
</body>
</html>
```

Cargo install line note: the crate is not on crates.io, so `--git` is the honest cargo path. If `cargo install collective` becomes real later, update the page then — do not pre-claim it.

- [ ] **Step 3: Write `site/style.css`**

```css
:root {
  --bg: #0c0d10;
  --fg: #d6d8de;
  --dim: #9297a8;
  --line: #26282f;
  --accent: #ff6059;   /* TUI high-danger red */
  --term-bg: #101218;
}
@media (prefers-color-scheme: light) {
  :root {
    --bg: #f7f6f3;
    --fg: #1d2024;
    --dim: #575c66;
    --line: #d8d6cf;
    --accent: #c0362f;
    --term-bg: #edece7;
  }
}

* { box-sizing: border-box; }

body {
  margin: 0;
  background: var(--bg);
  color: var(--fg);
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, "DejaVu Sans Mono", monospace;
  font-size: 1rem;
  line-height: 1.65;
  -webkit-font-smoothing: antialiased;
}

.hero, main, footer {
  max-width: 45rem;
  margin: 0 auto;
  padding: 0 1.25rem;
}

/* ---- hero ---- */
.hero { padding-top: 5rem; }
.prompt { color: var(--dim); margin: 0 0 0.25rem; }
h1 {
  font-size: clamp(2.4rem, 8vw, 3.4rem);
  font-weight: 700;
  letter-spacing: -0.02em;
  margin: 0 0 1.5rem;
}
.tagline {
  font-size: clamp(1.15rem, 4vw, 1.45rem);
  font-weight: 700;
  letter-spacing: -0.01em;
  line-height: 1.4;
  margin: 0 0 0.75rem;
  text-wrap: balance;
}
.sub { color: var(--dim); margin: 0 0 2.25rem; max-width: 38rem; }

.install-row {
  display: flex;
  align-items: stretch;
  gap: 0.5rem;
  flex-wrap: wrap;
  margin-bottom: 1rem;
}
.install-row code {
  background: var(--term-bg);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 0.65rem 0.9rem;
  overflow-x: auto;
  white-space: nowrap;
  max-width: 100%;
}
.copy {
  font: inherit;
  color: var(--fg);
  background: none;
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 0.65rem 0.9rem;
  cursor: pointer;
  transition: border-color 120ms ease-out, color 120ms ease-out;
}
.copy:hover { border-color: var(--dim); }
.copy.ok { color: var(--accent); border-color: var(--accent); }
.hero-links { margin: 0 0 3.5rem; }

/* ---- sections ---- */
main section { margin-bottom: 3.5rem; }
h2 {
  font-size: 1rem;
  font-weight: 700;
  text-transform: lowercase;
  color: var(--dim);
  margin: 0 0 1.25rem;
}
h2::before { content: "── "; }
.beat { margin-bottom: 1.5rem; }
.beat p { margin: 0 0 0.75rem; max-width: 65ch; }
.beat strong { color: var(--fg); }
main p { max-width: 65ch; }

/* ---- terminal blocks ---- */
pre {
  background: var(--term-bg);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 0.9rem 1.1rem;
  overflow-x: auto;
  line-height: 1.55;
  margin: 0.75rem 0 1.25rem;
}
pre.entry { color: var(--dim); }
pre.entry .danger, .danger { color: var(--accent); }

/* ---- demo ---- */
.demo #player {
  border: 1px solid var(--line);
  border-radius: 8px;
  overflow: hidden;
}
.demo-fallback { border: 0; border-radius: 0; margin: 0; }

/* ---- footer ---- */
footer {
  border-top: 1px solid var(--line);
  padding-top: 1.5rem;
  padding-bottom: 3rem;
  color: var(--dim);
}

/* ---- links & focus ---- */
a { color: var(--fg); text-underline-offset: 3px; }
a:hover { color: var(--accent); }
:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
  border-radius: 2px;
}

@media (prefers-reduced-motion: reduce) {
  .copy { transition: none; }
}
```

After pasting the Step 1 excerpt into `pre.entry`, wrap only its danger value in `<span class="danger">…</span>` (e.g. `<span class="danger">high</span>`) — that is the page's single accent use besides link hover and copy-confirm.

- [ ] **Step 4: Serve locally and check**

```bash
python3 -m http.server 8123 -d site &
curl -s http://localhost:8123/ | head -5
```

Expected: the doctype and `<title>collective — …`. Open `http://localhost:8123/` in a browser: hero, why, packs, install, footer all render; demo section shows the fallback block (player files don't exist yet — the two vendor 404s in the console are expected and disappear in Task 2).

- [ ] **Step 5: Mechanical zero-external check**

```bash
grep -nE '(src|href)="https?://' site/index.html
```

Expected: only `<a href="https://github.com/…">` anchors. Any `<link>`, `<script>`, or `<img>` with an external URL is a failure.

- [ ] **Step 6: Commit**

```bash
cargo clippy --all-targets -- -D warnings
git add site/index.html site/style.css
git commit -m "feat: landing page structure and styles"
```

---

### Task 2: Demo cast + vendored player

**Files:**
- Create: `site/demo.cast`
- Create: `site/vendor/asciinema-player.min.js`
- Create: `site/vendor/asciinema-player.css`
- Modify: `site/index.html` (fill the JS-disabled fallback with real drill lines; commit it in Step 7 with `git add site/`)

**Interfaces:**
- Consumes: `<div id="player">` and the `AsciinemaPlayer.create('demo.cast', …)` init from Task 1 (already in `index.html` — this task only adds the files it references).

- [ ] **Step 1: Build the release binary**

```bash
cargo build --release
alias c=./target/release/collective
```

- [ ] **Step 2: Capture real output for each cast segment**

Segment A — search:

```bash
./target/release/collective search "prevent sleep"
```

Save the full output. (Verified 2026-07-30: prints six curated rows, then `── tldr imports ──`, then tldr rows.)

Segment B — TUI: record a real session, since TUI frames are ANSI and must not be hand-invented:

```bash
brew list asciinema >/dev/null 2>&1 || brew install asciinema
asciinema rec /tmp/tui.cast --command "./target/release/collective"
# in the recording: type "git", wait a beat, press ^Y to copy, quit
```

Segment C — typed drill: record the same way:

```bash
asciinema rec /tmp/drill.cast --command "./target/release/collective drill --domain git"
# answer the first card WRONG with a plausible near-miss, note the marker output,
# then answer the next card (or same on repeat) correctly, accept the grade, quit
```

If it prints `nothing due. come back later.`, park the state and retry (all cards are due on fresh state): `mv ~/.collective/drill.json /tmp/drill.json.bak`, re-record, then `mv /tmp/drill.json.bak ~/.collective/drill.json`.

The real drill format (from `src/drill.rs`) is: `── {title}` · `your answer (or Enter to reveal): ` · `  you typed: {typed}  {mark}` · a caret line ending `^ first difference` · `graded: {label}   [Enter accepts · 1-4 overrides]: ` · `session done.` The cast must show these exact strings as recorded, not the sketch from the spec.

Segment D — pack add: if the tldr pack is already installed locally, remove and re-add to capture the real line, or capture from a temp `COLLECTIVE_HOME` if the tool supports it; otherwise run `./target/release/collective pack add tldr` and save the output. Expected shape: `installed tldr (1459 entries)` — but use whatever it actually prints.

- [ ] **Step 3: Author `site/demo.cast`**

Hand-author cast v2 from the captured material — header line then events:

```
{"version": 2, "width": 100, "height": 26, "idle_time_limit": 2, "title": "collective demo"}
[0.0, "o", "$ collective search \"prevent sleep\"\r\n"]
[0.8, "o", "<Segment A output, escaped, \r\n line endings>"]
...
```

Rules:
- Sequence and pacing per spec: A (~6s) → B (~8s) → C the longest (~15s) → D (~4s), ~35s total.
- Splice Segment B/C event lines from `/tmp/tui.cast` and `/tmp/drill.cast` (they are already valid cast v2 events — copy them, rebase the timestamps, drop dead air).
- Type-out effect for command lines: emit them in 2–4 chunks with ~80ms gaps, not one event per keystroke.
- JSON-escape `"` and `\` in output text; terminal newlines are `\r\n`.
- Trim Segment A to the six curated rows + the `── tldr imports ──` divider + two tldr rows so it fits 26 rows — trimming rows is fine, altering a row is not.

- [ ] **Step 4: Vendor the player**

```bash
mkdir -p site/vendor
curl -fLo site/vendor/asciinema-player.min.js https://github.com/asciinema/asciinema-player/releases/download/v3.9.0/asciinema-player.min.js
curl -fLo site/vendor/asciinema-player.css https://github.com/asciinema/asciinema-player/releases/download/v3.9.0/asciinema-player.css
```

If v3.9.0 404s, check `gh release list --repo asciinema/asciinema-player --limit 3` and pin the newest 3.x instead — record the chosen version in the commit message.

- [ ] **Step 5: Verify playback**

```bash
python3 -m http.server 8123 -d site &
```

Open `http://localhost:8123/`: the cast plays, loops, shows search → TUI → typed drill (wrong answer, `^ first difference` marker, correct retype, grade line) → pack add. Console shows no errors. With system reduced-motion enabled, the cast does not autoplay but plays via its controls.

- [ ] **Step 6: Fill the JS-disabled fallback and verify cast content against reality**

In `site/index.html`, replace the comment inside `pre.demo-fallback` with the first two lines of the Segment C drill capture (the `── {title}` line and the `your answer (or Enter to reveal): ` prompt), verbatim.

Then, for each non-TUI line in `demo.cast`, diff it against the Step 2 captures. Any line that does not appear in a capture is a spec violation — fix the cast, not the capture.

- [ ] **Step 7: Commit**

```bash
cargo clippy --all-targets -- -D warnings
git add site/demo.cast site/vendor/
git commit -m "feat: demo cast from real v0.4.0 output + vendored asciinema player v3.9.0"
```

---

### Task 3: Pages deploy workflow + repo metadata

**Files:**
- Create: `.github/workflows/pages.yml`

**Interfaces:**
- Consumes: the completed `site/` directory from Tasks 1–2.
- Produces: the live Pages URL `https://xooxoxxo.github.io/collective/` used by Task 4.

- [ ] **Step 1: Write `.github/workflows/pages.yml`**

```yaml
name: pages

on:
  push:
    branches: [main]
    paths: ["site/**", ".github/workflows/pages.yml"]

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: true

jobs:
  deploy:
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - uses: actions/checkout@v7
      - uses: actions/configure-pages@v5
      - uses: actions/upload-pages-artifact@v4
        with:
          path: site/
      - id: deployment
        uses: actions/deploy-pages@v4
```

(checkout is pinned v7 to match the repo's other workflows; check `.github/workflows/` and match whatever major they use.)

- [ ] **Step 2: Enable Pages with the workflow as source**

```bash
gh auth switch --user oytuneyucel
gh api -X POST repos/xooxoxxo/collective/pages -f build_type=workflow
```

If the API rejects it (422 already exists → instead `gh api -X PUT repos/xooxoxxo/collective/pages -f build_type=workflow`; other errors → do it once by hand in repo Settings → Pages → Source: GitHub Actions), say so in the task report.

- [ ] **Step 3: Set repo homepage and topics**

```bash
gh repo edit xooxoxxo/collective \
  --homepage "https://xooxoxxo.github.io/collective/" \
  --add-topic cli --add-topic rust --add-topic developer-tools \
  --add-topic tui --add-topic spaced-repetition
```

- [ ] **Step 4: Commit, push, watch the deploy**

```bash
cargo clippy --all-targets -- -D warnings
git add .github/workflows/pages.yml
git commit -m "feat: GitHub Pages deploy workflow for site/"
GH_TOKEN=$(gh auth token --user oytuneyucel) git push origin main
gh run list --repo xooxoxxo/collective --limit 2
```

Wait for the `pages` run to complete (`gh run watch <id> --repo xooxoxxo/collective --exit-status` — check the exit code itself, do not pipe it through `tail`). Expected: success, and `curl -sI https://xooxoxxo.github.io/collective/ | head -1` returns `HTTP/2 200`.

---

### Task 4: Browser verification pass (spec §7)

**Files:**
- Modify: `site/index.html`, `site/style.css` (fixes only, if checks fail)

**Interfaces:**
- Consumes: the deployed page from Task 3 and the local serve from Tasks 1–2.

- [ ] **Step 1: Run the mechanical design detector**

```bash
node ~/.agents/skills/impeccable/scripts/detect.mjs --json site/index.html site/style.css
```

Fix any finding it reports (it checks the mechanical craft floor: contrast, ghost cards, banned patterns).

- [ ] **Step 2: Browser checks on the deployed URL**

Using the Playwright browser tools (or a real browser with devtools), against `https://xooxoxxo.github.io/collective/`:

- Console: zero errors, zero warnings.
- Network panel: **every request is same-origin** — the vendored player is the point; verify it, don't assume it.
- 320px viewport: no horizontal scroll, install command scrolls inside its own box.
- Desktop viewport: single centred column, demo and code blocks contained.
- Cast: plays, loops, content matches the Task 2 captures.
- Copy button: click → `copied` for 2s → reverts; pasted text is exactly `brew install xooxoxxo/tap/collective`.
- JS disabled: page readable, fallback block shown in the demo slot, everything except player/copy works.
- Light mode: legible; spot-check body contrast ≥4.5:1 (`--fg` on `--bg`) with a contrast checker, both themes.
- Keyboard: tab reaches the copy button and all links with a visible focus ring.

- [ ] **Step 3: Fix and redeploy if needed**

Any failure: fix in `site/`, re-run the failed check locally, then:

```bash
cargo clippy --all-targets -- -D warnings
git add site/
git commit -m "fix: landing page verification findings"
GH_TOKEN=$(gh auth token --user oytuneyucel) git push origin main
```

Re-verify on the deployed URL after the pages run goes green.

- [ ] **Step 4: Close out**

Update Linear XO-169 with the live URL and mark it Done. Report the done-criteria checklist from the spec with each item's actual result.
