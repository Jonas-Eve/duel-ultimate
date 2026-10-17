//! The simulator: plays seeded AI-vs-AI batches between decks ([`run_batch`]) and reports how
//! they went ([`Report`]). The policy only sees the [`Observation`](duel_engine::Observation) of
//! the active seat (its legal actions other than `Concede`, its own view) and its own generator
//! ([`Policy`], from `duel-ai`); every seed derives from the batch seed ([`derive_seed`], also
//! from `duel-ai`).
//!
//! This library never reads the disk or the network: the data comes in as strings
//! ([`contenders_from`], [`setup_from`]). Only the `duel-simulator` binary embeds data files.

pub mod batch;
pub mod report;
pub mod stats;

pub use batch::{
    BatchConfig, Contender, DEFAULT_MAX_ACTIONS, GameRecord, contenders_from, game_seed, play_game,
    run_batch, run_batch_observed,
};
pub use duel_ai::{Policy, TAG_POLICY_ONE, TAG_POLICY_TWO, derive_seed, policy_rng, random_policy};
pub use report::{Kind, MatchupReport, Report, rules_label};
pub use stats::{fmt_mean, fmt_share, percent, wilson_95};

use duel_cards::{parse_cards, parse_decks};
use duel_engine::{GameSetup, PlayerSetup, RuleSet, player_setup};

/// The first deck of `decks` as player One and the second as player Two, with `cards` resolving
/// their card ids, and `data_versions` recorded in the game. Pure: nothing is read from disk;
/// invalid data is an `Err` message, never a panic.
pub fn setup_from(
    cards: &str,
    decks: &str,
    data_versions: Vec<String>,
) -> Result<GameSetup, String> {
    let cards = parse_cards(cards).map_err(|error| error.to_string())?;
    let decks = parse_decks(decks).map_err(|error| error.to_string())?;
    let side = |index: usize| -> Result<PlayerSetup, String> {
        let deck = decks
            .get(index)
            .ok_or_else(|| format!("a set needs two decks, found {}", decks.len()))?;
        player_setup(deck, &cards).map_err(|error| error.to_string())
    };
    Ok(GameSetup {
        one: side(0)?,
        two: side(1)?,
        data_versions,
        rules: RuleSet::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use duel_engine::{Event, WinReason, new_game};

    fn fixture_setup() -> GameSetup {
        setup_from(
            include_str!("../../../data/fixture/1.0.0/cards.json"),
            include_str!("../../../data/fixture/1.0.0/decks.json"),
            vec!["fixture@1.0.0".to_owned()],
        )
        .expect("the fixture is valid")
    }

    /// One game with the random policy and the default cap.
    fn play_with(setup: GameSetup, seed: u64) -> GameRecord {
        play_game(setup, seed, &&random_policy, DEFAULT_MAX_ACTIONS, |_, _| {})
    }

    /// How the game ended; an interrupted game (the cap) has no end.
    fn reason_of(record: &GameRecord) -> Option<WinReason> {
        record.winner.map(|(_, reason)| reason)
    }

    /// Plays one game of the fixture decks to its end.
    fn play(seed: u64) -> GameRecord {
        play_with(fixture_setup(), seed)
    }

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
            match reason_of(&play(seed)) {
                Some(WinReason::HeroDown) => hero_down += 1,
                Some(WinReason::Exhaustion) => {}
                Some(WinReason::Concede) | None => {
                    panic!("seed {seed}: the policy never concedes")
                }
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
            match reason_of(&outcome) {
                Some(WinReason::HeroDown | WinReason::Exhaustion) => {}
                Some(WinReason::Concede) | None => {
                    panic!("seed {seed}: conceded or hit the step cap")
                }
            }
            deaths += outcome.deaths;
        }
        assert!(deaths > 0, "creatures die in those games");
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

    #[test]
    fn the_fixture_setup_records_its_own_version() {
        assert_eq!(started_versions(fixture_setup()), ["fixture@1.0.0"]);
    }
}
