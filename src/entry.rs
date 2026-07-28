use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub title: String,
    pub cmd: String,
    #[serde(default)]
    pub undo: Option<String>,
    pub platform: Vec<String>,
    pub domains: Vec<String>,
    pub danger: Danger,
    pub explanation: String,
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Danger {
    Low,
    Medium,
    High,
}

impl Entry {
    pub fn validate(&self) -> Result<(), String> {
        let id_ok = !self.id.is_empty()
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_ok {
            return Err(format!(
                "bad id {:?}: use lowercase/digits/hyphens",
                self.id
            ));
        }
        if self.title.trim().is_empty() {
            return Err(format!("{}: empty title", self.id));
        }
        if self.cmd.trim().is_empty() {
            return Err(format!("{}: empty cmd", self.id));
        }
        if self.explanation.trim().is_empty() {
            return Err(format!("{}: empty explanation", self.id));
        }
        if self.platform.is_empty() {
            return Err(format!("{}: platform required", self.id));
        }
        if self.domains.is_empty() {
            return Err(format!("{}: at least one domain", self.id));
        }
        Ok(())
    }
}

impl Danger {
    pub fn parse(s: &str) -> Option<Danger> {
        match s.trim().to_lowercase().as_str() {
            "low" => Some(Danger::Low),
            "medium" => Some(Danger::Medium),
            "high" => Some(Danger::High),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
id: pmset-disable-sleep
title: Disable sleep entirely on macOS
cmd: sudo pmset -a disablesleep 1
undo: sudo pmset -a disablesleep 0
platform: [macos]
domains: [power]
danger: medium
explanation: Hard-disables sleep even with lid closed.
source: https://ss64.com/mac/pmset.html
tags: [sleep, clamshell]
"#;

    #[test]
    fn parses_valid_entry() {
        let e: Entry = serde_yaml::from_str(GOOD).unwrap();
        assert_eq!(e.id, "pmset-disable-sleep");
        assert_eq!(e.danger, Danger::Medium);
        assert_eq!(e.undo.as_deref(), Some("sudo pmset -a disablesleep 0"));
        assert!(e.validate().is_ok());
    }

    #[test]
    fn rejects_bad_id_chars() {
        let e: Entry =
            serde_yaml::from_str(&GOOD.replace("pmset-disable-sleep", "Bad_ID!")).unwrap();
        assert!(e.validate().is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let bad = format!("{GOOD}\nbogus_field: 1");
        assert!(serde_yaml::from_str::<Entry>(&bad).is_err());
    }

    #[test]
    fn rejects_empty_cmd() {
        let e: Entry =
            serde_yaml::from_str(&GOOD.replace("cmd: sudo pmset -a disablesleep 1", "cmd: \"\""))
                .unwrap();
        assert!(e.validate().is_err());
    }
}
