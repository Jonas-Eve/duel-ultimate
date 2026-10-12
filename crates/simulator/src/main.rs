//! Plays N seeded games between the fixture decks and prints the first-player win rate.
//! Policy: uniformly random among the legal actions other than `Concede`, drawn from the
//! simulator's own generator (separate from the game's). The real simulator
//! (`run_batch`, reports) comes with the simulator story.

// Red-phase skeleton: bodies are `todo!()` until the feat commit.
#![allow(dead_code)]

use duel_engine::{GameSetup, PlayerId, WinReason};

/// The fixture decks as a game setup: the first deck is One's, the second Two's.
fn fixture_setup() -> GameSetup {
    todo!()
}

/// Plays one game to its end and returns who won and why.
fn play(_seed: u64) -> (PlayerId, WinReason) {
    todo!()
}

fn main() {
    let games: u64 = std::env::args()
        .nth(1)
        .and_then(|n| n.parse().ok())
        .unwrap_or(1000);
    let one = (0..games)
        .filter(|&seed| play(seed).0 == PlayerId::One)
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
    fn every_game_ends() {
        for seed in 0..200 {
            let (_, reason) = play(seed);
            assert_ne!(reason, WinReason::HeroDown, "no attack exists yet");
        }
    }

    #[test]
    fn a_seed_always_gives_the_same_game() {
        for seed in 0..20 {
            assert_eq!(play(seed), play(seed));
        }
    }

    #[test]
    fn games_end_by_exhaustion_with_the_random_policy() {
        let exhausted = (0..50)
            .filter(|&s| play(s).1 == WinReason::Exhaustion)
            .count();
        assert_eq!(exhausted, 50, "nobody concedes, the piles run dry");
    }
}
