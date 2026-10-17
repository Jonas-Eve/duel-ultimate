//! Cards, heroes and decks are data, not code. A set is a versioned folder of JSON files
//! under `data/<set>/<version>/`; a game records which set versions it used.
//!
//! This crate is the single implementation of data validation. It handles untrusted input
//! and must never panic.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )
)]

mod art;
mod card;
mod deck;
mod hero;
mod validate;

pub use art::{
    ART_LICENSES, ArtEntry, ArtError, ArtFormat, ArtLicense, MAX_ART_AUTHOR_LEN, MAX_ART_DOC_BYTES,
    MAX_ART_SOURCE_LEN, parse_art,
};
pub use card::{Card, CreatureStats};
pub use deck::Deck;
pub use hero::{Effect, Hero, HeroEffect};
pub use validate::{
    CardsError, DECK_SIZE, MAX_HERO_EFFECTS, MAX_NAME_LEN, MAX_STAT, parse_cards, parse_decks,
    resolve_deck, resolve_deck_named, validate_card_names, validate_creature, validate_deck_shape,
    validate_hero,
};
