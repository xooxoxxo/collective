# collective — category audit and positioning

## Verdict in three sentences

Collective is functionally redundant with navi on core use cases—interactive fuzzy search, command prefill, optional packs—and navi outscales it decisively on variable sourcing (live shell command population with transformations), ecosystem (17.4k stars vs 0 GitHub stars for collective), and repository contribution workflow (PR-based featured_repos.txt vs JSON pack registry). Collective's genuine differentiator is not spaced repetition (prior art: srsh, 2021; multiple general-purpose SR CLI tools exist) but the *combination* of offline curated corpus + strict safety schema (danger ratings, undo commands, platform filtering) + learning-focused UX (SM-2 drills). However, this positioning serves a narrow market: developers who prioritize *memorizing* commands deeply over *generating* them on demand—a segment shrinking as LLM command assistants mature. Collective's viability depends on finding an audience that values learning and memorization as a deliberate discipline, not as a substitute for generation.

## The landscape

| Capability | collective | navi | cheat (plain text) | tealdeer (tldr client) | Atuin (personal history) | GitHub Copilot CLI | ShellGPT | Warp |
|---|---|---|---|---|---|---|---|---|
| **Offline corpus** | ✓ (152 curated) | ✓ (community repos) | ✓ (user .md) | ✓ (cached) | ✓ (local DB) | ✗ (API only) | ✗ (API only) | ✗ (cloud-based) |
| **Interactive TUI** | ✓ (ratatui fzf-style) | ✓ (fzf/skim) | ✗ | ✗ | Partial (search UI) | ✗ | ✗ | ✓ (terminal app) |
| **Fuzzy search** | ✓ (weighted) | ✓ (fzf pass-through) | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| **Live variable sourcing** | ✗ | ✓ (shell commands, transforms) | ✗ | ✗ | ✗ | ✗ (NL → command) | ✓ (NL → command) | ✗ |
| **Static placeholders** | ✓ | ✓ | Limited (inline) | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Danger/safety schema** | ✓ (4-level ratings) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Undo commands** | ✓ (optional) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Platform filtering** | ✓ (macOS/Linux/BSD) | ✗ | ✗ | ✗ | ✗ | ✓ (OS-aware generation) | ✓ (OS-aware generation) | ✓ |
| **Spaced-repetition learning** | ✓ (SM-2 drills) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Custom pack system** | ✓ (JSON registry) | ✓ (git clone + PR) | ✓ (user directories) | ✓ (cache updates) | ✗ | ✗ | ✗ | ✓ (shared workflows) |
| **Schema enforcement** | ✓ (strict YAML, build-time validation) | ✗ (free-form .cheat) | ✗ (free-form text) | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Shell integration** | ✓ (zsh/bash wrapper) | ✓ (shell widget) | Limited (--edit/-e) | Limited | ✓ | ✓ | ✓ (Ctrl+l hotkey) | Built-in |
| **On-demand AI generation** | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ | ✓ | Roadmap |
| **Ecosystem size** | 0 stars (not on GitHub) | 17,372 stars | 13,402 stars | 6,387 stars | 30,889 stars | 11,033 stars | 12,224 stars | 63,757 stars |

## Where collective genuinely stands alone

**Safety-first design with schema enforcement:** Collective requires danger levels (low/medium/high), explanations, and optional undo commands at entry definition time, enforced at build. No competitor in the cheatsheet/command-reference space does this. This is load-bearing for any positioning around "commands as a learnable discipline."

**Spaced-repetition drilling tuned to shell commands:** Collective implements SM-2 correctly (src/sm2.rs, 95 lines, verified against algorithm spec), with drill UI, persistence in ~/.collective/drill.json, and scheduling state. While prior art exists (srsh, 2021; general-purpose tools like anki-cli), no active shell-command-specific implementation matches collective's execution. This is unique only if maintained; it is not unique conceptually.

**Curated offline-first 152-entry corpus:** Hand-selected commands across macOS internals, dotfiles, shell wizardry, and modern CLI tools. Navi's featured repos aggregate community-driven content; collective's core is hand-curated for quality. The 152 built-in entries ship with the binary with zero setup, no network, no git clone required. This is genuinely distinctive if the curation is the value, not redundant if the value is "access to command examples."

## Where collective is redundant

