use serde::Deserialize;

#[derive(Debug, PartialEq, Deserialize)]
pub struct AiFields {
    pub title: String,
    pub domains: Vec<String>,
    pub danger: String,
    pub explanation: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub undo: String,
    pub platform: Vec<String>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Backend {
    Api,
    Cli,
    Manual,
}

pub fn select_backend(has_api_key: bool, claude_on_path: bool) -> Backend {
    if has_api_key {
        Backend::Api
    } else if claude_on_path {
        Backend::Cli
    } else {
        Backend::Manual
    }
}

/// Extract the first balanced {...} JSON object and deserialize it.
pub fn parse_response(text: &str) -> Result<AiFields, String> {
    let start = text.find('{').ok_or("no JSON object in response")?;
    let mut depth = 0usize;
    let mut end = None;
    for (i, c) in text[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end.ok_or("unterminated JSON object")?;
    serde_json::from_str(&text[start..end]).map_err(|e| format!("bad JSON: {e}"))
}

fn model() -> String {
    std::env::var("COLLECTIVE_MODEL").unwrap_or_else(|_| "claude-haiku-4-5-20251001".to_string())
}

fn prompt(cmd: &str) -> String {
    format!(
        "For the shell command below, return ONLY a JSON object with keys \
title (string, imperative), domains (array from: power, macos-admin, network, \
files, disk, debugging, security, shell, git, media), danger (\"low\"|\"medium\"|\"high\"; \
high=destructive/irreversible, medium=sudo/writes-system-state), explanation \
(2-3 sentences), tags (array of 3-5 keywords), undo (string, \"\" if none), \
platform (array, e.g. [\"macos\"] or [\"macos\",\"linux\"]). No prose.\n\nCommand: {cmd}"
    )
}

pub fn populate(cmd: &str) -> Result<AiFields, String> {
    let has_key = std::env::var("ANTHROPIC_API_KEY").map(|k| !k.is_empty()).unwrap_or(false);
    let claude = which_claude();
    match select_backend(has_key, claude.is_some()) {
        Backend::Api => populate_api(cmd),
        Backend::Cli => populate_cli(cmd, &claude.unwrap()),
        Backend::Manual => Err("no ANTHROPIC_API_KEY and no `claude` on PATH".to_string()),
    }
}

fn which_claude() -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    for dir in path.split(':') {
        let cand = std::path::Path::new(dir).join("claude");
        if cand.is_file() {
            return Some(cand.to_string_lossy().into_owned());
        }
    }
    None
}

fn populate_api(cmd: &str) -> Result<AiFields, String> {
    let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| "no API key")?;
    let body = serde_json::json!({
        "model": model(),
        "max_tokens": 1024,
        "messages": [{"role": "user", "content": prompt(cmd)}]
    });
    let resp = ureq::post("https://api.anthropic.com/v1/messages")
        .set("x-api-key", &key)
        .set("anthropic-version", "2023-06-01")
        .set("content-type", "application/json")
        .send_json(body)
        .map_err(|e| format!("API request failed: {e}"))?;
    let v: serde_json::Value = resp.into_json().map_err(|e| format!("bad API response: {e}"))?;
    let text = v["content"][0]["text"].as_str().ok_or("no text in API response")?;
    parse_response(text)
}

fn populate_cli(cmd: &str, claude: &str) -> Result<AiFields, String> {
    let out = std::process::Command::new(claude)
        .args(["-p", &prompt(cmd), "--output-format", "json", "--model", &model()])
        .output()
        .map_err(|e| format!("claude invocation failed: {e}"))?;
    if !out.status.success() {
        return Err(format!("claude exited with {}", out.status));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    // claude --output-format json wraps the model text in {"result": "..."};
    // fall back to treating stdout as the text if that shape is absent.
    let text = match serde_json::from_str::<serde_json::Value>(&stdout) {
        Ok(v) => v["result"].as_str().unwrap_or(&stdout).to_string(),
        Err(_) => stdout.to_string(),
    };
    parse_response(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_prefers_api_key() {
        assert_eq!(select_backend(true, true), Backend::Api);
        assert_eq!(select_backend(true, false), Backend::Api);
    }

    #[test]
    fn backend_falls_back_to_cli_then_manual() {
        assert_eq!(select_backend(false, true), Backend::Cli);
        assert_eq!(select_backend(false, false), Backend::Manual);
    }

    #[test]
    fn parses_clean_json() {
        let f = parse_response(r#"{"title":"T","domains":["shell"],"danger":"low","explanation":"E","tags":["a"],"undo":"","platform":["macos"]}"#).unwrap();
        assert_eq!(f.title, "T");
        assert_eq!(f.domains, vec!["shell"]);
        assert_eq!(f.danger, "low");
    }

    #[test]
    fn parses_json_wrapped_in_prose_or_fences() {
        let text = "Here you go:\n```json\n{\"title\":\"T\",\"domains\":[\"git\"],\"danger\":\"medium\",\"explanation\":\"E\",\"tags\":[\"x\"],\"undo\":\"\",\"platform\":[\"macos\"]}\n```\n";
        let f = parse_response(text).unwrap();
        assert_eq!(f.danger, "medium");
        assert_eq!(f.domains, vec!["git"]);
    }

    #[test]
    fn malformed_json_errors() {
        assert!(parse_response("no json here").is_err());
        assert!(parse_response("{ not valid").is_err());
    }
}
