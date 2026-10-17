//! Plays seeded AI-vs-AI games between the decks of an embedded data set and prints the
//! balance report. Usage: `duel-simulator [games] [--set <name>[@<version>]] [--seed <u64>]
//! [--hero-riposte <on|off>] [--board-limit <none|N>]`, in any order: `games` per matchup (1000
//! by default), the `starter` set (its latest version by default), seed 0 by default, and the
//! engine's default rules for each rule option not given (the report names them in its header).
//! The two rule options have no effect on the games before #127 gives them their behaviour. The batch itself is `duel_simulator::run_batch` (policy:
//! uniformly random among the legal actions other than `Concede`); this file only reads the
//! command line, picks the data and prints `Report::render()`.
//!
//! The sets are compiled into the binary (`include_str!`): `--set` picks from this closed list,
//! never a path, so nothing is read from the disk or the network at run time. A huge number of
//! games is merely slow.

use duel_engine::RuleSet;
use duel_simulator::{BatchConfig, DEFAULT_MAX_ACTIONS, contenders_from, random_policy, run_batch};

/// A data set embedded at compile time.
#[derive(Debug, PartialEq, Eq)]
struct Set {
    name: &'static str,
    version: &'static str,
    cards: &'static str,
    decks: &'static str,
}

/// The closed list of sets `--set` can choose from; for a name, the last entry is its latest
/// version.
const SETS: [Set; 3] = [
    Set {
        name: "fixture",
        version: "1.0.0",
        cards: include_str!("../../../data/fixture/1.0.0/cards.json"),
        decks: include_str!("../../../data/fixture/1.0.0/decks.json"),
    },
    Set {
        name: "starter",
        version: "1.0.0",
        cards: include_str!("../../../data/starter/1.0.0/cards.json"),
        decks: include_str!("../../../data/starter/1.0.0/decks.json"),
    },
    Set {
        name: "starter",
        version: "1.1.0",
        cards: include_str!("../../../data/starter/1.1.0/cards.json"),
        decks: include_str!("../../../data/starter/1.1.0/decks.json"),
    },
];

const DEFAULT_GAMES: u64 = 1000;
const DEFAULT_SEED: u64 = 0;
const DEFAULT_SET: &str = "starter";
const USAGE: &str = "[games] [--set <name>[@<version>]] [--seed <u64>] [--hero-riposte <on|off>] [--board-limit <none|N>]";

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
struct Config {
    /// Games per matchup.
    games: u64,
    seed: u64,
    set: &'static Set,
    rules: RuleSet,
}

/// The distinct set names, in registry order, for messages.
fn available_sets(sets: &[Set]) -> String {
    let mut names: Vec<&str> = Vec::new();
    for set in sets {
        if !names.contains(&set.name) {
            names.push(set.name);
        }
    }
    names.join(", ")
}

/// The set `spec` (`name`, its latest version, or `name@version`) names in `sets`. Pure:
/// nothing is read.
fn resolve_set<'a>(sets: &'a [Set], spec: &str) -> Result<&'a Set, String> {
    let (name, version) = match spec.split_once('@') {
        Some((name, version)) => (name, Some(version)),
        None => (spec, None),
    };
    let of_name = || sets.iter().filter(move |set| set.name == name);
    let found = match version {
        None => of_name().next_back(),
        Some(version) => of_name().find(|set| set.version == version),
    };
    found.ok_or_else(|| {
        if of_name().next().is_some() {
            let versions: Vec<&str> = of_name().map(|set| set.version).collect();
            format!(
                "unknown version of set \"{name}\" in \"{spec}\" (available: {})",
                versions.join(", ")
            )
        } else {
            format!(
                "unknown set \"{spec}\" (available: {})",
                available_sets(sets)
            )
        }
    })
}

/// The value of `--hero-riposte`: exactly `on` or `off`.
fn parse_hero_riposte(value: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(format!(
            "--hero-riposte: invalid value \"{value}\" (expected on or off)"
        )),
    }
}

/// The value of `--board-limit`: `none`, or an integer whose bounds are those of
/// [`RuleSet::validate`] (the only place they are written).
fn parse_board_limit(value: &str) -> Result<Option<u32>, String> {
    if value == "none" {
        return Ok(None);
    }
    let limit: u32 = value.parse().map_err(|_| {
        format!("--board-limit: invalid value \"{value}\" (expected none or a positive integer)")
    })?;
    let rules = RuleSet {
        board_limit: Some(limit),
        ..RuleSet::default()
    };
    rules
        .validate()
        .map_err(|error| format!("--board-limit: {error}"))?;
    Ok(Some(limit))
}

