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
        &["--timing", "--timing"],
        &["--against"],
        &["--against", "nope"],
        &["--against", "fixture", "--against", "fixture"],
        &["--against", "starter@9.9.9"],
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

#[test]
fn timing_prints_one_line_on_stderr_and_leaves_stdout_unchanged() {
    let plain = run(&["--seed", "5", "20"]);
    let timed = run(&["--seed", "5", "20", "--timing"]);
    assert!(plain.status.success() && timed.status.success());
    assert_eq!(timed.status.code(), Some(0));
    assert_eq!(
        timed.stdout, plain.stdout,
        "the duration never enters the report"
    );
    assert!(
        plain.stderr.is_empty(),
        "no clock, no line without the option"
    );
    let line = String::from_utf8(timed.stderr).expect("utf-8 line");
    assert_eq!(line.lines().count(), 1, "{line:?}");
    assert!(line.ends_with('\n'), "{line:?}");
    assert!(line.starts_with("timing: "), "{line:?}");
    // 3 matchups x 20 games.
    assert!(line.starts_with("timing: 60 games, "), "{line:?}");
    for part in [" games, ", " actions, ", " s, ", " games/s, ", " actions/s"] {
        assert!(line.contains(part), "{part:?} in {line:?}");
    }
}

#[test]
fn against_prints_the_diff_alone_and_is_deterministic() {
    let args = [
        "20",
        "--set",
        "starter@1.1.0",
        "--against",
        "starter@1.0.0",
        "--seed",
        "5",
    ];
    let output = run(&args);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = stdout(&output);
    assert!(
        text.starts_with("diff: starter@1.0.0 -> starter@1.1.0 | "),
        "{text:?}"
    );
    // The diff alone: no report header, no report table.
    assert!(!text.contains("set: "), "{text:?}");
    assert!(!text.contains("seat One"), "{text:?}");
    assert_eq!(text, stdout(&run(&args)));
    // The two versions are identical data and the same seed: nothing moved.
    for line in text.lines().filter(|l| l.contains("rush")) {
        if line.contains(" = ") {
            assert!(line.contains(" 0.0"), "{line}");
            assert!(!line.contains("yes"), "{line}");
        }
    }
}

