#![allow(clippy::unwrap_used)]

use std::process::{Command, Output};

fn mytool(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mytool"))
        .args(args)
        .env_remove("AITEST_DB")
        .env_remove("RUST_LOG")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn help_lists_the_command_groups_and_global_options() {
    let out = mytool(&["--help"]);
    assert!(out.status.success());
    let help = stdout(&out);
    for expected in [
        "dag",
        "run",
        "--db <DB>",
        "--log-level <LOG_LEVEL>",
        "[env: AITEST_DB=]",
    ] {
        assert!(help.contains(expected), "missing {expected:?} in:\n{help}");
    }
}

#[test]
fn dag_help_lists_its_subcommands() {
    let out = mytool(&["dag", "--help"]);
    assert!(out.status.success());
    let help = stdout(&out);
    for sub in ["validate", "save", "activate", "list"] {
        assert!(help.contains(sub), "missing {sub:?} in:\n{help}");
    }
}

#[test]
fn run_help_lists_its_options() {
    let help = stdout(&mytool(&["run", "--help"]));
    for opt in [
        "--dag <DAG>",
        "--env <ENV>",
        "--no-llm",
        "--report-json <REPORT_JSON>",
    ] {
        assert!(help.contains(opt), "missing {opt:?} in:\n{help}");
    }
}

#[test]
fn version_is_the_crate_version() {
    let out = mytool(&["--version"]);
    assert!(out.status.success());
    assert_eq!(
        stdout(&out).trim(),
        format!("mytool {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn usage_errors_exit_with_2() {
    for args in [
        &["frobnicate"][..],
        &["--log-level", "loud", "dag", "list"],
        &["run", "--env", "local"],
        &[],
    ] {
        let out = mytool(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
    }
}

#[test]
fn global_options_are_accepted_after_the_subcommand() {
    let out = mytool(&["dag", "list", "--db", "x.db", "--log-level", "debug"]);
    assert!(
        !stderr(&out).contains("unexpected argument"),
        "{}",
        stderr(&out)
    );
}
