//! Plays N seeded games between the decks of an embedded data set and prints the first-player
//! win rate. Usage: `duel-simulator [games] [--set <name>]` (1000 games of `fixture` by
//! default). The games themselves are played by the `duel_simulator` library (policy: uniformly
//! random among the legal actions other than `Concede`). The real simulator (`run_batch`,
//! reports) comes with the simulator story.
//!
//! The sets are compiled into the binary (`include_str!`): `--set` picks from this closed list,
//! never a path, so nothing is read from the disk or the network at run time.

use duel_engine::{GameSetup, PlayerId};
use duel_simulator::{play_with, setup_from};

/// A data set embedded at compile time. The first deck of `decks` is One's, the second Two's.
#[derive(Debug, PartialEq, Eq)]
struct Set {
    name: &'static str,
    version: &'static str,
    cards: &'static str,
    decks: &'static str,
}

/// The closed list of sets `--set` can choose from.
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

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
struct Config {
    games: u64,
    set: &'static Set,
}

/// The names `--set` accepts, for messages.
fn available_sets() -> String {
    SETS.map(|set| set.name).join(", ")
}

/// Reads `[games] [--set <name>]`, in any order. Pure: the caller prints the error and exits.
fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut games = None;
    let mut set = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--set" {
            let name = rest
                .next()
                .ok_or_else(|| format!("--set needs a value (available: {})", available_sets()))?;
            let found = SETS.iter().find(|set| set.name == name).ok_or_else(|| {
                format!("unknown set \"{name}\" (available: {})", available_sets())
            })?;
            if set.replace(found).is_some() {
                return Err("--set given twice".to_owned());
            }
        } else if arg.starts_with('-') {
            return Err(format!(
                "unknown flag \"{arg}\" (usage: [games] [--set <name>])"
            ));
        } else {
            let count: u64 = arg
                .parse()
                .map_err(|_| format!("invalid number of games \"{arg}\""))?;
            if games.replace(count).is_some() {
                return Err(format!(
                    "the number of games is given twice (again: \"{arg}\")"
                ));
            }
        }
    }
    Ok(Config {
        games: games.unwrap_or(DEFAULT_GAMES),
        set: set.unwrap_or(&SETS[0]),
    })
}

/// The line printed for `one_wins` victories of player one. The fixture keeps the historical
/// form; any other set names itself and its version (provisional form, to revisit with S4).
fn report(config: &Config, one_wins: u64) -> String {
    let games = config.games;
    let rate = 100.0 * one_wins as f64 / games as f64;
    if config.set.name == "fixture" {
        format!("{games} games: player one wins {rate:.1}%")
    } else {
        let Set { name, version, .. } = config.set;
        format!("{games} games [{name}@{version}]: player one wins {rate:.1}%")
    }
}

/// The decks of `set` as a game setup, recording `<name>@<version>` as its data version.
fn setup_of(set: &Set) -> GameSetup {
    setup_from(
        set.cards,
        set.decks,
        vec![format!("{}@{}", set.name, set.version)],
    )
    .expect("the embedded data is valid")
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
    let one = (0..config.games)
        .filter(|&seed| play_with(setup_of(config.set), seed).winner == PlayerId::One)
        .count() as u64;
    println!("{}", report(&config, one));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    fn parsed(list: &[&str]) -> (u64, &'static str) {
        let config = parse_args(&args(list)).expect("valid arguments");
        (config.games, config.set.name)
    }

    #[test]
    fn no_argument_means_1000_games_of_the_fixture() {
        assert_eq!(parsed(&[]), (1000, "fixture"));
    }

    #[test]
    fn a_number_sets_the_games() {
        assert_eq!(parsed(&["500"]), (500, "fixture"));
    }

    #[test]
    fn set_picks_the_embedded_set() {
        assert_eq!(parsed(&["--set", "starter"]), (1000, "starter"));
        assert_eq!(parsed(&["--set", "fixture"]), (1000, "fixture"));
    }

    #[test]
    fn the_games_and_the_set_come_in_any_order() {
        assert_eq!(parsed(&["--set", "starter", "50"]), (50, "starter"));
        assert_eq!(parsed(&["50", "--set", "starter"]), (50, "starter"));
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

    fn config_of(set: &str) -> Config {
        parse_args(&args(&["200", "--set", set])).expect("valid arguments")
    }

    #[test]
    fn the_fixture_report_keeps_its_historical_form() {
        assert_eq!(
            report(&config_of("fixture"), 17),
            "200 games: player one wins 8.5%"
        );
    }

    #[test]
    fn the_report_of_another_set_names_its_version() {
        assert_eq!(
            report(&config_of("starter"), 17),
            "200 games [starter@1.0.0]: player one wins 8.5%"
        );
    }
}
