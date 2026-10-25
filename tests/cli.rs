use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn reports_the_crate_version() {
    Command::cargo_bin("wordclock")
        .expect("binary should build")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unimplemented_subcommands_fail_loudly() {
    Command::cargo_bin("wordclock")
        .expect("binary should build")
        .arg("run")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not implemented yet"));
}

#[test]
fn sim_renders_a_legacy_time_with_a_fixed_color() {
    Command::cargo_bin("wordclock")
        .expect("binary should build")
        .args(["sim", "12.11.2017 17:23", "--color", "ff0000"])
        .assert()
        .success()
        .stdout(predicate::str::contains("| "));
}

#[test]
fn sim_rejects_an_invalid_color() {
    Command::cargo_bin("wordclock")
        .expect("binary should build")
        .args(["sim", "12.11.2017 17:23", "--color", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid RRGGBB color"));
}
