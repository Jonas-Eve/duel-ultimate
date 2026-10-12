//! Actions, events and the rules that apply them.

use crate::state::{CardInstanceId, GameState, PlayerId, WinReason};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Activates the active hero's effect at this index of its list (untrusted input).
    Activate {
        effect: u32,
    },
    Concede,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    GameStarted {
        first: PlayerId,
        budget: u32,
    },
    EffectActivated {
        player: PlayerId,
        effect: u32,
        cost: u32,
    },
    Drew {
        player: PlayerId,
        card: CardInstanceId,
    },
    /// After every non-final action: `player` holds the token with `budget`.
    TokenMoved {
        player: PlayerId,
        budget: u32,
    },
    /// Only when the token crossed the boundary.
    ActivePlayerChanged {
        player: PlayerId,
    },
    GameWon {
        winner: PlayerId,
        reason: WinReason,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("the game is already over")]
    GameOver,
    #[error("the active hero has no such effect")]
    UnknownEffect,
    #[error("this effect cannot be activated right now")]
    EffectNotApplicable,
}

impl RuleError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::GameOver => "game_over",
            Self::UnknownEffect => "unknown_effect",
            Self::EffectNotApplicable => "effect_not_applicable",
        }
    }
}

/// The single legality check, shared by [`apply`] and [`legal_actions`].
pub(crate) fn validate(_state: &GameState, _action: &Action) -> Result<(), RuleError> {
    todo!()
}

/// Ends the game if the active player has no legal action other than conceding.
pub(crate) fn check_exhaustion(_state: &mut GameState, _events: &mut Vec<Event>) {
    todo!()
}

/// Applies `action` for the active player. A refusal leaves the state (and its generator)
/// untouched. Never consumes randomness.
pub fn apply(_state: &mut GameState, _action: Action) -> Result<Vec<Event>, RuleError> {
    todo!()
}

/// Every action the active player may take: hero effects in the order of the hero's list,
/// then `Concede`. Empty once the game is over.
pub fn legal_actions(_state: &GameState) -> Vec<Action> {
    todo!()
}
