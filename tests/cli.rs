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
        .arg("sim")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not implemented yet"));
}
