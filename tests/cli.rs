use assert_cmd::Command;
use predicates::str;

#[test]
fn search_sleep_returns_pmset_entry() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "sleep"])
        .assert()
        .success()
        .stdout(str::contains("pmset-disable-sleep"));
}

#[test]
fn show_prints_cmd_undo_and_source() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["show", "pmset-disable-sleep"])
        .assert()
        .success()
        .stdout(str::contains("sudo pmset -a disablesleep 1"))
        .stdout(str::contains("undo: sudo pmset -a disablesleep 0"))
        .stdout(str::contains("source: "));
}

#[test]
fn show_unknown_id_fails_with_hint() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["show", "nope-nope"])
        .assert()
        .failure()
        .stderr(str::contains("collective search"));
}

#[test]
fn random_prints_an_entry() {
    Command::cargo_bin("collective")
        .unwrap()
        .arg("random")
        .assert()
        .success()
        .stdout(str::contains("source: "));
}

#[test]
fn drill_with_empty_stdin_exits_cleanly() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["drill", "--domain", "git"])
        .write_stdin("")
        .assert()
        .success();
}

#[test]
fn print_shell_zsh_emits_wrapper() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["--print-shell", "zsh"])
        .assert()
        .success()
        .stdout(str::contains("collective()"))
        .stdout(str::contains("print -z"));
}

#[test]
fn print_shell_bash_emits_wrapper() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["--print-shell", "bash"])
        .assert()
        .success()
        .stdout(str::contains("READLINE_LINE"));
}

#[test]
fn collect_manual_writes_overlay_file() {
    let home = std::env::temp_dir().join(format!("col-collect-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    // manual answers piped in field order: title, explanation, domains, danger, tags, undo, platform
    let stdin = "My Test Cmd\nDoes a thing.\nshell\nlow\nfoo,bar\n\nmacos\n";
    Command::cargo_bin("collective")
        .unwrap()
        .args(["collect", "echo hello", "--manual"])
        .env("HOME", &home)
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(str::contains("saved my-test-cmd"));
    let f = home.join(".collective/corpus/my-test-cmd.yaml");
    assert!(f.exists(), "overlay file not written");
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(text.contains("cmd: echo hello"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn search_curated_excludes_tldr_imports() {
    let out = Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "git", "--curated"])
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("tldr-"), "curated search leaked a tldr import:\n{stdout}");
}

#[test]
fn search_domain_filters_to_domain() {
    // "network" domain entries exist among curated gems (e.g. flush-dns-cache).
    Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "dns", "--domain", "network"])
        .assert()
        .success();
}

#[test]
fn collect_last_reads_env_and_writes_overlay() {
    let home = std::env::temp_dir().join(format!("col-last-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    // manual answers: title, explanation, domains, danger, tags, undo, platform
    let stdin = "Grab Last\nCaptured from history.\nshell\nlow\nhistory\n\nmacos\n";
    Command::cargo_bin("collective")
        .unwrap()
        .args(["collect", "--last", "--manual"])
        .env("HOME", &home)
        .env("COLLECTIVE_LAST_CMD", "echo captured")
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(str::contains("saved grab-last"));
    let f = home.join(".collective/corpus/grab-last.yaml");
    assert!(f.exists());
    assert!(std::fs::read_to_string(&f).unwrap().contains("cmd: echo captured"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn collect_last_without_env_errors() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["collect", "--last"])
        .env_remove("COLLECTIVE_LAST_CMD")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(str::contains("--last needs the shell wrapper"));
}
