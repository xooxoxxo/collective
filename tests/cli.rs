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
