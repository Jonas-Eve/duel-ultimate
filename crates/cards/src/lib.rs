//! Cards and decks are data, not code. A set is a versioned folder of JSON files under
//! `data/`; a game records which set versions it used.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deck {
    pub id: String,
    pub name: String,
    pub cards: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CardsError {
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("duplicate card id `{0}`")]
    DuplicateCard(String),
    #[error("deck `{deck}` references unknown card `{card}`")]
    UnknownCard { deck: String, card: String },
}

pub fn parse_cards(json: &str) -> Result<Vec<Card>, CardsError> {
    let cards: Vec<Card> =
        serde_json::from_str(json).map_err(|e| CardsError::Json(e.to_string()))?;
    let mut seen = HashSet::new();
    for card in &cards {
        if !seen.insert(card.id.as_str()) {
            return Err(CardsError::DuplicateCard(card.id.clone()));
        }
    }
    Ok(cards)
}

pub fn validate_deck(deck: &Deck, cards: &[Card]) -> Result<(), CardsError> {
    let known: HashSet<&str> = cards.iter().map(|c| c.id.as_str()).collect();
    match deck.cards.iter().find(|id| !known.contains(id.as_str())) {
        Some(card) => Err(CardsError::UnknownCard {
            deck: deck.id.clone(),
            card: card.clone(),
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARDS: &str = r#"[{"id":"a","name":"A"},{"id":"b","name":"B"}]"#;

    #[test]
    fn rejects_duplicate_ids() {
        let json = r#"[{"id":"a","name":"A"},{"id":"a","name":"A2"}]"#;
        assert_eq!(
            parse_cards(json),
            Err(CardsError::DuplicateCard("a".into()))
        );
    }

    #[test]
    fn rejects_deck_with_unknown_card() {
        let cards = parse_cards(CARDS).unwrap();
        let deck = Deck {
            id: "d".into(),
            name: "D".into(),
            cards: vec!["a".into(), "zzz".into()],
        };
        assert!(matches!(
            validate_deck(&deck, &cards),
            Err(CardsError::UnknownCard { .. })
        ));
    }

    #[test]
    fn accepts_valid_deck() {
        let cards = parse_cards(CARDS).unwrap();
        let deck = Deck {
            id: "d".into(),
            name: "D".into(),
            cards: vec!["a".into(), "b".into()],
        };
        assert_eq!(validate_deck(&deck, &cards), Ok(()));
    }
}
