# Typing Drill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `collective drill` check the command you typed from memory, grade you on whether you got it right, and show you where you went wrong.

**Architecture:** A new pure module `src/answer.rs` owns all comparison logic — normalised matching, placeholder slots, and locating the first differing token. `src/drill.rs` calls it, replacing today's `typed == e.cmd` and replacing the self-graded 1–4 prompt with a derived grade the user can override. Nothing else in the codebase changes.

**Tech Stack:** Rust 2021, no new dependencies. Existing: `serde`, `serde_json` for drill state; `rand` for card shuffling.

**Spec:** `docs/superpowers/specs/2026-07-29-typing-drill-design.md`

## Global Constraints

- **No new dependencies.** No regex crate — placeholder matching uses prefix/suffix string comparison. Nothing goes into `Cargo.toml`.
- **This package has NO lib target.** Unit tests run under plain `cargo test`; `cargo test --lib` fails with "no library targets found".
- `cargo clippy --all-targets -- -D warnings` must exit 0 and `cargo fmt` must be applied before every commit. The branch is genuinely clean, so any warning is yours.
- **Case is significant.** Never lowercase either side — shell commands are case-sensitive and `GIT LOG` is not correct.
- **Do not modify `src/sm2.rs`.** Scheduling is correct; this feature only changes which grade reaches `sm2::review`.
- **Do not modify `pick_due`, `load_state`, or `save_state`.** This feature changes the session loop, not scheduling or persistence. Their existing tests must keep passing untouched.
- **Every new test must fail if the behaviour it covers is removed.** This project has shipped four tests that passed for the wrong reason. For each task, break the line the test depends on, confirm the test fails, restore, confirm it passes. Report both results.
- Follow repo idioms: `#[cfg(test)] mod tests` at the end of each file, `pub fn` with a doc comment on anything another module calls.

## File Structure

| file | responsibility |
|---|---|
| `src/answer.rs` | **new** — all answer-comparison logic: `matches`, `Outcome`, `derived_grade`, `first_difference`. Pure, no IO, fully unit-testable. |
| `src/drill.rs` | **modify** — session loop uses the matcher, derives the grade, prints the diff. Roughly 25 lines change inside `run()`. |
| `src/main.rs` | **modify** — one line: `mod answer;` |

`answer.rs` lands at roughly 90 lines plus tests. `drill.rs` stays at about 200.

---

### Task 1: The matcher

**Files:**
- Create: `src/answer.rs`
- Modify: `src/main.rs` (add `mod answer;`)

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: `pub fn matches(expected: &str, typed: &str) -> bool` — used by Tasks 2 and 3

- [ ] **Step 1: Write the failing tests**

Create `src/answer.rs` containing a stub plus the full test table:

```rust
/// Does a typed answer match the expected command?
///
/// Formatting is forgiven, substance is not: whitespace collapses and flags may
/// be reordered, but positional arguments may not — their order carries
/// meaning. A `<placeholder>` slot accepts either the literal token or any
/// non-empty value, because drilling tests the shape of a command, not your
/// ability to invent a plausible port number.
pub fn matches(_expected: &str, _typed: &str) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_commands_match() {
        assert!(matches("git status", "git status"));
    }

    #[test]
    fn whitespace_runs_collapse() {
        assert!(matches("git  log   --oneline", "git log --oneline"));
        assert!(matches("git log --oneline", "  git log --oneline  "));
    }

    #[test]
    fn flags_may_be_reordered() {
        assert!(matches("git log --oneline -n5", "git log -n5 --oneline"));
    }

    #[test]
    fn positionals_may_not_be_reordered() {
        assert!(!matches("cp a b", "cp b a"));
    }

    #[test]
    fn repeated_flag_is_not_equal_to_one() {
        assert!(!matches("ssh -v -v host", "ssh -v host"));
    }

    #[test]
    fn joined_short_flags_are_distinct_tokens() {
        // -la and -al are different strings; recognising them as equivalent
        // would need per-command knowledge of flag semantics.
        assert!(!matches("ls -la", "ls -al"));
    }

    #[test]
    fn missing_flag_is_a_miss() {
        assert!(!matches("ls -la /tmp", "ls /tmp"));
    }

    #[test]
    fn placeholder_accepts_the_literal_token() {
        assert!(matches("lsof -i :<port>", "lsof -i :<port>"));
    }

    #[test]
    fn placeholder_accepts_a_real_value() {
        assert!(matches("lsof -i :<port>", "lsof -i :8080"));
    }

    #[test]
    fn placeholder_rejects_an_empty_slot() {
        assert!(!matches("lsof -i :<port>", "lsof -i :"));
    }

    #[test]
    fn bare_placeholder_accepts_any_value() {
        assert!(matches("cat <file>", "cat notes.txt"));
        assert!(matches("cat <file>", "cat <file>"));
    }

    #[test]
    fn case_differences_are_a_miss() {
        assert!(!matches("git log", "GIT LOG"));
    }

    #[test]
    fn empty_input_is_a_miss_not_a_panic() {
        assert!(!matches("git log", ""));
        assert!(!matches("git log", "   "));
        assert!(!matches("", "git log"));
    }

    #[test]
    fn extra_trailing_token_is_a_miss() {
        assert!(!matches("git log", "git log --oneline"));
    }
}
```

