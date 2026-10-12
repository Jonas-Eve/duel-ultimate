//! Game creation: validation, deterministic shuffle, opening hands, who starts.

use crate::rules::Event;
use crate::state::GameState;
use duel_cards::{CardsError, CreatureStats, Hero};
use serde::{Deserialize, Serialize};

/// One side of a game: a hero and the resolved stats of its 30 cards, in deck order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerSetup {
    pub hero: Hero,
    pub cards: Vec<CreatureStats>,
}

/// Everything a game needs. The engine reads neither disk nor identifiers: callers resolve
/// decks with `duel-cards` first. `data_versions` is recorded as is (replays, balance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameSetup {
    pub one: PlayerSetup,
    pub two: PlayerSetup,
    pub data_versions: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SetupError {
    #[error("invalid game data: {0}")]
    Invalid(#[from] CardsError),
}

impl SetupError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "invalid_setup",
        }
    }
}

/// Starts a game. Re-validates `setup` (it may come from an untrusted client) and never
/// panics. The events hold `GameStarted` and, if the first player has nothing to do,
/// `GameWon`.
pub fn new_game(_setup: GameSetup, _seed: u64) -> Result<(GameState, Vec<Event>), SetupError> {
    todo!()
}
