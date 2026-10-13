//! Plays N seeded games between the decks of an embedded data set and prints the first-player
//! win rate. Usage: `duel-simulator [games] [--set <name>]` (1000 games of `fixture` by
//! default). Policy: uniformly random among the legal actions other than `Concede` (attacks on
//! heroes and on creatures alike), drawn from the simulator's own generator (separate from the
//! game's). The real simulator (`run_batch`, reports) comes with the simulator story.
//!
//! The sets are compiled into the binary (`include_str!`): `--set` picks from this closed list,
//! never a path, so nothing is read from the disk or the network at run time.

use duel_cards::{parse_cards, parse_decks, resolve_deck};
use duel_engine::{
    Action, Event, GameSetup, PlayerId, PlayerSetup, WinReason, apply, legal_actions, new_game,
};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

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
    let cards = parse_cards(set.cards).expect("the embedded cards are valid");
    let decks = parse_decks(set.decks).expect("the embedded decks are valid");
    let side = |index: usize| {
        let deck = decks.get(index).expect("a set has two decks");
        PlayerSetup {
            hero: deck.hero.clone(),
            cards: resolve_deck(deck, &cards).expect("the embedded decks resolve"),
        }
    };
    GameSetup {
        one: side(0),
        two: side(1),
        data_versions: vec![format!("{}@{}", set.name, set.version)],
    }
}

/// The fixture decks as a game setup.
#[cfg(test)]
fn fixture_setup() -> GameSetup {
    setup_of(&SETS[0])
}

/// Mixed into the game seed so the policy's generator is not the game's.
const POLICY_SALT: u64 = 0x5DEE_CE66_D1CE_4E5B;
/// Defensive cap: every game ends by itself, but a rule bug must not hang the batch.
const MAX_STEPS: usize = 10_000;

/// How a game went: who won, why, and how many creatures died along the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Outcome {
    winner: PlayerId,
    reason: WinReason,
    deaths: usize,
}

/// Plays one game of the fixture decks to its end.
#[cfg(test)]
fn play(seed: u64) -> Outcome {
    play_with(fixture_setup(), seed)
}

/// Plays one game of `setup` to its end.
fn play_with(setup: GameSetup, seed: u64) -> Outcome {
    play_inspected(setup, seed, |_, _| {})
}

