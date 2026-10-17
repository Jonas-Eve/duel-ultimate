//! AI for the duel: policies that choose among the actions of an [`Observation`], the seeds
//! that feed them, and the harness ([`choose`]) that runs one on a game state.
//!
//! A policy never receives the game state: only the [`Observation`] of the active seat and its
//! own generator, distinct from the engine's (seeds derive through [`derive_seed`] and the
//! `TAG_*` constants). No game rule lives here; no I/O, no clock, no system entropy.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )
)]

mod choose;
mod policies;
mod policy;
mod seeds;

pub use choose::{AiError, choose};
pub use policies::random::random_policy;
pub use policy::Policy;
pub use seeds::{TAG_COMPUTER, TAG_POLICY_ONE, TAG_POLICY_TWO, derive_seed, policy_rng};
