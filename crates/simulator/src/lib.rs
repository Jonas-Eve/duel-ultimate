//! The simulator's game engine: plays seeded AI-vs-AI games between two decks and reports how
//! each went. Policy: uniformly random among the legal actions other than `Concede` (attacks on
//! heroes and on creatures alike), drawn from the simulator's own generator (separate from the
//! game's).
//!
//! This library never reads the disk or the network: the data comes in as strings
//! ([`setup_from`]). Only the `duel-simulator` binary embeds data files.

use duel_cards::{parse_cards, parse_decks, resolve_deck};
use duel_engine::{
    Action, Event, GameSetup, PlayerId, PlayerSetup, WinReason, apply, legal_actions, new_game,
};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Mixed into the game seed so the policy's generator is not the game's.
const POLICY_SALT: u64 = 0x5DEE_CE66_D1CE_4E5B;
/// Defensive cap: every game ends by itself, but a rule bug must not hang the batch.
pub const MAX_STEPS: usize = 10_000;

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
        Ok(PlayerSetup {
            hero: deck.hero.clone(),
            cards: resolve_deck(deck, &cards).map_err(|error| error.to_string())?,
        })
    };
    Ok(GameSetup {
        one: side(0)?,
        two: side(1)?,
        data_versions,
    })
}

/// How a game went: who won, why, and how many creatures died along the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub winner: PlayerId,
    pub reason: WinReason,
    pub deaths: usize,
}

/// Plays one game of `setup` to its end.
pub fn play_with(setup: GameSetup, seed: u64) -> Outcome {
    play_inspected(setup, seed, |_, _| {})
}

/// Plays one game of `setup` to its end, showing `on_step` each step: the actions the policy
/// chose among (never `Concede`) and the events of the action it applied.
pub fn play_inspected(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_setup() -> GameSetup {
        setup_from(
            include_str!("../../../data/fixture/1.0.0/cards.json"),
            include_str!("../../../data/fixture/1.0.0/decks.json"),
            vec!["fixture@1.0.0".to_owned()],
        )
        .expect("the fixture is valid")
    }

    /// Plays one game of the fixture decks to its end.
    fn play(seed: u64) -> Outcome {
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
