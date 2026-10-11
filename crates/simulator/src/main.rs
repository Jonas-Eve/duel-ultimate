//! Plays N seeded games and prints the first-player win rate. Placeholder policy for now
//! (the player on turn 10 concedes); real decks and AIs come with the first mechanics.

use duel_engine::{Action, GameState, PlayerId, apply};

fn play(seed: u64) -> PlayerId {
    let mut state = GameState::new(seed);
    loop {
        let action = if state.turn >= 10 {
            Action::Concede
        } else {
            Action::EndTurn
        };
        apply(&mut state, action).expect("game is running");
        if let Some(winner) = state.winner {
            return winner;
        }
    }
}

fn main() {
    let games: u64 = std::env::args()
        .nth(1)
        .and_then(|n| n.parse().ok())
        .unwrap_or(1000);
    let one = (0..games)
        .filter(|&seed| play(seed) == PlayerId::One)
        .count();
    println!(
        "{games} games: player one wins {:.1}%",
        100.0 * one as f64 / games as f64
    );
}
