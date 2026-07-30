//! App registry, app derivation from commands, and PATH availability.
//! No `crate::` imports: build.rs includes this file via #[path] to
//! validate apps.yaml at build time (same pattern as entry.rs).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct AppInfo {
    pub binary: String,
    pub name: String,
    pub description: String,
    pub homepage: String,
    #[serde(default)]
    pub install: Install,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Install {
    pub brew: Option<String>,
    pub apt: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Registry {
    pub apps: Vec<AppInfo>,
}

impl Registry {
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        for a in &self.apps {
            for (field, v) in [
                ("binary", &a.binary),
                ("name", &a.name),
                ("description", &a.description),
                ("homepage", &a.homepage),
            ] {
                if v.trim().is_empty() {
                    return Err(format!("app {:?}: empty {field}", a.binary));
                }
            }
            if !seen.insert(a.binary.clone()) {
                return Err(format!("duplicate app binary: {}", a.binary));
            }
        }
        Ok(())
    }
}

#[allow(dead_code)]
pub fn registry() -> &'static HashMap<String, AppInfo> {
    static REG: OnceLock<HashMap<String, AppInfo>> = OnceLock::new();
    REG.get_or_init(|| {
        let reg: Registry = serde_yaml_bw::from_str(include_str!("../apps.yaml"))
            .expect("apps.yaml validated at build time");
        reg.apps.into_iter().map(|a| (a.binary.clone(), a)).collect()
    })
}

const BUILTINS: &[&str] = &[
    "cd", "export", "alias", "set", "unset", "source", "eval", "echo", "read",
    "trap", "ulimit",
];

/// The binary an entry needs: explicit `app:` field wins, else derived.
pub fn entry_binary(app_field: Option<&str>, cmd: &str) -> Option<String> {
    match app_field {
        Some(a) => Some(a.to_string()),
        None => derive_binary(cmd),
    }
}

/// The binary a command needs, or None for builtins/empty commands.
#[allow(dead_code)]
pub fn derive_binary(cmd: &str) -> Option<String> {
    let tok = cmd
        .split_whitespace()
        .find(|t| *t != "sudo" && *t != "env" && !t.contains('='))?;
    let base = tok.rsplit('/').next().unwrap_or(tok);
    if BUILTINS.contains(&base) {
        return None;
    }
    Some(base.to_string())
}

/// PATH presence for the binaries scanned this run. Binaries never scanned
/// resolve to available — graying must never be a false positive.
pub struct Availability(HashMap<String, bool>);

impl Availability {
    pub fn scan<'a>(binaries: impl Iterator<Item = &'a str>, path_var: &str) -> Availability {
        let dirs: Vec<&str> = path_var.split(':').filter(|d| !d.is_empty()).collect();
        let mut map = HashMap::new();
        for bin in binaries {
            if map.contains_key(bin) {
                continue;
            }
            let found = dirs.iter().any(|d| is_executable(&std::path::Path::new(d).join(bin)));
            map.insert(bin.to_string(), found);
        }
        Availability(map)
    }

    pub fn available(&self, binary: Option<&str>) -> bool {
        match binary {
            None => true,
            Some(b) => *self.0.get(b).unwrap_or(&true),
        }
    }
}

fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// The install command for the running platform, if the registry has one.
#[allow(dead_code)]
pub fn install_for_platform(app: &AppInfo) -> Option<&str> {
    #[cfg(target_os = "macos")]
    return app.install.brew.as_deref();
    #[cfg(not(target_os = "macos"))]
    return app.install.apt.as_deref();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_plain_and_prefixed_commands() {
        assert_eq!(derive_binary("btop"), Some("btop".into()));
        assert_eq!(derive_binary("rg --files"), Some("rg".into()));
        assert_eq!(derive_binary("sudo pmset -a disablesleep 1"), Some("pmset".into()));
        assert_eq!(derive_binary("env FOO=1 jq '.x'"), Some("jq".into()));
        assert_eq!(derive_binary("RUST_LOG=debug cargo test"), Some("cargo".into()));
        assert_eq!(derive_binary("/usr/local/bin/htop"), Some("htop".into()));
    }

    #[test]
    fn builtins_and_empty_have_no_app() {
        assert_eq!(derive_binary("cd /tmp"), None);
        assert_eq!(derive_binary("export PATH=/x:$PATH"), None);
        assert_eq!(derive_binary(""), None);
        assert_eq!(derive_binary("sudo"), None);
    }

    #[test]
    fn registry_parses_and_contains_seed() {
        let reg = registry();
        assert!(reg.contains_key("rg"));
        assert_eq!(reg["rg"].name, "ripgrep");
        assert_eq!(reg["rg"].install.brew.as_deref(), Some("brew install ripgrep"));
    }

    #[test]
    fn scan_finds_executables_and_misses_absent() {
        let dir = std::env::temp_dir().join(format!("col-apps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("fakeapp");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let plain = dir.join("notexec");
        std::fs::write(&plain, "x").unwrap();

        let path_var = dir.to_str().unwrap().to_string();
        let names = ["fakeapp", "notexec", "missingapp"];
        let avail = Availability::scan(names.iter().copied(), &path_var);
        assert!(avail.available(Some("fakeapp")));
        assert!(!avail.available(Some("notexec")), "exec bit required");
        assert!(!avail.available(Some("missingapp")));
        assert!(avail.available(None), "no app is always available");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unscanned_binary_defaults_to_available() {
        let avail = Availability::scan(std::iter::empty(), "/nonexistent-path-dir");
        assert!(avail.available(Some("never-scanned")), "never gray falsely");
    }

    #[test]
    fn install_for_platform_picks_current_os() {
        let app = registry().get("rg").unwrap();
        let got = install_for_platform(app);
        #[cfg(target_os = "macos")]
        assert_eq!(got, Some("brew install ripgrep"));
        #[cfg(target_os = "linux")]
        assert_eq!(got, Some("apt install ripgrep"));
    }

    #[test]
    fn entry_binary_prefers_explicit_field() {
        assert_eq!(entry_binary(Some("delta"), "git diff"), Some("delta".into()));
        assert_eq!(entry_binary(None, "rg --files"), Some("rg".into()));
        assert_eq!(entry_binary(None, "cd /tmp"), None);
    }
}
