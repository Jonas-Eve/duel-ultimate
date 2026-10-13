use crate::{Card, CreatureStats, Deck, Effect, Hero};
use std::collections::{BTreeMap, BTreeSet};

/// Exact number of cards in a deck.
pub const DECK_SIZE: usize = 30;
/// Upper bound of every numeric stat (keeps hostile data from producing endless games).
pub const MAX_STAT: u32 = 10_000;
/// Maximum number of effects a hero can have (bounds hostile data, like [`MAX_STAT`]).
pub const MAX_HERO_EFFECTS: usize = 8;

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
    #[error("a hero has at most {max} effects, found {found}")]
    TooManyEffects { found: usize, max: usize },
    #[error("effect #{effect}: `cost` must be at least 1 (a cost of 0 would be free)")]
    EffectCostBelowOne { effect: usize },
    #[error("effect #{effect}: `count` must be at least 1")]
    EffectCountBelowOne { effect: usize },
    #[error("effect #{effect}: `{field}` is {value}, above the maximum of {max}")]
    EffectStatTooHigh {
        effect: usize,
        field: &'static str,
        value: u32,
        max: u32,
    },
    #[error("card `{id}`: {source}")]
    InvalidCard { id: String, source: Box<CardsError> },
    #[error("deck `{id}`: {source}")]
    InvalidDeck { id: String, source: Box<CardsError> },
}

fn cost_in_range(field: &'static str, value: u32) -> Result<(), CardsError> {
    if value < 1 {
        return Err(CardsError::CostBelowOne { field });
    }
    below_max(field, value)
}

fn positive_in_range(field: &'static str, value: u32) -> Result<(), CardsError> {
    if value < 1 {
        return Err(CardsError::NonPositiveStat { field });
    }
    below_max(field, value)
}

fn below_max(field: &'static str, value: u32) -> Result<(), CardsError> {
    if value > MAX_STAT {
        return Err(CardsError::StatTooHigh {
            field,
            value,
            max: MAX_STAT,
        });
    }
    Ok(())
}

fn ensure_unique<'a>(
    ids: impl Iterator<Item = &'a str>,
    duplicate: fn(String) -> CardsError,
) -> Result<(), CardsError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(duplicate(id.to_owned()));
        }
    }
    Ok(())
}

/// Parses and validates a `cards.json` document.
pub fn parse_cards(json: &str) -> Result<Vec<Card>, CardsError> {
    let cards: Vec<Card> =
        serde_json::from_str(json).map_err(|e| CardsError::Json(e.to_string()))?;
    ensure_unique(cards.iter().map(Card::id), CardsError::DuplicateCard)?;
    for card in &cards {
        validate_creature(&card.stats()).map_err(|e| CardsError::InvalidCard {
            id: card.id().to_owned(),
            source: Box::new(e),
        })?;
    }
    Ok(cards)
}

/// Parses and validates a `decks.json` document (hero and deck shape; card ids are
/// checked against a card list by [`resolve_deck`]).
pub fn parse_decks(json: &str) -> Result<Vec<Deck>, CardsError> {
    let decks: Vec<Deck> =
        serde_json::from_str(json).map_err(|e| CardsError::Json(e.to_string()))?;
    ensure_unique(
        decks.iter().map(|d| d.id.as_str()),
        CardsError::DuplicateDeck,
    )?;
    for deck in &decks {
        validate_hero(&deck.hero)
            .and_then(|()| validate_deck_shape(&deck.hero, deck.cards.len()))
            .map_err(|e| CardsError::InvalidDeck {
                id: deck.id.clone(),
                source: Box::new(e),
            })?;
    }
    Ok(decks)
}

pub fn validate_creature(stats: &CreatureStats) -> Result<(), CardsError> {
    cost_in_range("cost", stats.cost)?;
    cost_in_range("attack_cost", stats.attack_cost)?;
    positive_in_range("force", stats.force)?;
    positive_in_range("health", stats.health)?;
    // 0 is valid (the default: no riposte); only the upper bound applies.
    below_max("riposte", stats.riposte)
}

pub fn validate_hero(hero: &Hero) -> Result<(), CardsError> {
    if hero.max_time < 1 {
        return Err(CardsError::MaxTimeBelowOne);
    }
    below_max("max_time", hero.max_time)?;
    if hero.start_budget < 1 || hero.start_budget > hero.max_time {
        return Err(CardsError::StartBudgetOutOfRange {
            start_budget: hero.start_budget,
            max_time: hero.max_time,
        });
    }
    positive_in_range("health", hero.health)?;
    validate_effects(hero)
}

fn validate_effects(hero: &Hero) -> Result<(), CardsError> {
    if hero.effects.len() > MAX_HERO_EFFECTS {
        return Err(CardsError::TooManyEffects {
            found: hero.effects.len(),
            max: MAX_HERO_EFFECTS,
        });
    }
    for (effect, hero_effect) in hero.effects.iter().enumerate() {
        if hero_effect.cost < 1 {
            return Err(CardsError::EffectCostBelowOne { effect });
        }
        too_high(effect, "cost", hero_effect.cost)?;
        match hero_effect.effect {
            Effect::Draw { count } => {
                if count < 1 {
                    return Err(CardsError::EffectCountBelowOne { effect });
                }
                too_high(effect, "count", count)?;
            }
        }
    }
    Ok(())
}

fn too_high(effect: usize, field: &'static str, value: u32) -> Result<(), CardsError> {
    if value > MAX_STAT {
        return Err(CardsError::EffectStatTooHigh {
            effect,
            field,
            value,
            max: MAX_STAT,
        });
    }
    Ok(())
}

/// Checks what depends on the deck length: exactly [`DECK_SIZE`] cards, and a starting
/// hand that fits in it.
pub fn validate_deck_shape(hero: &Hero, deck_len: usize) -> Result<(), CardsError> {
    if deck_len != DECK_SIZE {
        return Err(CardsError::WrongDeckSize {
            found: deck_len,
            expected: DECK_SIZE,
        });
    }
    // `u32 -> usize` is lossless on every supported target; the fallback only guards
    // exotic 16-bit ones.
    if usize::try_from(hero.starting_hand).map_or(true, |hand| hand > deck_len) {
        return Err(CardsError::StartingHandExceedsDeck {
            starting_hand: hero.starting_hand,
            deck_len,
        });
    }
    Ok(())
}

/// Validates a deck (hero, shape, card references, stats) and returns the resolved
/// stats of its cards, in deck order.
pub fn resolve_deck(deck: &Deck, cards: &[Card]) -> Result<Vec<CreatureStats>, CardsError> {
    validate_hero(&deck.hero)?;
    validate_deck_shape(&deck.hero, deck.cards.len())?;
    let by_id: BTreeMap<&str, &Card> = cards.iter().map(|c| (c.id(), c)).collect();
    deck.cards
        .iter()
        .map(|id| {
            let card = by_id
                .get(id.as_str())
                .ok_or_else(|| CardsError::UnknownCard {
                    deck: deck.id.clone(),
                    card: id.clone(),
                })?;
            let stats = card.stats();
            validate_creature(&stats).map_err(|e| CardsError::InvalidCard {
                id: id.clone(),
                source: Box::new(e),
            })?;
            Ok(stats)
        })
        .collect()
}
