//! Per-game rule options. A closed set (one field per option, one decision in
//! [`RuleSet::validate`] per field): only options that are actually measured are added, never a
//! generic list. Not versioned data (`duel-cards` holds cards and decks): the options of a game
//! travel in `GameSetup.rules` and are recorded in `Event::GameStarted`.
//!
//! **Both options have no effect yet** (until #127): no rule, no `validate`, no `apply`, no
//! potential formula reads them, and they never touch the generator.

use duel_cards::DECK_SIZE;
use serde::{Deserialize, Serialize};

/// Largest `board_limit`: the size of a deck, past which a limit could never bind.
pub const MAX_BOARD_LIMIT: u32 = DECK_SIZE as u32;

/// The options of one game. In JSON a partial object is read with the engine default for each
/// absent field (the default of the time, so an explicit `"board_limit": null` is how to write
/// "no limit" whatever the default is); the output is always the complete object, in
/// declaration order. Unknown fields are refused: the value may come from an untrusted client.
///
/// No effect before #127.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleSet {
    /// Whether a hero hits back when attacked. No effect before #127.
    pub hero_riposte: bool,
    /// Largest number of creatures a board may hold; `None` means no limit, `Some(n)` needs
    /// `1 <= n <=` [`MAX_BOARD_LIMIT`]. No effect before #127.
    pub board_limit: Option<u32>,
}

impl Default for RuleSet {
    /// The engine default. Written by hand, not derived: #127 changes this body (and only it).
    fn default() -> Self {
        Self::legacy()
    }
}

impl RuleSet {
    /// Every option off: the behaviour of the engine before the options existed. Never
    /// modified, whatever the default becomes: it is the "options disabled" reference.
    pub const fn legacy() -> Self {
        Self {
            hero_riposte: false,
            board_limit: None,
        }
    }

    /// Whether this is the engine default (what `GameSetup` and `GameStarted` leave out).
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Checks the options of a setup that may come from an untrusted client.
    pub fn validate(&self) -> Result<(), RuleSetError> {
        // No `..`: an option added later breaks the build here, where it must be decided.
        let RuleSet {
            hero_riposte,
            board_limit,
        } = *self;
        let _ = hero_riposte; // a bool: every value is valid
        match board_limit {
            Some(value) if !(1..=MAX_BOARD_LIMIT).contains(&value) => {
                Err(RuleSetError::BoardLimitOutOfRange {
                    value,
                    max: MAX_BOARD_LIMIT,
                })
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuleSetError {
    #[error("`board_limit` is {value}, but must be between 1 and {max}")]
    BoardLimitOutOfRange { value: u32, max: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_is_every_option_off() {
        assert_eq!(
            RuleSet::legacy(),
            RuleSet {
                hero_riposte: false,
                board_limit: None
            }
        );
    }

    #[test]
    fn the_bounds_are_inclusive() {
        let with = |n| RuleSet {
            board_limit: Some(n),
            ..RuleSet::legacy()
        };
        assert!(with(1).validate().is_ok());
        assert!(with(MAX_BOARD_LIMIT).validate().is_ok());
        assert!(with(0).validate().is_err());
        assert!(with(MAX_BOARD_LIMIT + 1).validate().is_err());
    }
}