**Interactive TUI and fuzzy search:** Navi's fzf/skim integration is more mature and battle-tested. Collective's ratatui-based TUI is smaller and slower than fzf (written in C). This is a redo of work navi solved first and better.

**Variable sourcing and placeholders:** Navi's variable system is vastly superior. It supports live shell command population (e.g., `$ dir: find ~/ -type d` to populate a directory chooser), transformations (--column, --map, --delimiter, --preview, --expand, --multi), and fzf pass-through options (--query, --header). Collective's `<placeholder>` system is static prompts only. For advanced users, navi's capability gap is unbridgeable.

**Repository ecosystem and contribution workflow:** Navi's featured_repos.txt (12 active repos, PR-based contributions) is more discoverable and maintainable than collective's JSON registry. GitHub's PR workflow is the lingua franca of open-source; navi's repo system plays into existing OS culture. Collective's registry requires direct binary updates, not community PRs.

**tldr pack as a static snapshot:** Collective's 1,459 tldr entries are compiled YAML built at release time from committed files—a stale snapshot. Tealdeer downloads and syncs live with tldr-pages daily. If the value is tldr access, use tealdeer. If the value is demonstrating collective's tooling with bundled content, the snapshot suffices, but it is not an advantage.

**Platform filtering:** Implemented but niche. Navi and most competitors let users search across all platforms and run what works; collective's platform metadata is a small DX win, not a strategic advantage.

## The strategic question

Does a curated, memorisable command corpus still make sense when an LLM can generate any command on demand?

**Answer: Only for a specific, shrinking segment.**

ShellGPT and Copilot CLI solve the core pain point—"I forgot the syntax for X"—with zero memorization friction: type a natural-language request, get a working command instantly. No setup, no studying, no recall practice. For the 80% of developers optimizing for speed-to-execution, this eliminates the need for collective entirely.

Collective's value proposition shifts to education and discipline: developers who *want* to understand common commands deeply, build muscle memory, and reduce dependence on AI-assisted lookup. This is a deliberate choice, not a friction-driven default. The market for this shrinks as LLM confidence grows—developers are less incentivized to memorize when generation becomes faster than recall.

However, this market is not zero. System administrators, DevOps engineers, and teams working in air-gapped environments (no API access, no LLM feasibility) still need offline command reference. More niche: learners and teachers who use spaced repetition as a pedagogical tool. The positioning requires acknowledging the LLM shift and selling collective as *complementary to* generation, not a replacement for it.

## Positioning options

### Option 1: "Command learning for the air-gapped and the disciplined"

**Pitch:** The only offline, safety-annotated, learning-enabled command reference for systems teams that can't reach an LLM and developers who memorize as a practice discipline.

**Target audience:** DevOps, SRE, system administrators; developers building muscle memory deliberately; organizations with network restrictions or regulatory compliance.

