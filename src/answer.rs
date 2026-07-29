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
pub fn derived_grade(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Match => 3,
        Outcome::Miss | Outcome::Revealed => 1,
    }
}

/// Does a typed answer match the expected command?
///
/// Formatting is forgiven, substance is not: whitespace collapses and flags may
/// be reordered, but positional arguments may not — their order carries
/// meaning. A `<placeholder>` slot accepts either the literal token or any
/// non-empty value, because drilling tests the shape of a command, not your
/// ability to invent a plausible port number.
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
    expected.len() == typed.len() && expected.iter().zip(typed).all(|(e, g)| token_matches(e, g))
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
///
/// Exactly one `<...>` slot per token is supported. A token like `<a>-<b>`
/// will not match `x-y`; the second `<b>` is treated as literal text.
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

    #[test]
    fn single_dash_flag_may_move_past_a_positional() {
        // Pins is_flag: if single-dash tokens were treated as positionals,
        // the positional sequences would be [ls, -l, /tmp] and [ls, /tmp, -l]
        // — different orders — and this would fail.
        assert!(matches("ls -l /tmp", "ls /tmp -l"));
    }

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
}
