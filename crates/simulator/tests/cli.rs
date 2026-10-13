//! Runs the real binary: the report's structure and determinism (never a win rate), and a bad
//! command line is a message on stderr and exit code 2, never a panic.

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

/// The matchup lines of a report: the three after the header and its rule.
fn matchup_lines(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.len() >= 3, "{text:?}");
    lines[3..].to_vec()
}

#[test]
fn a_run_prints_the_report_structure_for_the_three_starter_matchups() {
    let output = run(&["20"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = stdout(&output);
    assert!(
        text.starts_with("set: starter@1.0.0 | seed: 0 | games per matchup: 20 | action cap: "),
        "{text:?}"
    );
    assert!(text.ends_with('\n'));
    assert!(
        text.lines()
            .nth(1)
            .is_some_and(|l| l.starts_with("matchup"))
    );
    let lines = matchup_lines(&text);
    assert_eq!(lines.len(), 3, "{text:?}");
    for (line, name) in lines
        .iter()
        .zip(["rush vs bulwark", "rush (mirror)", "bulwark (mirror)"])
    {
        let rest = line
            .strip_prefix(name)
            .unwrap_or_else(|| panic!("{line:?}"));
        // N is the number of games per matchup, in each line.
        assert_eq!(rest.split_whitespace().next(), Some("20"), "{line:?}");
        assert!(line.contains('[') && line.contains(']'), "{line:?}");
    }
}

#[test]
fn the_same_command_prints_the_same_report_and_another_seed_another() {
    let again = (stdout(&run(&["20"])), stdout(&run(&["20"])));
    assert_eq!(again.0, again.1);
    let seeded = |seed: &str| stdout(&run(&["20", "--seed", seed]));
    assert_eq!(seeded("1"), seeded("1"));
    assert_ne!(seeded("1"), seeded("2"));
    assert!(seeded("1").contains("seed: 1 |"));
}

#[test]
fn each_set_names_its_version() {
    let fixture = stdout(&run(&["10", "--set", "fixture"]));
    assert!(fixture.starts_with("set: fixture@1.0.0 | "), "{fixture:?}");
    assert_eq!(matchup_lines(&fixture).len(), 3);
    let starter = stdout(&run(&["10", "--set", "starter"]));
    assert!(starter.starts_with("set: starter@1.0.0 | "), "{starter:?}");
    let exact = stdout(&run(&["10", "--set", "starter@1.0.0"]));
    assert_eq!(starter, exact);
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
    for args in [
        &["--bogus"][..],
        &["abc"],
        &["--set"],
        &["10", "20"],
        &["--seed", "x"],
        &["--seed"],
        &["0"],
        &["--set", "starter@9.9.9"],
        &["--seed", "1", "--seed", "2"],
    ] {
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
