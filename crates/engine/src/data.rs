//! Game setup from data strings: `cards.json` + `decks.json` + two deck identifiers in,
//! a [`GameSetup`] out. The single implementation: the simulator and `engine-wasm` call it
//! instead of resolving decks themselves. Pure: no disk, no clock; the JSON is analysed by
//! `duel-cards`, which re-validates everything, and nothing here panics.

use crate::ruleset::RuleSet;
use crate::setup::{GameSetup, PlayerSetup};
use duel_cards::{Card, CardsError, Deck, parse_cards, parse_decks, resolve_deck_named};

/// Why a setup could not be built from data.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DataError {
    #[error("{0}")]
    Cards(#[from] CardsError),
    #[error("unknown deck `{0}`")]
    UnknownDeck(String),
}

impl DataError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Cards(CardsError::Json(_)) => "invalid_json",
            Self::Cards(_) => "invalid_setup",
            Self::UnknownDeck(_) => "unknown_deck",
        }
    }
}

/// One side of a game from a deck: its hero, and the stats and names of its cards (resolved
/// against `cards` by [`resolve_deck_named`], the only resolution path).
pub fn player_setup(deck: &Deck, cards: &[Card]) -> Result<PlayerSetup, CardsError> {
    let (stats, card_names) = resolve_deck_named(deck, cards)?;
    Ok(PlayerSetup {
        hero: deck.hero.clone(),
        cards: stats,
        card_names,
    })
}

/// Builds a game setup from the text of `cards.json` and `decks.json`: the deck `deck_one`
/// as player One and `deck_two` as player Two (the same deck twice is a mirror), found by
/// strict equality of their identifiers. The cards are analysed first, then the decks, then
/// the identifiers are looked up (One's first), then each deck is resolved.
/// `data_versions` is recorded as is (opaque).
pub fn game_setup_from_data(
    cards: &str,
    decks: &str,
    deck_one: &str,
    deck_two: &str,
    data_versions: Vec<String>,
) -> Result<GameSetup, DataError> {
    let cards = parse_cards(cards)?;
    let decks = parse_decks(decks)?;
    let find = |id: &str| {
        decks
            .iter()
            .find(|deck| deck.id == id)
            .ok_or_else(|| DataError::UnknownDeck(id.to_owned()))
    };
    let (one, two) = (find(deck_one)?, find(deck_two)?);
    Ok(GameSetup {
        one: player_setup(one, &cards)?,
        two: player_setup(two, &cards)?,
        data_versions,
        rules: RuleSet::default(),
    })
}
