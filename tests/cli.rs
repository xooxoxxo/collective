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

/// Temp HOME seeded with one curated and one bulk-import overlay entry, so
/// grouping behavior is asserted against fixtures rather than corpus contents.
fn grouping_home(tag: &str) -> std::path::PathBuf {
    let home = std::env::temp_dir().join(format!("col-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let dir = home.join(".collective/corpus");
    std::fs::create_dir_all(&dir).unwrap();
    let entry = |id: &str, domain: &str| {
        format!(
            "id: {id}\ntitle: zzmarker widget\ncmd: run {id}\nplatform: [macos]\n\
             domains: [{domain}]\ndanger: low\nexplanation: fixture entry.\nsource: fixture\n"
        )
    };
    std::fs::write(dir.join("zz-curated.yaml"), entry("zz-curated", "shell")).unwrap();
    std::fs::write(
        dir.join("zz-import.yaml"),
        entry("zz-import", "tldr-import"),
    )
    .unwrap();
    home
}

#[test]
fn search_prints_separator_between_groups() {
    let home = grouping_home("sep");
    Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "zzmarker"])
        .env("HOME", &home)
        .assert()
        .success()
        .stdout(str::contains("── tldr imports ──"))
        .stdout(str::contains("zz-curated"))
        .stdout(str::contains("zz-import"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn search_curated_excludes_tldr_imports() {
    let home = grouping_home("cur");
    let out = Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "zzmarker", "--curated"])
        .env("HOME", &home)
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("zz-curated"),
        "curated entry missing:\n{stdout}"
    );
    assert!(
        !stdout.contains("zz-import"),
        "curated search leaked an import:\n{stdout}"
    );
    let _ = std::fs::remove_dir_all(&home);
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
    assert!(std::fs::read_to_string(&f)
        .unwrap()
        .contains("cmd: echo captured"));
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

#[test]
fn completions_zsh_emits_script() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(str::contains("_collective"));
}

#[test]
fn completions_unknown_shell_errors() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["completions", "tcsh"])
        .assert()
        .failure();
}

#[test]
fn search_curated_output_has_no_separator() {
    let home = grouping_home("nosep");
    let out = Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "zzmarker", "--curated"])
        .env("HOME", &home)
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(!stdout.contains("── tldr imports ──"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn pack_list_reports_empty_and_then_the_installed_pack() {
    let home = std::env::temp_dir().join(format!("col-packlist-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let dir = home.join(".collective/packs");
    std::fs::create_dir_all(&dir).unwrap();

    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "list"])
        .env("HOME", &home)
        .assert()
        .success()
        .stdout(str::contains("no packs installed"));

    std::fs::write(
        dir.join("demo.json"),
        r#"{"manifest":{"name":"demo","version":"2.1.0","count":1},"entries":[
            {"id":"demo-entry","title":"Demo","cmd":"echo demo","platform":["macos"],
             "domains":["shell"],"danger":"low","explanation":"e","source":"s"}]}"#,
    )
    .unwrap();

    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "list"])
        .env("HOME", &home)
        .assert()
        .success()
        .stdout(str::contains("demo"))
        .stdout(str::contains("2.1.0"));

    // The installed pack's entries must be searchable.
    Command::cargo_bin("collective")
        .unwrap()
        .args(["search", "demo"])
        .env("HOME", &home)
        .assert()
        .success()
        .stdout(str::contains("demo-entry"));

    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "remove", "demo"])
        .env("HOME", &home)
        .assert()
        .success();
    assert!(!dir.join("demo.json").exists());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn pack_remove_rejects_a_traversing_name() {
    Command::cargo_bin("collective")
        .unwrap()
        .args(["pack", "remove", "../../evil"])
        .assert()
        .failure()
        .stderr(str::contains("bad pack name"));
}