**Build/cut requirements:**
- Double down on the SM-2 drill UX and pedagogy (market this as "spaced-repetition learning for CLI mastery").
- Expand danger/undo metadata coverage (most entries still lack undo; make this a quality differentiator).
- Cut the tldr pack (it's a liability if the pitch is "curated, carefully vetted entries," not "massive aggregation"). Keep the 152 gems.
- Add offline-first documentation emphasizing air-gapped use and learning workflows.
- Build a learning curriculum (drill exercises, guides, progressions) to justify the memorization premise.

**Strongest argument against:** Tiny market. Developers in air-gapped environments are rare; developers who voluntarily memorize CLI commands in 2026 are rarer still. Revenue and adoption will remain niche.

### Option 2: "Navi's safety-conscious alternative"

**Pitch:** Interactive command search with built-in danger warnings and undo commands—navi for teams that can't deploy risky commands without guardrails.

**Target audience:** Enterprises with strict CLI governance, teams shipping infrastructure code, teaching environments.

**Build/cut requirements:**
- Drop spaced-repetition (no one choosing between navi and collective for learning; it's a differentiator-killer not a draw).
- Rebuild the TUI to match navi's speed and fzf integration more closely (or embed fzf directly).
- Add live variable sourcing to narrow the navi gap (hire for this; it's navi's moat).
- Cut custom packs as a user-facing feature (maintain the built-in 152, let users fork + rebuild binary if they need custom entries).
- Add per-entry safety review workflows and approval gates.

**Strongest argument against:** Navi is free, well-maintained, and already chosen by 17k developers. Competing on UX while keeping collective smaller is a slow losing game. Enterprise adoption requires sales and support infrastructure collective doesn't have.

### Option 3: "Sunset and focus on the learning angle as an internal project"

**Pitch:** Collective remains a personal tool and teaching experiment, not a public project. Publish the SM-2 learning framework and curated 152-entry corpus as reference material for building command-learning systems in larger tools.

**Target audience:** Educators, tool builders, pedagogical researchers; not end-users.

**Build/cut requirements:**
- Freeze the project at current state (no new feature debt).
- Move to a monorepo or archive with clear documentation of design decisions (why safety schema, why SM-2, why curated).
- Publish a guide: "Building Learning Systems for CLI Commands" (lessons from collective's design).
- Open-source the SM-2 drill core and schema as a library for others to build on.
- Remove public promotion; keep the tool for personal use and teaching examples.

**Strongest argument against:** Abandoning a working tool. If the market for Option 1 exists even at 1%, shipping a product (however niche) is more valuable than publishing essays. The tool works; the question is audience, not viability.

## Recommendation

**Option 1: "Command learning for the air-gapped and the disciplined."**

**Reasoning:**

Collective's only defensible position in 2026 is pedagogical. The interactive TUI and variable sourcing are navi problems (and navi wins them). The tldr pack is a tealdeer problem (and tealdeer wins it). But no tool in the ecosystem combines offline-first design + strict safety schema + spaced-repetition learning for shell commands. This intersection is empty. If developers are paying attention to learning at all—deliberate practice, memorization, discipline—collective serves them. The market is small, but it is real: educators teaching CLI mastery, system administrators drilling commands, developers in air-gapped environments, learners building shell fluency as a career investment.

The single fact that would change this: **if you measure adoption and find that >50% of active users report using the SM-2 drill feature regularly, the learning angle is real.** If it's <10%, the tool is primarily a navi alternative in people's heads, which is a losing position.

**Concrete actions:**
1. Cut the tldr pack entirely. It dilutes the positioning and creates maintenance debt.
2. Expand the built-in 152 entries with undo commands and deeper explanations (quality over quantity).
3. Build a 3-part learning guide: "Introduction to Drilling," "Mastering Shell Fundamentals," "Advanced DevOps Commands." Gamify progression.
4. Publish drill statistics and learning milestones (e.g., "You've drilled git workflows 42 times; time to advance").
5. Add an air-gapped installation guide and document use in restricted networks explicitly.
6. Drop any promise of competing with navi on variable sourcing or ecosystem; acknowledge the trade-off.

## What to cut

**The tldr pack (packs/tldr, 1,459 entries):** It adds bulk, maintenance surface area, and contradicts the "curated" positioning. A user who wants 1,459 tldr entries uses tealdeer, not collective. Keep the hand-curated 152 gems; remove the bulk import.

**Custom pack support (collective pack add <registry>):** Simplify to ship the built-in corpus only. Users who need custom entries can fork + rebuild the binary, which is fine for the "disciplined" segment. Remove the JSON registry and pack system entirely.

**Generic fuzzy search UX improvements competing with fzf:** The ratatui TUI is fine as-is. Don't try to outrun fzf; it's pointless. Spend that engineering on the drill UX instead.

**Variable system enhancements:** Don't chase navi's variable sourcing. It's too much work for too little return. Keep the static `<placeholder>` system; it's sufficient for the teaching and air-gapped use cases.

---

**Sources cited in this paper:**

- navi variable system: denisidoro/navi/docs/cheatsheet/syntax/README.md lines 54–138
- navi featured repos: denisidoro/cheats/featured_repos.txt (12 entries, verified via clone)
- tealdeer: dbrgn/tealdeer/README, 6,387 stars, live sync capability confirmed
- srsh (spaced repetition precedent): ryanbloom/srsh, 0 stars, 9 commits, last 2021-06-26
- Collective SM-2: src/sm2.rs (95 lines, algorithm verified), src/drill.rs (188 lines, tests passing)
- collective built-in entries: README.md lines 102–103, verified count 152 (10 root + 142 gems)
- tldr pack size: packs/tldr/ directory (1,459 YAML files, verified via find)
- Navi ecosystem: 17,372 GitHub stars (API verified 2026-07-29)
- ShellGPT competitive threat: TheR1D/shell_gpt, 12,224 stars, OS-aware generation confirmed
