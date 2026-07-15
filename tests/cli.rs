use assert_cmd::Command;
use predicates::str;

#[test]
fn search_sleep_returns_pmset_entry() {
    Command::cargo_bin("col")
        .unwrap()
        .args(["search", "sleep"])
        .assert()
        .success()
        .stdout(str::contains("pmset-disable-sleep"));
}

#[test]
fn show_prints_cmd_undo_and_source() {
    Command::cargo_bin("col")
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
    Command::cargo_bin("col")
        .unwrap()
        .args(["show", "nope-nope"])
        .assert()
        .failure()
        .stderr(str::contains("col search"));
}

#[test]
fn random_prints_an_entry() {
    Command::cargo_bin("col")
        .unwrap()
        .arg("random")
        .assert()
        .success()
        .stdout(str::contains("source: "));
}

#[test]
fn drill_with_empty_stdin_exits_cleanly() {
    Command::cargo_bin("col")
        .unwrap()
        .args(["drill", "--domain", "git"])
        .write_stdin("")
        .assert()
        .success();
}