/// Plays one game of `setup` to its end, showing `on_step` each step: the actions the policy
/// chose among (never `Concede`) and the events of the action it applied.
fn play_inspected(
    setup: GameSetup,
    seed: u64,
    mut on_step: impl FnMut(&[Action], &[Event]),
) -> Outcome {
    let (mut state, _) = new_game(setup, seed).expect("the setup is valid");
    let mut policy = ChaCha8Rng::seed_from_u64(seed ^ POLICY_SALT);
    let mut deaths = 0;
    for _ in 0..MAX_STEPS {
        if let Some((winner, reason)) = state.winner() {
            return Outcome {
                winner,
                reason,
                deaths,
            };
        }
        let playable: Vec<Action> = legal_actions(&state)
            .into_iter()
            .filter(|action| *action != Action::Concede)
            .collect();
        let action = if playable.is_empty() {
            Action::Concede
        } else {
            playable[policy.random_range(0..playable.len())]
        };
        let events = apply(&mut state, action).expect("a legal action applies");
        on_step(&playable, &events);
        deaths += events
            .iter()
            .filter(|event| matches!(event, Event::Died { .. }))
            .count();
    }
    apply(&mut state, Action::Concede).expect("the game is still running");
    let (winner, reason) = state.winner().expect("a concession ends the game");
    Outcome {
        winner,
        reason,
        deaths,
    }
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
    use duel_engine::Target;

    #[test]
    fn the_fixture_setup_is_valid() {
        let setup = fixture_setup();
        assert_eq!(setup.one.cards.len(), 30);
        assert_eq!(setup.two.cards.len(), 30);
        assert!(duel_engine::new_game(setup, 0).is_ok());
    }

    #[test]
    fn every_game_ends_by_a_hero_going_down_or_exhaustion_never_by_concession() {
        let mut hero_down = 0;
        for seed in 0..200 {
            match play(seed).reason {
                WinReason::HeroDown => hero_down += 1,
                WinReason::Exhaustion => {}
                WinReason::Concede => panic!("seed {seed}: the policy never concedes"),
            }
        }
        assert!(hero_down > 0, "the random policy attacks, so heroes fall");
    }

    #[test]
    fn creatures_die_during_the_batch() {
        let deaths: usize = (0..200).map(|seed| play(seed).deaths).sum();
        assert!(deaths > 0, "the random policy attacks creatures too");
    }

    #[test]
    fn a_seed_always_gives_the_same_game() {
        for seed in 0..20 {
            assert_eq!(play(seed), play(seed));
        }
    }

    /// The fixture with ripostes of 0..=4 and provocation on a third of the cards.
    fn riposting_setup() -> GameSetup {
        let mut setup = fixture_setup();
        for side in [&mut setup.one, &mut setup.two] {
            for (n, card) in side.cards.iter_mut().enumerate() {
                card.riposte = u32::try_from(n % 5).expect("a small index");
                card.provocation = n % 3 == 0;
            }
        }
        setup
    }

    #[test]
    fn games_with_riposte_and_provocation_all_end_by_a_hero_going_down_or_exhaustion() {
        let mut deaths = 0;
        for seed in 0..100 {
            let outcome = play_with(riposting_setup(), seed);
            match outcome.reason {
                WinReason::HeroDown | WinReason::Exhaustion => {}
                WinReason::Concede => panic!("seed {seed}: conceded or hit the step cap"),
            }
            deaths += outcome.deaths;
        }
        assert!(deaths > 0, "creatures die in those games");
    }

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

    fn started_versions(setup: GameSetup) -> Vec<String> {
        let (_, events) = new_game(setup, 0).expect("the setup is valid");
        events
            .into_iter()
            .find_map(|event| match event {
                Event::GameStarted { data_versions, .. } => Some(data_versions),
                _ => None,
            })
            .expect("a game starts with GameStarted")
    }

    fn starter_setup() -> GameSetup {
        let starter = SETS.iter().find(|set| set.name == "starter");
        setup_of(starter.expect("the starter set is embedded"))
    }

    #[test]
    fn the_starter_setup_is_valid() {
        let setup = starter_setup();
        assert_eq!(setup.one.cards.len(), 30);
        assert_eq!(setup.two.cards.len(), 30);
        assert_eq!(setup.one.hero.name, "Capitaine d'avant-garde");
        assert_eq!(setup.two.hero.name, "Gardienne du rempart");
        assert_eq!(started_versions(setup), ["starter@1.0.0"]);
    }

    #[test]
    fn the_fixture_setup_records_its_own_version() {
        assert_eq!(started_versions(fixture_setup()), ["fixture@1.0.0"]);
    }

    /// The starter decks in both orders: rush as One, then as Two.
    fn starter_setups() -> [GameSetup; 2] {
        let setup = starter_setup();
        let swapped = GameSetup {
            one: setup.two.clone(),
            two: setup.one.clone(),
            data_versions: setup.data_versions.clone(),
        };
        [setup, swapped]
    }

    #[test]
    fn starter_games_all_end_by_a_hero_going_down_or_exhaustion_and_use_both_mechanics() {
        let (mut hero_down, mut deaths, mut ripostes, mut withheld) = (0, 0, 0, 0);
        for setup in starter_setups() {
            for seed in 0..200 {
                let outcome = play_inspected(setup.clone(), seed, |actions, events| {
                    let attacks = |target: fn(&Target) -> bool| {
                        actions.iter().any(|action| {
                            matches!(action, Action::Attack { target: t, .. } if target(t))
                        })
                    };
                    if attacks(|t| matches!(t, Target::Creature(_)))
                        && !attacks(|t| *t == Target::Hero)
                    {
                        withheld += 1;
                    }
                    ripostes += events
                        .iter()
                        .filter(|event| matches!(event, Event::Riposted { .. }))
                        .count();
                });
                match outcome.reason {
                    WinReason::HeroDown => hero_down += 1,
                    WinReason::Exhaustion => {}
                    WinReason::Concede => panic!("seed {seed}: conceded or hit the step cap"),
                }
                deaths += outcome.deaths;
            }
        }
        assert!(hero_down > 0, "heroes fall");
        assert!(deaths > 0, "creatures die");
        assert!(ripostes > 0, "ripostes happen");
        assert!(
            withheld > 0,
            "a provoking creature hides the hero from attackers"
        );
    }

    #[test]
    fn a_starter_seed_always_gives_the_same_game() {
        for setup in starter_setups() {
            for seed in 0..20 {
                assert_eq!(
                    play_with(setup.clone(), seed),
                    play_with(setup.clone(), seed)
                );
            }
        }
    }

    /// Without any randomness: the first creature to summon, else the first attack offered
    /// (the hero first, or the provoking creature when there is one), else the first effect.
    fn scripted(state: &duel_engine::GameState) -> Action {
        let actions: Vec<Action> = legal_actions(state)
            .into_iter()
            .filter(|action| *action != Action::Concede)
            .collect();
        let first = |wanted: fn(&Action) -> bool| actions.iter().copied().find(&wanted);
        first(|a| matches!(a, Action::Play { .. }))
            .or_else(|| first(|a| matches!(a, Action::Attack { .. })))
            .or_else(|| first(|a| matches!(a, Action::Activate { .. })))
            .expect("a running game always offers an action")
    }

    /// Plays the scripted policy to the end; returns the whole journal.
    fn scripted_game(setup: GameSetup, seed: u64) -> (duel_engine::GameState, Vec<Event>) {
        let (mut state, mut journal) = new_game(setup, seed).expect("the setup is valid");
        for _ in 0..MAX_STEPS {
            if state.winner().is_some() {
                return (state, journal);
            }
            let action = scripted(&state);
            journal.extend(apply(&mut state, action).expect("a legal action applies"));
        }
        panic!("seed {seed}: the scripted game did not end");
    }

    #[test]
    fn the_scripted_starter_game_ends_by_a_hero_going_down() {
        let [straight, swapped] = starter_setups();
        for (setup, rush_is) in [(straight, PlayerId::One), (swapped, PlayerId::Two)] {
            let rush_health = if rush_is == PlayerId::One {
                setup.one.hero.health
            } else {
                setup.two.hero.health
            };
            let bulwark_health = if rush_is == PlayerId::One {
                setup.two.hero.health
            } else {
                setup.one.hero.health
            };
            for seed in 0..10 {
                let (state, journal) = scripted_game(setup.clone(), seed);
                assert_eq!(journal, scripted_game(setup.clone(), seed).1, "seed {seed}");
                assert!(matches!(
                    journal.first(),
                    Some(Event::GameStarted { first, .. }) if *first == rush_is
                ));
                let Some(Event::GameWon { winner, reason }) = journal.last().cloned() else {
                    panic!("seed {seed}: the journal ends with GameWon");
                };
                assert_eq!(reason, WinReason::HeroDown, "seed {seed}");
                assert_eq!(state.winner(), Some((winner, reason)));
                let loser = winner.opponent();
                assert_eq!(state.hero_health(loser), 0, "seed {seed}");
                let start = if loser == rush_is {
                    rush_health
                } else {
                    bulwark_health
                };
                let taken: u32 = journal
                    .iter()
                    .filter_map(|event| match event {
                        Event::Damaged { player, amount } if *player == loser => Some(*amount),
                        _ => None,
                    })
                    .sum();
                assert!(
                    taken >= start,
                    "seed {seed}: {taken} damage for {start} health"
                );
                let lethal = journal
                    .iter()
                    .rposition(|event| matches!(event, Event::Attacked { .. }))
                    .expect("the lethal attack");
                assert!(
                    journal[lethal..].iter().all(|event| !matches!(
                        event,
                        Event::TokenMoved { .. } | Event::ActivePlayerChanged { .. }
                    )),
                    "seed {seed}: no hand-over after the lethal attack"
                );
                assert!(legal_actions(&state).is_empty(), "seed {seed}");
            }
        }
    }
}