/// Reads `[games] [--set <name>[@<version>]] [--seed <u64>] [--hero-riposte <on|off>]
/// [--board-limit <none|N>]`, in any order. Pure: the caller
/// prints the error and exits.
fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut games = None;
    let mut seed = None;
    let mut set = None;
    let mut hero_riposte = None;
    let mut board_limit = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--set" {
            let spec = rest.next().ok_or_else(|| {
                format!("--set needs a value (available: {})", available_sets(&SETS))
            })?;
            if set.replace(resolve_set(&SETS, spec)?).is_some() {
                return Err("--set given twice".to_owned());
            }
        } else if arg == "--seed" {
            let value = rest
                .next()
                .ok_or_else(|| "--seed needs a value (a non-negative integer)".to_owned())?;
            let parsed: u64 = value
                .parse()
                .map_err(|_| format!("invalid seed \"{value}\""))?;
            if seed.replace(parsed).is_some() {
                return Err("--seed given twice".to_owned());
            }
        } else if arg == "--hero-riposte" {
            let value = rest
                .next()
                .ok_or_else(|| "--hero-riposte needs a value (on or off)".to_owned())?;
            if hero_riposte.replace(parse_hero_riposte(value)?).is_some() {
                return Err("--hero-riposte given twice".to_owned());
            }
        } else if arg == "--board-limit" {
            let value = rest.next().ok_or_else(|| {
                "--board-limit needs a value (none or a positive integer)".to_owned()
            })?;
            if board_limit.replace(parse_board_limit(value)?).is_some() {
                return Err("--board-limit given twice".to_owned());
            }
        } else if arg.starts_with('-') {
            return Err(format!("unknown flag \"{arg}\" (usage: {USAGE})"));
        } else {
            let count: u64 = arg
                .parse()
                .map_err(|_| format!("invalid number of games \"{arg}\""))?;
            if count == 0 {
                return Err("the number of games must be at least 1".to_owned());
            }
            if games.replace(count).is_some() {
                return Err(format!(
                    "the number of games is given twice (again: \"{arg}\")"
                ));
            }
        }
    }
    let default_set = resolve_set(&SETS, DEFAULT_SET).expect("the default set is registered");
    // A rule not given keeps the engine default (whatever it is), not "off".
    let default_rules = RuleSet::default();
    Ok(Config {
        games: games.unwrap_or(DEFAULT_GAMES),
        seed: seed.unwrap_or(DEFAULT_SEED),
        set: set.unwrap_or(default_set),
        rules: RuleSet {
            hero_riposte: hero_riposte.unwrap_or(default_rules.hero_riposte),
            board_limit: board_limit.unwrap_or(default_rules.board_limit),
        },
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match parse_args(&args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("duel-simulator: {message}");
            std::process::exit(2);
        }
    };
    let contenders =
        contenders_from(config.set.cards, config.set.decks).expect("the embedded data is valid");
    let batch = BatchConfig {
        games: config.games,
        seed: config.seed,
        max_actions: DEFAULT_MAX_ACTIONS,
        data_versions: vec![format!("{}@{}", config.set.name, config.set.version)],
        rules: config.rules,
    };
    print!(
        "{}",
        run_batch(&contenders, &batch, &random_policy).render()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use duel_engine::MAX_BOARD_LIMIT;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    fn parsed(list: &[&str]) -> (u64, u64, String) {
        let config = parse_args(&args(list)).expect("valid arguments");
        (
            config.games,
            config.seed,
            format!("{}@{}", config.set.name, config.set.version),
        )
    }

    fn rules_of(list: &[&str]) -> RuleSet {
        parse_args(&args(list)).expect("valid arguments").rules
    }

    fn parsed_is(list: &[&str], games: u64, seed: u64, set: &str) {
        assert_eq!(parsed(list), (games, seed, set.to_owned()), "{list:?}");
    }

    #[test]
    fn no_argument_means_1000_games_seed_0_of_the_starter_set() {
        parsed_is(&[], 1000, 0, "starter@1.1.0");
    }

    #[test]
    fn a_number_sets_the_games_per_matchup() {
        parsed_is(&["500"], 500, 0, "starter@1.1.0");
    }

    #[test]
    fn set_picks_the_embedded_set_by_name_or_by_name_and_version() {
        parsed_is(&["--set", "fixture"], 1000, 0, "fixture@1.0.0");
        parsed_is(&["--set", "starter"], 1000, 0, "starter@1.1.0");
        parsed_is(&["--set", "starter@1.0.0"], 1000, 0, "starter@1.0.0");
        parsed_is(&["--set", "starter@1.1.0"], 1000, 0, "starter@1.1.0");
    }

    #[test]
    fn seed_reads_a_u64() {
        parsed_is(&["--seed", "42"], 1000, 42, "starter@1.1.0");
        parsed_is(
            &["--seed", "18446744073709551615"],
            1000,
            u64::MAX,
            "starter@1.1.0",
        );
    }

    #[test]
    fn the_games_the_seed_and_the_set_come_in_any_order() {
        let want = (50, 7, "fixture@1.0.0".to_owned());
        for list in [
            &["50", "--seed", "7", "--set", "fixture"][..],
            &["--set", "fixture", "--seed", "7", "50"],
            &["--seed", "7", "50", "--set", "fixture"],
        ] {
            assert_eq!(parsed(list), want, "{list:?}");
        }
    }

    #[test]
    fn rules_not_given_are_the_engine_default() {
        assert_eq!(rules_of(&[]), RuleSet::default());
        assert_eq!(rules_of(&["10", "--seed", "3"]), RuleSet::default());
    }

    #[test]
    fn each_rule_flag_sets_its_option_and_leaves_the_other_at_the_engine_default() {
        let default = RuleSet::default();
        for (value, on) in [("on", true), ("off", false)] {
            assert_eq!(
                rules_of(&["--hero-riposte", value]),
                RuleSet {
                    hero_riposte: on,
                    ..default
                }
            );
        }
        assert_eq!(
            rules_of(&["--board-limit", "none"]),
            RuleSet {
                board_limit: None,
                ..default
            }
        );
        for limit in [1, 5, 30] {
            assert_eq!(
                rules_of(&["--board-limit", &limit.to_string()]),
                RuleSet {
                    board_limit: Some(limit),
                    ..default
                }
            );
        }
    }

    #[test]
    fn the_rule_flags_come_in_any_order_with_the_others() {
        let want = RuleSet {
            hero_riposte: true,
            board_limit: Some(5),
        };
        for list in [
            &[
                "--hero-riposte",
                "on",
                "--board-limit",
                "5",
                "50",
                "--seed",
                "7",
            ][..],
            &[
                "--board-limit",
                "5",
                "--seed",
                "7",
                "--hero-riposte",
                "on",
                "50",
            ],
            &[
                "50",
                "--board-limit",
                "5",
                "--set",
                "fixture",
                "--hero-riposte",
                "on",
            ],
        ] {
            assert_eq!(rules_of(list), want, "{list:?}");
            assert_eq!(parse_args(&args(list)).expect("valid").games, 50);
        }
    }

    #[test]
    fn a_bad_hero_riposte_is_named() {
        for list in [
            &["--hero-riposte"][..],
            &["10", "--hero-riposte"],
            &["--hero-riposte", "ON"],
            &["--hero-riposte", "yes"],
            &["--hero-riposte", "1"],
            &["--hero-riposte", "true"],
        ] {
            assert!(rejected(list).contains("--hero-riposte"), "{list:?}");
        }
        assert!(rejected(&["--hero-riposte", "yes"]).contains("yes"));
    }

    #[test]
    fn a_bad_board_limit_is_named() {
        for value in ["0", "31", "-1", "abc", "1.5", "99999999999", "NONE", ""] {
            let message = rejected(&["--board-limit", value]);
            assert!(message.contains("--board-limit"), "{value}: {message}");
            assert!(message.contains(value), "{value}: {message}");
        }
        assert!(rejected(&["--board-limit"]).contains("--board-limit"));
    }

    #[test]
    fn the_bounds_of_the_board_limit_come_from_the_engine() {
        // The message of `RuleSet::validate` is carried over, not rewritten here.
        let error = RuleSet {
            board_limit: Some(0),
            ..RuleSet::default()
        }
        .validate()
        .expect_err("0 is out of range");
        let message = rejected(&["--board-limit", "0"]);
        assert!(message.contains(&error.to_string()), "{message}");
        assert_eq!(
            rules_of(&["--board-limit", &MAX_BOARD_LIMIT.to_string()]).board_limit,
            Some(MAX_BOARD_LIMIT)
        );
        assert!(
            parse_args(&args(&[
                "--board-limit",
                &(MAX_BOARD_LIMIT + 1).to_string()
            ]))
            .is_err()
        );
    }

    #[test]
    fn a_rule_flag_given_twice_is_rejected() {
        assert!(
            rejected(&["--hero-riposte", "on", "--hero-riposte", "off"])
                .contains("--hero-riposte given twice")
        );
        assert!(
            rejected(&["--board-limit", "5", "--board-limit", "none"])
                .contains("--board-limit given twice")
        );
    }

    fn rejected(list: &[&str]) -> String {
        parse_args(&args(list)).expect_err("invalid arguments")
    }

    fn timing_of(list: &[&str]) -> bool {
        parse_args(&args(list)).expect("valid arguments").timing
    }

    #[test]
    fn timing_is_off_by_default() {
        assert!(!timing_of(&[]));
        assert!(!timing_of(&["10", "--seed", "3"]));
    }

    #[test]
    fn timing_is_a_flag_without_a_value() {
        // The number after it is the games, not a value of the flag.
        assert!(timing_of(&["--timing"]));
        let config = parse_args(&args(&["--timing", "50"])).expect("valid arguments");
        assert!(config.timing);
        assert_eq!(config.games, 50);
    }

    #[test]
    fn timing_comes_in_any_order_with_the_other_options() {
        for list in [
            &["--timing", "50", "--seed", "7", "--set", "fixture"][..],
            &["50", "--seed", "7", "--timing", "--set", "fixture"],
            &["--set", "fixture", "--seed", "7", "50", "--timing"],
            &[
                "--hero-riposte",
                "off",
                "--board-limit",
                "none",
                "--timing",
                "50",
                "--seed",
                "7",
                "--set",
                "fixture",
            ],
        ] {
            let config = parse_args(&args(list)).expect("valid arguments");
            assert!(config.timing, "{list:?}");
            assert_eq!((config.games, config.seed), (50, 7), "{list:?}");
            assert_eq!(config.set.name, "fixture", "{list:?}");
        }
    }

    #[test]
    fn timing_given_twice_is_rejected() {
        let message = rejected(&["--timing", "--timing"]);
        assert!(message.contains("--timing"), "{message}");
        assert!(message.contains("--timing given twice"), "{message}");
    }

    #[test]
    fn the_usage_lists_timing() {
        assert!(USAGE.contains("--timing"));
        assert!(rejected(&["--bogus"]).contains("--timing"));
    }

    #[test]
    fn an_unknown_set_is_named_with_the_available_ones() {
        let message = rejected(&["--set", "nope"]);
        assert!(message.contains("nope"), "{message}");
        assert!(message.contains("fixture"), "{message}");
        assert!(message.contains("starter"), "{message}");
    }

    #[test]
    fn an_unknown_flag_is_named() {
        assert!(rejected(&["--bogus"]).contains("--bogus"));
        assert!(rejected(&["10", "-x"]).contains("-x"));
    }

    #[test]
    fn set_without_a_value_is_rejected() {
        assert!(rejected(&["--set"]).contains("--set"));
        assert!(rejected(&["10", "--set"]).contains("--set"));
    }

    #[test]
    fn an_invalid_number_is_named() {
        assert!(rejected(&["abc"]).contains("abc"));
        assert!(rejected(&["1.5"]).contains("1.5"));
    }

    #[test]
    fn two_numbers_are_rejected() {
        let message = rejected(&["10", "20"]);
        assert!(message.contains("20"), "{message}");
    }

    #[test]
    fn set_given_twice_is_rejected() {
        assert!(rejected(&["--set", "starter", "--set", "fixture"]).contains("--set"));
    }

    #[test]
    fn zero_games_is_rejected() {
        assert!(rejected(&["0"]).contains("at least 1"));
    }

    #[test]
    fn a_bad_seed_is_named() {
        assert!(rejected(&["--seed", "x"]).contains('x'));
        assert!(rejected(&["--seed", "-1"]).contains("-1"));
        assert!(rejected(&["--seed", "18446744073709551616"]).contains("1844"));
        assert!(rejected(&["--seed"]).contains("--seed"));
    }

    #[test]
    fn seed_given_twice_is_rejected() {
        assert!(rejected(&["--seed", "1", "--seed", "2"]).contains("--seed"));
    }

    fn set(name: &'static str, version: &'static str) -> Set {
        Set {
            name,
            version,
            cards: "",
            decks: "",
        }
    }

    fn registry() -> [Set; 3] {
        [set("a", "1.0.0"), set("b", "1.0.0"), set("a", "2.0.0")]
    }

    #[test]
    fn a_name_alone_means_its_last_registered_version() {
        let sets = registry();
        assert_eq!(resolve_set(&sets, "a"), Ok(&sets[2]));
        assert_eq!(resolve_set(&sets, "b"), Ok(&sets[1]));
    }

    #[test]
    fn name_at_version_means_that_exact_version() {
        let sets = registry();
        assert_eq!(resolve_set(&sets, "a@1.0.0"), Ok(&sets[0]));
        assert_eq!(resolve_set(&sets, "a@2.0.0"), Ok(&sets[2]));
    }

    #[test]
    fn an_unknown_version_names_the_available_ones() {
        let message = resolve_set(&registry(), "a@3.0.0").expect_err("unknown version");
        assert!(message.contains("3.0.0"), "{message}");
        assert!(resolve_set(&registry(), "a@").is_err());
        assert!(
            message.contains("1.0.0") && message.contains("2.0.0"),
            "{message}"
        );
    }

    #[test]
    fn an_unknown_name_names_the_available_sets() {
        for spec in ["nope", "nope@1.0.0", "@1.0.0", ""] {
            let message = resolve_set(&registry(), spec).expect_err("unknown");
            assert!(message.contains("(available: a, b)"), "{message}");
        }
    }
}
