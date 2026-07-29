# Typing drill — Design Spec

Date: 2026-07-29
Status: Approved (brainstorming session)
Builds on: v0.3.1 — the existing `collective drill` SM-2 flashcard session

## What

`collective drill` currently asks you to type the command, compares it with `==`,
prints `✓ exact` or `✗ differs`, and then throws the result away — you still
grade yourself 1–4. The check is decorative.

This makes it real. Three changes:

1. Compare **normalised**, so formatting differences don't fail you and
   substance differences do.
2. **Derive the SM-2 grade** from whether you got it right, with an override.
3. Show a **useful diff** on a miss — where you went wrong, not just that you did.

No new subcommand and no new flag. The prompt that already exists stops lying.

## Why this and not something else

Self-reported recall is generous. Pressing "good" because a command looked
familiar is recognition, not production, and recognition is the weaker signal by
a wide margin. Typing the command from memory and having it checked is the
difference between "I'd know that if I saw it" and "I can produce it."

It is also the one thing in this space with no maintained equivalent.
`ryanbloom/srsh` proved the idea in 2021 and has been dead since (0 stars, 9
commits). General-purpose spaced-repetition CLIs exist but carry no command
corpus. `navi`, the nearest neighbour by far, has no learning mode at all.

## 1. Scope

**In:** the matcher, grade derivation, the miss diff, and their tests.

**Out:** splitting `src/pack.rs` (711 lines, the largest file in the project).
It is real and queued, but it shares no code with this work and bundling a
refactor into a feature diff makes both harder to review. Separate spec.

**Out:** any change to `src/sm2.rs`. The scheduling algorithm is correct and
this feature only changes which grade reaches it.

## 2. The matcher

New module `src/answer.rs` — pure functions, no IO, table-tested. It is the
piece everything else depends on, so it gets its own file rather than growing
`drill.rs`.

```rust
/// Does a typed answer match the expected command?
pub fn matches(expected: &str, typed: &str) -> bool
```

Algorithm:

1. Collapse runs of whitespace and split both sides on whitespace.
2. If the token sequences are equal under the placeholder rule → **match**.
3. Otherwise split each side into **flags** (tokens starting with `-`) and
   **positionals** (everything else). If the positional sequences are equal in
   order *and* the flag collections are equal as multisets → **match**.
4. Otherwise → **miss**.

Step 3 is what makes reordering safe where reordering is meaningful:

| expected | typed | result | why |
|---|---|---|---|
| `git log --oneline -n5` | `git log -n5 --oneline` | match | same flags, same positionals |
| `git  log   --oneline` | `git log --oneline` | match | whitespace collapsed |
| `cp a b` | `cp b a` | **miss** | positional order carries meaning |
| `ls -la` | `ls -al` | **miss** | different tokens; see ceilings |

Flags are compared as a multiset rather than a set so that a repeated flag
(`-v -v`) is not silently equal to a single one.

### Placeholder rule

Entries contain `<token>` slots. An expected token containing `<…>` matches any
non-empty typed token sharing its literal prefix and suffix.

| expected token | typed | result |
|---|---|---|
| `:<port>` | `:<port>` | match — literal is fine |
| `:<port>` | `:8080` | match — a real value is fine |
| `:<port>` | `:` | miss — the slot is empty |
| `<file>` | `notes.txt` | match |

Implemented by splitting the expected token on its `<…>` span and checking the
typed token starts with the prefix, ends with the suffix, and has at least one
character between them. No regex dependency.

Rationale: drilling tests whether you know the *shape* of the command. Requiring
`<port>` verbatim is pedantic when you know it perfectly well; requiring a
plausible port number tests invention, not recall. Both pass.

## 3. Grade derivation

| outcome | derived grade |
|---|---|
| match | `3` (good) |
| miss | `1` (again) |
| Enter with no input (reveal) | `1` (again) |

The prompt states what it chose and offers the override:

```
graded: good   [Enter accepts · 1-4 overrides]
```

Enter accepts. `1`–`4` overrides. Anything else re-prompts, matching the input
handling already in `drill.rs`.

The override exists because the matcher cannot know intent. A typo you would
have got right, or a command you typed a better way than the corpus stores, are
both cases where the human is right and the checker is not. Deriving the grade
removes the *routine* dishonesty of self-grading without removing judgment.

## 4. The miss diff

On a miss, print both sides and mark the first differing token:

```
  expected: lsof -i :<port> -sTCP:LISTEN
  you typed: lsof -i :8080 -sTCP:LISTEN
                                ^ first difference
```

First-difference marking is chosen over a full diff algorithm deliberately: the
common failure is one wrong flag or a forgotten argument, and a character-level
diff of a one-line command is noise. If the first differing token turns out not
to be enough in practice, that is a cheap later change.

## 5. Deliberate ceilings

Stated here so they are decisions rather than surprises.

- **`-n 5` and `-n5` do not match each other.** Recognising them as equivalent
  needs per-command knowledge of which flags take values — effectively a shell
  parser plus a flag database. Enter-to-reveal and the grade override cover it.
- **Quoted strings containing spaces split into multiple tokens.** Both sides
  split identically so equality still works; only the flag-reordering path is
  confused by it, and reordering flags inside a `--jq '...'` string is not a
  real scenario.
- **No length threshold.** Some corpus entries are long one-liners that nobody
  should type from memory. Rather than tune a cutoff forever, the existing
  Enter-to-reveal path handles them: press Enter, see it, grade honestly.
- **Case is significant.** Shell commands are case-sensitive and normalising
  case would accept `GIT LOG` as correct.

## 6. Files

| file | change |
|---|---|
| `src/answer.rs` | **new** — `matches()`, placeholder handling, first-difference location |
| `src/drill.rs` | use `matches()`, derive the grade, print the diff; ~20 lines changed in `run()` |
| `src/main.rs` | add `mod answer;` |

`drill.rs` is 188 lines and stays comfortably sized. `answer.rs` is expected to
be roughly 80 lines plus its tests.

## 7. Testing

`matches()` is a pure function, so it gets a table of pairs asserted directly —
each row a case the design commits to:

- whitespace: leading, trailing, and internal runs collapse
- flag reordering matches; positional reordering does **not**
- repeated flags are not equal to a single occurrence
- placeholder: literal, real value, and empty slot
- a flag in the expected answer omitted by the typist is a miss
- empty typed input is a miss, not a panic
- case difference is a miss

Grade derivation is tested at the seam rather than through stdin: a small
function mapping outcome to grade, asserted for match, miss, and reveal, with
the override applied on top.

The existing `drill.rs` tests (`pick_due`, state round-trip, corrupt state)
must keep passing untouched — this feature changes the session loop, not
scheduling or persistence.

**Every new test must fail if the behaviour it covers is removed.** This project
has had four tests that passed for the wrong reason; each new assertion here
gets checked by breaking the line it depends on and confirming the failure.

## Rollout order

1. `src/answer.rs` with its full test table, wired to nothing.
2. `drill.rs` uses it: normalised comparison replaces `==`.
3. Grade derivation and the override prompt.
4. The miss diff.

Each step leaves the suite green.

## Done criteria

- `collective drill` accepts `git log -n5 --oneline` for `git log --oneline -n5`,
  and rejects `cp b a` for `cp a b`.
- A typed match grades itself `good` without a second prompt; Enter accepts and
  `1`–`4` overrides.
- A miss shows expected, typed, and the first difference.
- Full suite green, clippy clean at `-D warnings`.
- No change to scheduling, persistence, or any other subcommand.