Add `mod answer;` to `src/main.rs`, alphabetically first in the module list (before `mod ai;`).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test answer`
Expected: FAIL — every test panics at `todo!()`.

- [ ] **Step 3: Implement the matcher**

Replace the `todo!()` body and add the helpers below it:

```rust
pub fn matches(expected: &str, typed: &str) -> bool {
    let exp: Vec<&str> = expected.split_whitespace().collect();
    let got: Vec<&str> = typed.split_whitespace().collect();
    if exp.is_empty() || got.is_empty() {
        return false;
    }
    if seq_matches(&exp, &got) {
        return true;
    }
    // Second chance: flags may appear in any order, positionals may not.
    let (exp_flags, exp_pos) = split_flags(&exp);
    let (got_flags, got_pos) = split_flags(&got);
    seq_matches(&exp_pos, &got_pos) && multiset_matches(&exp_flags, &got_flags)
}

/// A token is a flag if it leads with `-` and carries something after it, so a
/// bare `-` (stdin, by convention) stays a positional.
fn is_flag(token: &str) -> bool {
    token.starts_with('-') && token.len() > 1
}

fn split_flags<'a>(tokens: &[&'a str]) -> (Vec<&'a str>, Vec<&'a str>) {
    let mut flags = Vec::new();
    let mut positionals = Vec::new();
    for t in tokens {
        if is_flag(t) {
            flags.push(*t);
        } else {
            positionals.push(*t);
        }
    }
    (flags, positionals)
}

fn seq_matches(expected: &[&str], typed: &[&str]) -> bool {
    expected.len() == typed.len()
        && expected
            .iter()
            .zip(typed)
            .all(|(e, g)| token_matches(e, g))
}

/// Compared as a multiset, not a set, so `-v -v` is not equal to `-v`.
fn multiset_matches(expected: &[&str], typed: &[&str]) -> bool {
    let mut e = expected.to_vec();
    let mut g = typed.to_vec();
    e.sort_unstable();
    g.sort_unstable();
    seq_matches(&e, &g)
}

