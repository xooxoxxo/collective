use std::io::{self, Write};

/// Inner names of `<...>` tokens, unique, first-seen order.
pub fn tokens(cmd: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes = cmd.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(rel) = cmd[i + 1..].find('>') {
                let name = &cmd[i + 1..i + 1 + rel];
                if !name.is_empty() && !name.contains('<') && !out.iter().any(|t| t == name) {
                    out.push(name.to_string());
                }
                i = i + 1 + rel + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Replace `<name>` with its answer. Empty answer leaves the token in place.
pub fn fill(cmd: &str, answers: &[(String, String)]) -> String {
    let mut out = cmd.to_string();
    for (name, ans) in answers {
        if !ans.is_empty() {
            out = out.replace(&format!("<{name}>"), ans);
        }
    }
    out
}

/// Prompt for each token on stdin and substitute. No tokens -> unchanged.
pub fn fill_interactive(cmd: &str) -> String {
    let toks = tokens(cmd);
    if toks.is_empty() {
        return cmd.to_string();
    }
    let mut answers = Vec::new();
    for t in &toks {
        print!("<{t}>: ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        answers.push((t.clone(), line.trim().to_string()));
    }
    fill(cmd, &answers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_tokens() {
        assert!(tokens("git status").is_empty());
    }

    #[test]
    fn one_token() {
        assert_eq!(tokens("lsof -iTCP:<port>"), vec!["port"]);
    }

    #[test]
    fn repeated_token_collapses() {
        assert_eq!(tokens("cp <file> <file>.bak"), vec!["file"]);
    }

    #[test]
    fn multiple_distinct_tokens_in_order() {
        assert_eq!(tokens("scp <src> <host>:<dest>"), vec!["src", "host", "dest"]);
    }

    #[test]
    fn fill_substitutes_and_leaves_empty() {
        let cmd = "lsof -iTCP:<port> <host>";
        let answers = vec![("port".to_string(), "3000".to_string()), ("host".to_string(), String::new())];
        assert_eq!(fill(cmd, &answers), "lsof -iTCP:3000 <host>");
    }
}
