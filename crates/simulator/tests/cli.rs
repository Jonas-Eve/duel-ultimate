//! Runs the real binary: the report's structure and determinism (never a win rate), and a bad
//! command line is a message on stderr and exit code 2, never a panic.

use duel_engine::RuleSet;
use duel_simulator::rules_label;
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

/// The matchup lines of the first table of the first policy block: the three after the header
/// of the table and its rule.
fn matchup_lines(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    let block = lines
        .iter()
        .position(|line| line.starts_with("policy: "))
        .unwrap_or_else(|| panic!("a policy block in {text:?}"));
    assert!(lines.len() >= block + 6, "{text:?}");
    lines[block + 3..block + 6].to_vec()
}

#[test]
fn a_run_prints_the_report_structure_for_the_three_starter_matchups() {
    let output = run(&["20"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = stdout(&output);
    assert!(
        text.starts_with("set: starter@1.1.0 | seed: 0 | games per matchup: 20 | action cap: "),
        "{text:?}"
    );
    assert!(text.ends_with('\n'));
    assert_eq!(text.lines().nth(1), Some("policy: random"));
    assert!(
        text.lines()
            .nth(2)
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
fn a_run_prints_one_policy_block_with_two_tables_and_a_summary() {
    let output = run(&["20"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = stdout(&output);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.iter().filter(|l| l.starts_with("policy: ")).count(),
        1
    );
    // Two tables: the games and the durations.
    let heads: Vec<&&str> = lines.iter().filter(|l| l.starts_with("matchup ")).collect();
    assert_eq!(heads.len(), 2, "{text:?}");
    assert!(heads[0].contains("seat One") && heads[0].contains("interrupted"));
    assert!(
        heads[1].contains("median") && heads[1].contains("p90") && heads[1].contains("vs target")
    );
    // The five verdict lines close the block, in this order, informational whatever they say.
    let verdict: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| {
            [
                "worst pair:",
                "dispersion (",
                "cross pairs in band ",
                "mirror first player ",
                "duration target:",
            ]
            .iter()
            .any(|prefix| l.starts_with(prefix))
        })
        .collect();
    assert_eq!(verdict.len(), 5, "{text:?}");
    assert_eq!(&lines[lines.len() - 5..], verdict.as_slice());
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
    assert!(starter.starts_with("set: starter@1.1.0 | "), "{starter:?}");
    let exact = stdout(&run(&["10", "--set", "starter@1.1.0"]));
    assert_eq!(starter, exact);
    let older = stdout(&run(&["10", "--set", "starter@1.0.0"]));
    assert!(older.starts_with("set: starter@1.0.0 | "), "{older:?}");
}

#[test]
fn starter_1_0_0_and_1_1_0_give_the_same_report_but_for_the_first_line() {
    let rest = |set: &str| {
        let text = stdout(&run(&["20", "--set", set, "--seed", "5"]));
        text.split_once('\n').expect("a first line").1.to_owned()
    };
    assert_eq!(rest("starter@1.0.0"), rest("starter@1.1.0"));
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
        &["--hero-riposte"],
        &["--hero-riposte", "ON"],
        &["--hero-riposte", "yes"],
        &["--hero-riposte", "1"],
        &["--hero-riposte", "on", "--hero-riposte", "off"],
        &["--board-limit"],
        &["--board-limit", "0"],
        &["--board-limit", "31"],
        &["--board-limit", "-1"],
        &["--board-limit", "abc"],
        &["--board-limit", "1.5"],
        &["--board-limit", "99999999999"],
        &["--board-limit", "5", "--board-limit", "none"],
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

/// The first line of a report.
fn header(args: &[&str]) -> String {
    let output = run(args);
    assert!(output.status.success(), "{args:?}");
    stdout(&output)
        .lines()
        .next()
        .expect("a header line")
        .to_owned()
}

#[test]
fn the_header_names_the_default_rules_of_the_engine() {
    assert_eq!(
        header(&["20"]),
        format!(
            "set: starter@1.1.0 | seed: 0 | games per matchup: 20 | action cap: 10000 | policies: random | rules: {}",
            rules_label(&RuleSet::default())
        )
    );
}

#[test]
fn the_header_names_the_rules_given_on_the_command_line() {
    let args = ["20", "--hero-riposte", "on", "--board-limit", "5"];
    assert_eq!(
        header(&args),
        "set: starter@1.1.0 | seed: 0 | games per matchup: 20 | action cap: 10000 | policies: random | rules: hero_riposte=on board_limit=5"
    );
    assert_eq!(stdout(&run(&args)), stdout(&run(&args)));
}

#[test]
fn a_rule_not_given_keeps_the_engine_default() {
    let default = RuleSet::default();
    assert_eq!(
        header(&["20", "--hero-riposte", "on"])
            .rsplit_once("rules: ")
            .map(|(_, rules)| rules.to_owned()),
        Some(rules_label(&RuleSet {
            hero_riposte: true,
            ..default
        }))
    );
    assert_eq!(
        header(&["20", "--board-limit", "7"])
            .rsplit_once("rules: ")
            .map(|(_, rules)| rules.to_owned()),
        Some(rules_label(&RuleSet {
            board_limit: Some(7),
            ..default
        }))
    );
}

/// The options change nothing yet: this module is deleted or rewritten on purpose by #127,
/// which gives them their behaviour. Until then a batch with options plays the default batch.
mod while_inert {
    use super::*;

    #[test]
    fn the_matchup_lines_of_a_batch_with_options_are_those_of_the_default_batch() {
        for set in ["starter", "fixture"] {
            let base = stdout(&run(&["30", "--set", set, "--seed", "4"]));
            let optioned = stdout(&run(&[
                "30",
                "--set",
                set,
                "--seed",
                "4",
                "--hero-riposte",
                "on",
                "--board-limit",
                "5",
            ]));
            assert_eq!(matchup_lines(&base), matchup_lines(&optioned), "{set}");
        }
    }
}
