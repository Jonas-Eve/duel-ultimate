//! Game setup from data strings (red stub).

use crate::setup::{GameSetup, PlayerSetup};
use duel_cards::{Card, CardsError, Deck, resolve_deck_named};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DataError {
    #[error("{0}")]
    Cards(#[from] CardsError),
    #[error("unknown deck `{0}`")]
    UnknownDeck(String),
}

impl DataError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Cards(CardsError::Json(_)) => "invalid_json",
            Self::Cards(_) => "invalid_setup",
            Self::UnknownDeck(_) => "unknown_deck",
        }
    }
}

pub fn player_setup(deck: &Deck, cards: &[Card]) -> Result<PlayerSetup, CardsError> {
    let (stats, card_names) = resolve_deck_named(deck, cards)?;
    Ok(PlayerSetup {
        hero: deck.hero.clone(),
        cards: stats,
        card_names,
    })
}

pub fn game_setup_from_data(
    _cards: &str,
    _decks: &str,
    deck_one: &str,
    _deck_two: &str,
    _data_versions: Vec<String>,
) -> Result<GameSetup, DataError> {
    Err(DataError::UnknownDeck(deck_one.to_owned()))
}