#[test]
fn against_the_same_set_has_only_zero_deltas() {
    let output = run(&[
        "20",
        "--set",
        "fixture",
        "--against",
        "fixture",
        "--seed",
        "3",
    ]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(
        text.starts_with("diff: fixture@1.0.0 -> fixture@1.0.0 | "),
        "{text:?}"
    );
    assert!(!text.contains("NOTE"), "{text:?}");
    assert!(
        !text
            .lines()
            .filter(|l| !l.starts_with("significant:"))
            .any(|l| l.split_whitespace().any(|c| c == "yes")),
        "{text:?}"
    );
    assert!(!text.contains('+'), "{text:?}");
    assert!(
        !text.contains("new") && !text.contains("removed"),
        "{text:?}"
    );
}

#[test]
fn against_without_set_compares_with_the_default_set() {
    let text = stdout(&run(&["10", "--against", "starter@1.0.0"]));
    assert!(
        text.starts_with("diff: starter@1.0.0 -> starter@1.1.0 | "),
        "{text:?}"
    );
}

#[test]
fn against_unknown_exits_with_code_2() {
    let output = run(&["10", "--against", "nope"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let message = String::from_utf8(output.stderr).expect("utf-8 message");
    assert!(message.contains("unknown set \"nope\""), "{message}");
}

#[test]
fn against_never_reads_a_path() {
    for path in ["../data/starter/1.0.0", "/etc/passwd", "."] {
        let output = run(&["10", "--against", path]);
        assert_eq!(output.status.code(), Some(2), "{path}");
        assert!(output.stdout.is_empty(), "{path}");
        let message = String::from_utf8(output.stderr).expect("utf-8 message");
        assert!(message.contains("unknown set"), "{message}");
        assert!(message.contains(path), "{message}");
    }
}

// --- #160: `--policy` and `--noise` ---

/// The lines of the section of policy `name`: from its `policy: <name>` line to the next
/// `policy: ` line (or the end).
fn section(text: &str, name: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| *line == format!("policy: {name}"))
        .unwrap_or_else(|| panic!("a section {name} in {text:?}"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with("policy: "))
        .map_or(lines.len(), |offset| start + 1 + offset);
    lines[start..end].join("\n").trim_end().to_owned()
}

fn section_names(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| line.strip_prefix("policy: "))
        .collect()
}

#[test]
fn policy_flag_default_is_random_byte_for_byte() {
    let plain = run(&["20"]);
    let explicit = run(&["20", "--policy", "random"]);
    assert!(plain.status.success() && explicit.status.success());
    assert_eq!(explicit.stdout, plain.stdout);
    assert!(explicit.stderr.is_empty());
}

#[test]
fn policy_greedy_prints_a_greedy_section_and_is_deterministic() {
    let args = ["10", "--seed", "3", "--policy", "greedy"];
    let output = run(&args);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let text = stdout(&output);
    assert_eq!(output.stdout, run(&args).stdout);
    assert!(text.contains("policies: greedy"), "{text:?}");
    assert_eq!(section_names(&text), ["greedy"]);
    assert_eq!(matchup_lines(&text).len(), 3, "{text:?}");
}

#[test]
fn policy_list_prints_one_section_per_policy_in_the_given_order() {
    let both = stdout(&run(&["10", "--seed", "4", "--policy", "greedy,random"]));
    assert_eq!(section_names(&both), ["greedy", "random"]);
    assert!(both.contains("policies: greedy, random"), "{both:?}");
    let random_alone = stdout(&run(&["10", "--seed", "4", "--policy", "random"]));
    let greedy_alone = stdout(&run(&["10", "--seed", "4", "--policy", "greedy"]));
    assert_eq!(section(&both, "random"), section(&random_alone, "random"));
    assert_eq!(section(&both, "greedy"), section(&greedy_alone, "greedy"));
}

#[test]
fn noise_wraps_greedy_and_names_the_section() {
    let args = ["10", "--seed", "2", "--policy", "greedy", "--noise", "20"];
    let output = run(&args);
    assert_eq!(output.status.code(), Some(0));
    let text = stdout(&output);
    assert_eq!(section_names(&text), ["greedy-noise20"]);
    assert!(text.contains("policies: greedy-noise20"), "{text:?}");
    assert_eq!(output.stdout, run(&args).stdout);

    let zero = run(&["10", "--seed", "2", "--policy", "greedy", "--noise", "0"]);
    let plain = run(&["10", "--seed", "2", "--policy", "greedy"]);
    assert_eq!(zero.stdout, plain.stdout);

    let full = run(&["10", "--seed", "2", "--policy", "greedy", "--noise", "100"]);
    assert_eq!(full.status.code(), Some(0));
    assert_eq!(section_names(&stdout(&full)), ["greedy-noise100"]);
}

#[test]
fn noise_never_wraps_random() {
    let text = stdout(&run(&[
        "10",
        "--seed",
        "2",
        "--policy",
        "random,greedy",
        "--noise",
        "30",
    ]));
    assert_eq!(section_names(&text), ["random", "greedy-noise30"]);
    let random_alone = stdout(&run(&["10", "--seed", "2", "--policy", "random"]));
    assert_eq!(section(&text, "random"), section(&random_alone, "random"));
}

#[test]
fn bad_policy_arguments_exit_2() {
    let cases: [&[&str]; 13] = [
        &["--policy", "nope"],
        &["--policy", "greedy,greedy"],
        &["--policy", ""],
        &["--policy", "random,"],
        &["--policy"],
        &["--policy", "random", "--policy", "greedy"],
        &["--noise", "101"],
        &["--noise", "-1"],
        &["--noise", "1.5"],
        &["--noise", "abc"],
        &["--noise"],
        &["--noise", "5", "--noise", "5"],
        &["--policy", "random", "--noise", "10"],
    ];
    for case in cases {
        let mut args = vec!["5"];
        args.extend_from_slice(case);
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "{case:?}");
        assert!(output.stdout.is_empty(), "{case:?}");
        let stderr = String::from_utf8(output.stderr).expect("utf-8");
        assert!(stderr.starts_with("duel-simulator: "), "{case:?}: {stderr}");
        assert!(!stderr.contains("panicked"), "{case:?}: {stderr}");
    }
    // The unknown name lists the registry.
    let stderr = String::from_utf8(run(&["--policy", "nope"]).stderr).expect("utf-8");
    assert!(
        stderr.contains("random") && stderr.contains("greedy"),
        "{stderr}"
    );
    // `--noise` alone applies to the default `random`: nothing to wrap.
    assert_eq!(run(&["5", "--noise", "10"]).status.code(), Some(2));
}

#[test]
fn greedy_works_with_against_and_timing() {
    let against = stdout(&run(&[
        "6",
        "--set",
        "fixture",
        "--against",
        "fixture",
        "--seed",
        "3",
        "--policy",
        "greedy",
    ]));
    assert!(
        against.starts_with("diff: fixture@1.0.0 -> fixture@1.0.0 | "),
        "{against:?}"
    );
    assert!(!against.contains('+'), "{against:?}");
    let plain = run(&["6", "--seed", "5", "--policy", "greedy"]);
    let timed = run(&["6", "--seed", "5", "--policy", "greedy", "--timing"]);
    assert!(plain.status.success() && timed.status.success());
    assert_eq!(timed.stdout, plain.stdout);
}
