//! Plays seeded AI-vs-AI games between the decks of an embedded data set and prints the
//! balance report. Usage: `duel-simulator [games] [--set <name>[@<version>]] [--seed <u64>]`,
//! in any order: `games` per matchup (1000 by default), the `starter` set (its latest version
//! by default) and seed 0 by default. The batch itself is `duel_simulator::run_batch` (policy:
//! uniformly random among the legal actions other than `Concede`); this file only reads the
//! command line, picks the data and prints `Report::render()`.
//!
//! The sets are compiled into the binary (`include_str!`): `--set` picks from this closed list,
//! never a path, so nothing is read from the disk or the network at run time. A huge number of
//! games is merely slow.

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
const SETS: [Set; 2] = [
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
];

const DEFAULT_GAMES: u64 = 1000;
const DEFAULT_SEED: u64 = 0;
const DEFAULT_SET: &str = "starter";
const USAGE: &str = "[games] [--set <name>[@<version>]] [--seed <u64>]";

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
struct Config {
    /// Games per matchup.
    games: u64,
    seed: u64,
    set: &'static Set,
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

/// Reads `[games] [--set <name>[@<version>]] [--seed <u64>]`, in any order. Pure: the caller
/// prints the error and exits.
fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut games = None;
    let mut seed = None;
    let mut set = None;
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
    Ok(Config {
        games: games.unwrap_or(DEFAULT_GAMES),
        seed: seed.unwrap_or(DEFAULT_SEED),
        set: set.unwrap_or(default_set),
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
    };
    print!("{}", run_batch(&contenders, &batch, random_policy).render());
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn rejected(list: &[&str]) -> String {
        parse_args(&args(list)).expect_err("invalid arguments")
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
