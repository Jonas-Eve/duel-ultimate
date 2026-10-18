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
mod effects;
mod hero;
mod tags;
mod validate;

pub use art::{
    ART_LICENSES, ArtEntry, ArtError, ArtFormat, ArtLicense, MAX_ART_AUTHOR_LEN, MAX_ART_DOC_BYTES,
    MAX_ART_SOURCE_LEN, parse_art,
};
pub use card::{Card, CardSpec, CreatureStats, SpellSpec, UnitSpec};
pub use deck::Deck;
pub use effects::{
    AttackMode, CardEffect, DURATION_NAMES, Duration, KEYWORD_NAMES, Keyword, MAX_CARD_EFFECTS,
    MAX_EFFECT_PICKS, MAX_OPS_PER_EFFECT, MODE_NAMES, OP_NAMES, Op, OpKind, PhiClass, PickKind,
    PickRole, PickSlot, Polarity, SELECT_NAMES, SIDE_NAMES, STAT_NAMES, Selector, SelectorKind,
    Side, Stat, TRIGGER_NAMES, Trigger,
};
pub use hero::{Effect, Hero, HeroEffect};
pub use tags::{
    MAX_DATASET_TAGS, MAX_TAG_LEN, MAX_TAGS_DOC_BYTES, MAX_TAGS_PER_CARD, TagsError, parse_tags,
};
pub use validate::{
    CardsError, DECK_SIZE, MAX_HERO_EFFECTS, MAX_NAME_LEN, MAX_STAT, parse_card_set, parse_cards,
    parse_decks, resolve_deck, resolve_deck_named, validate_card_names, validate_card_spec,
    validate_creature, validate_deck_shape, validate_hero, validate_spell, validate_unit,
};
