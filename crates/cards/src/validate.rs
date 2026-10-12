use crate::{Card, CreatureStats, Deck, Hero};

/// Exact number of cards in a deck.
pub const DECK_SIZE: usize = 30;
/// Upper bound of every numeric stat (keeps hostile data from producing endless games).
pub const MAX_STAT: u32 = 10_000;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CardsError {
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("duplicate card id `{0}`")]
    DuplicateCard(String),
    #[error("duplicate deck id `{0}`")]
    DuplicateDeck(String),
    #[error("deck `{deck}` references unknown card `{card}`")]
    UnknownCard { deck: String, card: String },
    #[error("`{field}` must be at least 1 (a cost of 0 would be free)")]
    CostBelowOne { field: &'static str },
    #[error("`max_time` must be at least 1")]
    MaxTimeBelowOne,
    #[error("`start_budget` is {start_budget} but must be between 1 and `max_time` ({max_time})")]
    StartBudgetOutOfRange { start_budget: u32, max_time: u32 },
    #[error("`{field}` must be greater than 0")]
    NonPositiveStat { field: &'static str },
    #[error("`starting_hand` is {starting_hand} but the deck only has {deck_len} cards")]
    StartingHandExceedsDeck { starting_hand: u32, deck_len: usize },
    #[error("a deck must have exactly {expected} cards, found {found}")]
    WrongDeckSize { found: usize, expected: usize },
    #[error("`{field}` is {value}, above the maximum of {max}")]
    StatTooHigh {
        field: &'static str,
        value: u32,
        max: u32,
    },
    #[error("card `{id}`: {source}")]
    InvalidCard { id: String, source: Box<CardsError> },
    #[error("deck `{id}`: {source}")]
    InvalidDeck { id: String, source: Box<CardsError> },
}

pub fn parse_cards(json: &str) -> Result<Vec<Card>, CardsError> {
    serde_json::from_str(json).map_err(|e| CardsError::Json(e.to_string()))
}

pub fn parse_decks(json: &str) -> Result<Vec<Deck>, CardsError> {
    serde_json::from_str(json).map_err(|e| CardsError::Json(e.to_string()))
}

pub fn validate_creature(_stats: &CreatureStats) -> Result<(), CardsError> {
    Ok(())
}

pub fn validate_hero(_hero: &Hero) -> Result<(), CardsError> {
    Ok(())
}

pub fn validate_deck_shape(_hero: &Hero, _deck_len: usize) -> Result<(), CardsError> {
    Ok(())
}

pub fn resolve_deck(_deck: &Deck, _cards: &[Card]) -> Result<Vec<CreatureStats>, CardsError> {
    Ok(Vec::new())
}
