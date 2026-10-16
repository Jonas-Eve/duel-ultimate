//! Pure rules engine. No I/O, no clock, no global randomness: every random draw comes from
//! the seeded generator held in [`GameState`], so a game is replayable from `seed + actions`.
//!
//! One continuous turn: whoever holds the token acts while they have budget. Every action
//! costs time through a single time rule; crossing the boundary hands the token over.
//! The engine consumes data already resolved by `duel-cards` and re-validates it in
//! [`new_game`], because the setup may come from an untrusted client (WASM).
//! The [`data`](game_setup_from_data) module builds a [`GameSetup`] from `cards.json` /
//! `decks.json` strings and deck identifiers (the one implementation shared by the simulator
//! and `engine-wasm`); it parses text, never reads the disk. A card's display name travels
//! with its instance but no rule, formula or draw reads it.
//!
//! Randomness is consumed only at setup, in this documented order: shuffle of One's deck,
//! shuffle of Two's deck, then `random_below(2)` if (and only if) the initiatives are equal.
//! [`apply`] never consumes the generator.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )
)]

mod combat;
mod data;
mod effects;
mod rules;
mod setup;
mod state;
#[cfg(test)]
mod testing;
mod time;
mod view;

pub use data::{DataError, game_setup_from_data, player_setup};
pub use rules::{Action, Event, RuleError, Target, apply, legal_actions};
pub use setup::{GameSetup, PlayerSetup, SetupError, new_game};
pub use state::{CardInstanceId, GameState, PlayerId, WinReason};
pub use view::{CardView, GameResult, HeroView, OpponentSide, OwnSide, PlayerView, player_view};
