//! Pure rules engine. No I/O, no clock, no global randomness: every random draw comes from
//! the seeded generator held in [`GameState`], so a game is replayable from `seed + actions`.

use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerId {
    One,
    Two,
}

impl PlayerId {
    pub fn opponent(self) -> Self {
        match self {
            Self::One => Self::Two,
            Self::Two => Self::One,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    EndTurn,
    Concede,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    TurnStarted { player: PlayerId, turn: u32 },
    GameWon { winner: PlayerId },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("the game is already over")]
    GameOver,
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub turn: u32,
    pub active: PlayerId,
    pub winner: Option<PlayerId>,
    rng: ChaCha8Rng,
}

impl GameState {
    /// Starts a game; the seed also decides who plays first.
    pub fn new(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let active = if rng.random_bool(0.5) {
            PlayerId::One
        } else {
            PlayerId::Two
        };
        Self {
            turn: 1,
            active,
            winner: None,
            rng,
        }
    }

    /// Draws from the game's own seeded generator, the only source of randomness rules may use.
    pub fn random_below(&mut self, bound: u32) -> u32 {
        self.rng.random_range(0..bound)
    }
}

/// Applies `action` for the active player and returns the events it produced.
pub fn apply(state: &mut GameState, action: Action) -> Result<Vec<Event>, RuleError> {
    if state.winner.is_some() {
        return Err(RuleError::GameOver);
    }
    let events = match action {
        Action::EndTurn => {
            state.active = state.active.opponent();
            state.turn += 1;
            vec![Event::TurnStarted {
                player: state.active,
                turn: state.turn,
            }]
        }
        Action::Concede => {
            let winner = state.active.opponent();
            state.winner = Some(winner);
            vec![Event::GameWon { winner }]
        }
    };
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_first_player() {
        assert_eq!(GameState::new(42).active, GameState::new(42).active);
    }

    #[test]
    fn same_seed_same_draws() {
        let (mut a, mut b) = (GameState::new(7), GameState::new(7));
        let draws = |s: &mut GameState| (0..5).map(|_| s.random_below(100)).collect::<Vec<_>>();
        assert_eq!(draws(&mut a), draws(&mut b));
    }

    #[test]
    fn end_turn_alternates_players() {
        let mut state = GameState::new(1);
        let first = state.active;
        apply(&mut state, Action::EndTurn).unwrap();
        assert_eq!(state.active, first.opponent());
        assert_eq!(state.turn, 2);
    }

    #[test]
    fn concede_ends_the_game() {
        let mut state = GameState::new(1);
        let loser = state.active;
        apply(&mut state, Action::Concede).unwrap();
        assert_eq!(state.winner, Some(loser.opponent()));
        assert_eq!(apply(&mut state, Action::EndTurn), Err(RuleError::GameOver));
    }
}
