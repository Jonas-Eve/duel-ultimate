//! Runs the real binary: the default output is pinned byte for byte, and a bad command line
//! is a message on stderr and exit code 2, never a panic.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_duel-simulator"))
        .args(args)
        .output()
        .expect("the simulator runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("utf-8 output")
}

#[test]
fn the_default_set_prints_the_historical_line() {
    let output = run(&["1000"]);
    assert!(output.status.success());
    assert_eq!(stdout(&output), "1000 games: player one wins 8.5%\n");
}

#[test]
fn naming_the_fixture_changes_nothing() {
    let output = run(&["1000", "--set", "fixture"]);
    assert!(output.status.success());
    assert_eq!(stdout(&output), "1000 games: player one wins 8.5%\n");
}

#[test]
fn the_starter_set_prints_its_version() {
    let output = run(&["20", "--set", "starter"]);
    assert!(output.status.success());
    let line = stdout(&output);
    assert!(
        line.starts_with("20 games [starter@1.0.0]: player one wins ") && line.ends_with("%\n"),
        "{line:?}"
    );
}

#[test]
fn an_unknown_set_exits_with_code_2_and_a_message() {
    let output = run(&["--set", "nope"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let message = String::from_utf8(output.stderr).expect("utf-8 message");
    assert!(message.contains("nope"), "{message}");
}

#[test]
fn a_bad_command_line_never_panics() {
    for args in [&["--bogus"][..], &["abc"], &["--set"], &["10", "20"]] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let message = String::from_utf8(output.stderr).expect("utf-8 message");
        assert!(
            !message.is_empty() && !message.contains("panicked"),
            "{message}"
        );
    }
}
