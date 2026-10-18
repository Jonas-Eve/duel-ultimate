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
//! [`player_view`] projects a state for one seat (its own hand, the opponent's hand and both
//! piles only as counts): the types make the opposing hand impossible to express. It does not
//! authenticate the seat it is asked for; a server calls it with the seat of the connection.
//!
//! [`Observation`] is what an AI policy receives instead of the state: the active seat, its legal
//! actions without `Concede` and, on demand, its own [`PlayerView`]; it gives no way back to
//! the [`GameState`] (the signature is the guarantee, nothing is authenticated).
//!
//! [`preview_legal_actions`] gives, for each legal action and in the same order, its time cost,
//! whether it hands the token over, the budget after it, whether it ends the game and whether it
//! destroys the attacker, by a dry run on a clone (the time rule stays in one place, the generator is never touched); it never
//! copies what a draw brings. `Observation::outcome` reuses that same dry run and adds the view of
//! the acting seat after the action, where a drawn card is only counted ([`OutcomeView`]: the
//! masking follows the `Drew` events, not a difference of hands).
//!
//! [`RuleSet`] holds the per-game options (two closed ones: `hero_riposte`, `board_limit`),
//! carried by `GameSetup.rules`, validated by [`new_game`] (`invalid_setup`) and copied into
//! `Event::GameStarted` only when they differ from the engine default. `hero_riposte` acts (#170:
//! a non-lethal hit on the hero destroys the attacker, `Event::HeroRiposted`); `board_limit` has no
//! effect yet (#171). Neither touches the generator. To replay a historical game, pass its rules
//! explicitly.
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
mod observation;
mod preview;
mod rules;
mod ruleset;
mod setup;
mod state;
#[cfg(test)]
mod testing;
mod time;
mod view;

pub use data::{DataError, game_setup_from_data, player_setup};
pub use observation::{ActionOutcome, Observation};
pub use preview::{ActionPreview, preview_legal_actions};
pub use rules::{Action, Event, RuleError, Target, apply, legal_actions};
pub use ruleset::{MAX_BOARD_LIMIT, RuleSet, RuleSetError};
pub use setup::{GameSetup, PlayerSetup, SetupError, new_game};
pub use state::{CardInstanceId, GameState, PlayerId, WinReason};
pub use view::{
    CardView, GameResult, HeroView, OpponentSide, OutcomeOwnSide, OutcomeView, OwnSide, PlayerView,
    player_view,
};