/// One token. An expected token containing `<...>` matches any non-empty typed
/// token sharing its literal prefix and suffix, so `:<port>` accepts both
/// `:<port>` and `:8080` but not a bare `:`.
fn token_matches(expected: &str, typed: &str) -> bool {
    if expected == typed {
        return true;
    }
    let Some(open) = expected.find('<') else {
        return false;
    };
    let Some(close_rel) = expected[open..].find('>') else {
        return false;
    };
    let close = open + close_rel;
    let prefix = &expected[..open];
    let suffix = &expected[close + 1..];
    typed.len() > prefix.len() + suffix.len()
        && typed.starts_with(prefix)
        && typed.ends_with(suffix)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test answer`
Expected: PASS — all 13 tests.

- [ ] **Step 5: Falsify the tests**

Temporarily change `is_flag` to `token.starts_with("--")`, run `cargo test answer`, and confirm `flags_may_be_reordered` still passes but `repeated_flag_is_not_equal_to_one` or `missing_flag_is_a_miss` changes behaviour. Then temporarily make `token_matches` return `expected == typed` outright and confirm the three placeholder tests FAIL. Restore both, confirm all pass again. Report both results in your report — a test that cannot fail is not a test.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/answer.rs src/main.rs
git commit -m "feat: normalised answer matching for drills

Compares a typed command with the expected one, forgiving formatting and
strict about substance: whitespace collapses, flags may be reordered,
positional arguments may not because their order carries meaning. A
<placeholder> slot accepts the literal token or any non-empty value."
```

---

### Task 2: Drill uses the matcher

**Files:**
- Modify: `src/drill.rs:86-95` (inside `run()`)

**Interfaces:**
- Consumes: `answer::matches(&str, &str) -> bool` from Task 1
- Produces: nothing new; this task only changes behaviour inside `run()`

- [ ] **Step 1: Replace the exact comparison**

`src/drill.rs` currently reads:

```rust
        let typed = buf.trim();
        println!("  {}", e.cmd);
        if !typed.is_empty() {
            let mark = if typed == e.cmd {
                "✓ exact"
            } else {
                "✗ differs"
            };
            println!("  you typed: {typed}  {mark}");
        }
```

Replace that block with:

```rust
        let typed = buf.trim();
        let correct = !typed.is_empty() && crate::answer::matches(&e.cmd, typed);
        println!("  {}", e.cmd);
        if !typed.is_empty() {
            let mark = if correct { "✓ correct" } else { "✗ not quite" };
            println!("  you typed: {typed}  {mark}");
        }
```

`correct` is bound here because Task 3 uses it to derive the grade. The wording
changes from "exact" to "correct" because the comparison is no longer exact —
saying "exact" of a normalised match would be a lie.

- [ ] **Step 2: Verify the behaviour by hand**

```bash
cargo run -- drill --domain git
```

Type a command with reordered flags — for a card whose answer is
`git log --oneline -n5`, type `git log -n5 --oneline`. Expected: `✓ correct`.
Then type something wrong and expect `✗ not quite`. Press Ctrl-D to exit.

This step is manual because `run()` reads stdin directly; the matcher itself is
covered by Task 1's tests.

- [ ] **Step 3: Run the suite**

Run: `cargo test`
Expected: PASS. The existing `drill.rs` tests cover `pick_due` and state
persistence, neither of which this touches.

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/drill.rs
git commit -m "feat: drill accepts normalised answers

The session compared with == , so a reordered flag or a doubled space failed
you on a command you knew. It now uses the normalised matcher."
```

---

### Task 3: Derive the grade

**Files:**
- Modify: `src/answer.rs` (add `Outcome` and `derived_grade`)
- Modify: `src/drill.rs:96-114` (the grade prompt inside `run()`)

**Interfaces:**
- Consumes: `answer::matches` from Task 1
- Produces:
  - `pub enum Outcome { Match, Miss, Revealed }`
  - `pub fn derived_grade(outcome: Outcome) -> u8`

- [ ] **Step 1: Write the failing tests**

Add to `src/answer.rs`, above its `#[cfg(test)]` block:

```rust
/// How a card was answered, before the user gets a say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Typed and matched.
    Match,
    /// Typed and did not match.
    Miss,
    /// Not attempted — the answer was revealed.
    Revealed,
}

/// The SM-2 grade a session proposes for an outcome. Typing is far better
/// evidence of recall than self-report, so the session grades you and lets you
/// override rather than asking every time.
pub fn derived_grade(_outcome: Outcome) -> u8 {
    todo!()
}
```

And these tests inside the existing `mod tests`:

```rust
    #[test]
    fn a_match_grades_good() {
        assert_eq!(derived_grade(Outcome::Match), 3);
    }

    #[test]
    fn a_miss_grades_again() {
        assert_eq!(derived_grade(Outcome::Miss), 1);
    }

    #[test]
    fn a_reveal_grades_again() {
        assert_eq!(derived_grade(Outcome::Revealed), 1);
    }

    #[test]
    fn derived_grades_are_in_sm2_range() {
        for o in [Outcome::Match, Outcome::Miss, Outcome::Revealed] {
            let g = derived_grade(o);
            assert!((1..=4).contains(&g), "grade {g} out of range for {o:?}");
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test answer`
Expected: FAIL — the four new tests panic at `todo!()`.

- [ ] **Step 3: Implement**

```rust
pub fn derived_grade(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Match => 3,
        Outcome::Miss | Outcome::Revealed => 1,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test answer`
Expected: PASS.

- [ ] **Step 5: Replace the grade prompt in `drill.rs`**

The current block is:

```rust
        let grade = loop {
            print!("grade  1=again 2=hard 3=good 4=easy: ");
            io::stdout().flush().unwrap();
            let mut g = String::new();
            match stdin.read_line(&mut g) {
                Ok(0) => {
                    println!("\nsession ended.");
                    return;
                }
                Ok(_) => match g.trim().parse::<u8>() {
                    Ok(n @ 1..=4) => break n,
                    _ => continue,
                },
                Err(err) => {
                    eprintln!("input error: {err}");
                    return;
                }
            }
        };
```

Replace it with:

```rust
        let outcome = if typed.is_empty() {
            crate::answer::Outcome::Revealed
        } else if correct {
            crate::answer::Outcome::Match
        } else {
            crate::answer::Outcome::Miss
        };
        let proposed = crate::answer::derived_grade(outcome);
        let label = match proposed {
            1 => "again",
            2 => "hard",
            3 => "good",
            _ => "easy",
        };
        let grade = loop {
            print!("graded: {label}   [Enter accepts · 1-4 overrides]: ");
            io::stdout().flush().unwrap();
            let mut g = String::new();
            match stdin.read_line(&mut g) {
                Ok(0) => {
                    println!("\nsession ended.");
                    return;
                }
                Ok(_) => {
                    let g = g.trim();
                    if g.is_empty() {
                        break proposed;
                    }
                    match g.parse::<u8>() {
                        Ok(n @ 1..=4) => break n,
                        _ => continue,
                    }
                }
                Err(err) => {
                    eprintln!("input error: {err}");
                    return;
                }
            }
        };
```

Empty input accepts the proposal; `1`–`4` overrides; anything else re-prompts,
matching the loop's existing behaviour. Ctrl-D still ends the session.

- [ ] **Step 6: Verify by hand and run the suite**

```bash
cargo run -- drill --domain git
```

Answer one card correctly and press Enter at the grade prompt — expect
`graded: good` and no second question. Answer one wrong and press Enter —
expect `graded: again`. Answer one wrong and type `3` — expect the override
taken. Ctrl-D to exit.

Run: `cargo test`
Expected: PASS.

- [ ] **Step 7: Falsify**

Temporarily change `derived_grade`'s `Outcome::Match` arm to return `1`, run
`cargo test answer`, and confirm `a_match_grades_good` FAILS. Restore and
confirm it passes. Report both results.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/answer.rs src/drill.rs
git commit -m "feat: derive the drill grade from what you typed

Self-reported recall is generous - pressing 'good' because a command looked
familiar is recognition, not production. The session now grades from whether
the typed answer matched, and Enter accepts it. 1-4 still overrides, because
the matcher cannot know that a typo was a command you actually knew."
```

---

### Task 4: Show where the answer went wrong

**Files:**
- Modify: `src/answer.rs` (add `first_difference`)
- Modify: `src/drill.rs` (print the marker on a miss)

**Interfaces:**
- Consumes: nothing new
- Produces: `pub fn first_difference(expected: &str, typed: &str) -> Option<usize>` — the index of the first differing whitespace-separated token

- [ ] **Step 1: Write the failing tests**

Add to `src/answer.rs` above the test module:

```rust
/// Index of the first whitespace-separated token that differs, for pointing at
/// the mistake. `None` when the answer matches token-for-token. Compares in
/// order, so a reordered-but-correct answer still reports the first positional
/// difference — the marker is a hint, not a verdict.
pub fn first_difference(_expected: &str, _typed: &str) -> Option<usize> {
    todo!()
}
```

And these tests inside `mod tests`:

```rust
    #[test]
    fn no_difference_when_tokens_match() {
        assert_eq!(first_difference("git log --oneline", "git log --oneline"), None);
    }

    #[test]
    fn reports_the_first_differing_token() {
        assert_eq!(first_difference("git log --oneline", "git log --graph"), Some(2));
        assert_eq!(first_difference("cp a b", "mv a b"), Some(0));
    }

    #[test]
    fn a_short_answer_differs_at_the_missing_token() {
        assert_eq!(first_difference("ls -la /tmp", "ls -la"), Some(2));
    }

    #[test]
    fn a_long_answer_differs_at_the_extra_token() {
        assert_eq!(first_difference("git log", "git log --oneline"), Some(2));
    }

    #[test]
    fn a_placeholder_value_is_not_a_difference() {
        assert_eq!(first_difference("lsof -i :<port>", "lsof -i :8080"), None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test answer`
Expected: FAIL — the five new tests panic at `todo!()`.

- [ ] **Step 3: Implement**

```rust
pub fn first_difference(expected: &str, typed: &str) -> Option<usize> {
    let exp: Vec<&str> = expected.split_whitespace().collect();
    let got: Vec<&str> = typed.split_whitespace().collect();
    for i in 0..exp.len().max(got.len()) {
        match (exp.get(i), got.get(i)) {
            (Some(e), Some(g)) if token_matches(e, g) => continue,
            _ => return Some(i),
        }
    }
    None
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test answer`
Expected: PASS.

- [ ] **Step 5: Print the marker on a miss**

In `src/drill.rs`, the block from Task 2 currently ends with the `you typed`
line. Extend it so a miss points at the token:

```rust
        let typed = buf.trim();
        let correct = !typed.is_empty() && crate::answer::matches(&e.cmd, typed);
        println!("  {}", e.cmd);
        if !typed.is_empty() {
            let mark = if correct { "✓ correct" } else { "✗ not quite" };
            println!("  you typed: {typed}  {mark}");
            if !correct {
                if let Some(i) = crate::answer::first_difference(&e.cmd, typed) {
                    let caret_col: usize = typed
                        .split_whitespace()
                        .take(i)
                        .map(|t| t.chars().count() + 1)
                        .sum();
                    println!("  {}{} first difference", " ".repeat(caret_col + 11), "^");
                }
            }
        }
```

The `+ 11` aligns the caret under the typed command, accounting for the
`  you typed: ` prefix. Counting `chars()` rather than bytes keeps the caret
aligned for non-ASCII commands.

- [ ] **Step 6: Verify by hand and run the suite**

```bash
cargo run -- drill --domain git
```

Answer a card with one wrong flag and confirm the caret sits under the wrong
token, not at the start of the line. Ctrl-D to exit.

Run: `cargo test`
Expected: PASS — 102 existing plus 22 new.

- [ ] **Step 7: Falsify**

Temporarily change `first_difference` to always `return None`, run
`cargo test answer`, and confirm `reports_the_first_differing_token` FAILS.
Restore, confirm it passes. Report both results.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/answer.rs src/drill.rs
git commit -m "feat: point at the first wrong token on a missed drill

'✗ not quite' told you that you were wrong without telling you where. A miss
now marks the first differing token, which is almost always the whole story
for a one-line command."
```

---

## Self-Review

**Spec coverage:**

| spec section | task |
|---|---|
| §2 matcher: whitespace, flag reorder, positional order | Task 1 |
| §2 placeholder rule | Task 1 |
| §3 grade derivation and override | Task 3 |
| §4 miss diff, first differing token | Task 4 |
| §5 ceilings (`-n 5`, quoting, no length threshold, case) | Task 1 tests pin the `-la`/`-al` and case cases; the rest are non-features requiring no code |
| §6 files: `answer.rs` new, `drill.rs` modified, `mod answer;` | Tasks 1–4 |
| §7 testing, including falsification | Every task ends with a falsification step |

No gaps. Nothing in the plan touches `sm2.rs`, `pick_due`, `load_state`, or
`save_state`, as the spec requires.

**Placeholder scan:** No TBDs. Every step carries the code it needs. The two
`todo!()` bodies are intentional TDD red phases, resolved within their own task.

**Type consistency:** `matches(&str, &str) -> bool` is defined in Task 1 and
called identically in Tasks 2 and 3. `Outcome` and `derived_grade(Outcome) -> u8`
are defined in Task 3 and used only there. `first_difference(&str, &str) ->
Option<usize>` is defined in Task 4 and used only there. `token_matches` is a
private helper introduced in Task 1 and reused by Task 4's `first_difference` —
both live in `answer.rs`, so no visibility change is needed. The `correct`
binding introduced in Task 2 is consumed by Tasks 3 and 4.

**Sequencing:** every task ends with a green suite. Task 2 changes behaviour
before the grade derivation exists, which is safe: the old self-grade prompt
still runs and simply ignores `correct` until Task 3 wires it.
