//! Plays N seeded games between the fixture decks and prints the first-player win rate.
//! Policy: uniformly random among the legal actions other than `Concede` (attacks on heroes
//! and on creatures alike), drawn from the simulator's own generator (separate from the
//! game's). The real simulator (`run_batch`, reports) comes with the simulator story.

use duel_cards::{parse_cards, parse_decks, resolve_deck};
use duel_engine::{
    Action, Event, GameSetup, PlayerId, PlayerSetup, WinReason, apply, legal_actions, new_game,
};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

const FIXTURE_VERSION: &str = "fixture@1.0.0";
const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

/// Mixed into the game seed so the policy's generator is not the game's.
const POLICY_SALT: u64 = 0x5DEE_CE66_D1CE_4E5B;
/// Defensive cap: every game ends by itself, but a rule bug must not hang the batch.
const MAX_STEPS: usize = 10_000;

/// The fixture decks as a game setup: the first deck is One's, the second Two's.
fn fixture_setup() -> GameSetup {
    let cards = parse_cards(FIXTURE_CARDS).expect("the embedded fixture cards are valid");
    let decks = parse_decks(FIXTURE_DECKS).expect("the embedded fixture decks are valid");
    let side = |index: usize| {
        let deck = decks.get(index).expect("the fixture has two decks");
        PlayerSetup {
            hero: deck.hero.clone(),
            cards: resolve_deck(deck, &cards).expect("the fixture decks resolve"),
        }
    };
    GameSetup {
        one: side(0),
        two: side(1),
        data_versions: vec![FIXTURE_VERSION.to_owned()],
    }
}

/// How a game went: who won, why, and how many creatures died along the way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Outcome {
    winner: PlayerId,
    reason: WinReason,
    deaths: usize,
}

/// Plays one game to its end.
fn play(seed: u64) -> Outcome {
    let (mut state, _) = new_game(fixture_setup(), seed).expect("the fixture is a valid setup");
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
    let games: u64 = std::env::args()
        .nth(1)
        .and_then(|n| n.parse().ok())
        .unwrap_or(1000);
    let one = (0..games)
        .filter(|&seed| play(seed).winner == PlayerId::One)
        .count();
    println!(
        "{games} games: player one wins {:.1}%",
        100.0 * one as f64 / games as f64
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
