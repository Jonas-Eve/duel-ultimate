use crate::tags::validate_card_tags;
use crate::{
    AttackMode, Card, CardEffect, CardSpec, CreatureStats, Deck, Effect, Hero, MAX_CARD_EFFECTS,
    MAX_EFFECT_PICKS, MAX_OPS_PER_EFFECT, Op, Side, SpellSpec, Trigger, UnitSpec,
};
use std::collections::{BTreeMap, BTreeSet};

/// Exact number of cards in a deck.
pub const DECK_SIZE: usize = 30;
/// Upper bound of every numeric stat (keeps hostile data from producing endless games).
pub const MAX_STAT: u32 = 10_000;
/// Maximum number of effects a hero can have (bounds hostile data, like [`MAX_STAT`]).
pub const MAX_HERO_EFFECTS: usize = 8;

/// Maximum length of a card name, in characters (untrusted text shown by clients).
pub const MAX_NAME_LEN: usize = 64;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CardsError {
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("duplicate card id `{0}`")]
    DuplicateCard(String),
    #[error("duplicate card name `{0}`")]
    DuplicateCardName(String),
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
    #[error(
        "`card_names` has {names} entries but the deck has {cards} cards (empty, or one per card)"
    )]
    CardNamesMismatch { names: usize, cards: usize },
    #[error("a card name is {len} characters long, above the maximum of {max}")]
    NameTooLong { len: usize, max: usize },
    #[error("an immobile unit has no `{field}`")]
    ImmobileWithAttackStat { field: &'static str },
    #[error("a unit that is not immobile needs `{field}`")]
    MissingAttackStat { field: &'static str },
    #[error("a card has at most {max} tags, found {found}")]
    TooManyTags { found: usize, max: usize },
    #[error("a tag cannot be empty")]
    TagEmpty,
    #[error("a tag is {len} characters long, above the maximum of {max}")]
    TagTooLong { len: usize, max: usize },
    #[error("a tag cannot contain a control character")]
    TagControlChar,
    #[error("a tag cannot start or end with whitespace")]
    TagUntrimmed,
    #[error("duplicate tag `{0}` on a card")]
    DuplicateTag(String),
    #[error("unknown tag `{0}` (not in the tag list of the data set)")]
    UnknownTag(String),
    #[error("a card has at most {max} effects, found {found}")]
    TooManyCardEffects { found: usize, max: usize },
    #[error("effect #{effect}: an effect has at least one operation")]
    EffectWithoutOps { effect: usize },
    #[error("effect #{effect}: an effect has at most {max} operations, found {found}")]
    TooManyOps {
        effect: usize,
        found: usize,
        max: usize,
    },
    #[error("effect #{effect}: only an activated effect has a `cost`")]
    EffectCostForbidden { effect: usize },
    #[error("effect #{effect}: an activated effect needs a `cost`")]
    EffectCostRequired { effect: usize },
    #[error("effect #{effect}: `{op}` is not allowed with the trigger `{trigger}`")]
    TriggerNotAllowed {
        effect: usize,
        op: &'static str,
        trigger: &'static str,
    },
    #[error("effect #{effect}: `{op}` is not allowed on the selector `{select}`")]
    SelectorNotAllowed {
        effect: usize,
        op: &'static str,
        select: &'static str,
    },
    #[error("effect #{effect}: `{op}` is not allowed on this side (`opposing` only)")]
    SideNotAllowed { effect: usize, op: &'static str },
    #[error("effect #{effect}: `{field}` of `{op}` must be at least 1")]
    OpValueBelowOne {
        effect: usize,
        op: &'static str,
        field: &'static str,
    },
    #[error("effect #{effect}: `delta` of `{op}` cannot be 0")]
    DeltaZero { effect: usize, op: &'static str },
    #[error("effect #{effect}: `attackers` is {found}, must be 1 or 2")]
    AttackersOutOfRange { effect: usize, found: u32 },
    #[error("effect #{effect}: an attack with 2 attackers needs a `mode`")]
    AttackModeMissing { effect: usize },
    #[error("effect #{effect}: `mode` is only for an attack with 2 attackers")]
    AttackModeUnexpected { effect: usize },
    #[error("effect #{effect}: it expects {found} picks, at most {max}")]
    TooManyExpectedPicks {
        effect: usize,
        found: usize,
        max: usize,
    },
    #[error("effect #{effect}: an immobile unit cannot declare an `attack` effect")]
    ImmobileWithAttackEffect { effect: usize },
    #[error("a spell has at least one effect")]
    SpellWithoutEffects,
    #[error("effect #{effect}: every effect of a spell is `on_play`")]
    SpellEffectNotOnPlay { effect: usize },
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

/// Parses and validates a `cards.json` document. Fail-closed on tags: no tag list is given, so
/// a card carrying a tag is refused (see [`parse_card_set`]).
pub fn parse_cards(json: &str) -> Result<Vec<Card>, CardsError> {
    parse_card_set(json, &[])
}

/// Like [`parse_cards`], with the closed tag list of the data set (`tags.json`, see
/// [`parse_tags`](crate::parse_tags)): a tag outside `tags` is refused, after the checks of the
/// shape of the card.
pub fn parse_card_set(json: &str, tags: &[String]) -> Result<Vec<Card>, CardsError> {
    let cards: Vec<Card> =
        serde_json::from_str(json).map_err(|e| CardsError::Json(e.to_string()))?;
    ensure_unique(cards.iter().map(Card::id), CardsError::DuplicateCard)?;
    ensure_unique(cards.iter().map(Card::name), CardsError::DuplicateCardName)?;
    for card in &cards {
        let checked = validate_card(card).and_then(|spec| {
            match spec.tags().iter().find(|t| !tags.contains(t)) {
                Some(unknown) => Err(CardsError::UnknownTag(unknown.clone())),
                None => Ok(()),
            }
        });
        checked.map_err(|e| invalid_card(card, e))?;
    }
    Ok(cards)
}

fn invalid_card(card: &Card, error: CardsError) -> CardsError {
    CardsError::InvalidCard {
        id: card.id().to_owned(),
        source: Box::new(error),
    }
}

/// The data-level checks of one card (which of `force` and `attack_cost` it may carry), then
/// the validation of its spec.
fn validate_card(card: &Card) -> Result<CardSpec, CardsError> {
    let Card::Creature {
        force,
        attack_cost,
        immobile,
        ..
    } = card
    else {
        // A spell has no stat of a unit to check at the data level.
        let spec = card.spec();
        validate_card_spec(&spec)?;
        return Ok(spec);
    };
    if *immobile {
        for (field, value) in [("force", force), ("attack_cost", attack_cost)] {
            if value.is_some() {
                return Err(CardsError::ImmobileWithAttackStat { field });
            }
        }
    } else {
        for (field, value) in [("force", force), ("attack_cost", attack_cost)] {
            if value.is_none() {
                return Err(CardsError::MissingAttackStat { field });
            }
        }
    }
    let spec = card.spec();
    validate_card_spec(&spec)?;
    Ok(spec)
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

/// Validates a card of any kind: see [`validate_unit`] and [`validate_spell`].
pub fn validate_card_spec(spec: &CardSpec) -> Result<(), CardsError> {
    match spec {
        CardSpec::Unit(unit) => validate_unit(unit),
        CardSpec::Spell(spell) => validate_spell(spell),
    }
}

/// Validates a unit. Not immobile: [`validate_creature`]. Immobile: the same bounds for `cost`,
/// `health` and `riposte`, and `force` and `attack_cost` must be 0 (the internal state of a unit
/// without them). Then its effects (at most [`MAX_CARD_EFFECTS`], each valid, and none with an
/// `attack` operation if it is immobile), then its tags.
pub fn validate_unit(unit: &UnitSpec) -> Result<(), CardsError> {
    let stats = &unit.stats;
    if unit.immobile {
        cost_in_range("cost", stats.cost)?;
        positive_in_range("health", stats.health)?;
        below_max("riposte", stats.riposte)?;
        for (field, value) in [("force", stats.force), ("attack_cost", stats.attack_cost)] {
            if value != 0 {
                return Err(CardsError::ImmobileWithAttackStat { field });
            }
        }
    } else {
        validate_creature(stats)?;
    }
    validate_card_effects(&unit.effects, false, unit.immobile)?;
    validate_card_tags(&unit.tags)
}

/// Validates a spell: its `cost` (bounded like the cost of a unit), at least one effect, every
/// effect `on_play`, the bounds of the effects, then the tags.
pub fn validate_spell(spell: &SpellSpec) -> Result<(), CardsError> {
    cost_in_range("cost", spell.cost)?;
    if spell.effects.is_empty() {
        return Err(CardsError::SpellWithoutEffects);
    }
    if let Some(effect) = spell
        .effects
        .iter()
        .position(|e| e.trigger != Trigger::OnPlay)
    {
        return Err(CardsError::SpellEffectNotOnPlay { effect });
    }
    validate_card_effects(&spell.effects, true, false)?;
    validate_card_tags(&spell.tags)
}

/// The effects of a card: their number, then each effect in order.
fn validate_card_effects(
    effects: &[CardEffect],
    is_spell: bool,
    immobile: bool,
) -> Result<(), CardsError> {
    if effects.len() > MAX_CARD_EFFECTS {
        return Err(CardsError::TooManyCardEffects {
            found: effects.len(),
            max: MAX_CARD_EFFECTS,
        });
    }
    for (index, effect) in effects.iter().enumerate() {
        validate_card_effect(index, effect, is_spell, immobile)?;
    }
    Ok(())
}

/// One effect, in this order: number of operations, cost, each operation (trigger, selector,
/// side, values, attack parameters), number of expected picks, immobile.
fn validate_card_effect(
    index: usize,
    effect: &CardEffect,
    is_spell: bool,
    immobile: bool,
) -> Result<(), CardsError> {
    if effect.ops.is_empty() {
        return Err(CardsError::EffectWithoutOps { effect: index });
    }
    if effect.ops.len() > MAX_OPS_PER_EFFECT {
        return Err(CardsError::TooManyOps {
            effect: index,
            found: effect.ops.len(),
            max: MAX_OPS_PER_EFFECT,
        });
    }
    match (effect.trigger, effect.cost) {
        (Trigger::Activated, None) => {
            return Err(CardsError::EffectCostRequired { effect: index });
        }
        (Trigger::Activated, Some(cost)) => {
            if cost < 1 {
                return Err(CardsError::EffectCostBelowOne { effect: index });
            }
            too_high(index, "cost", cost)?;
        }
        (_, Some(_)) => return Err(CardsError::EffectCostForbidden { effect: index }),
        (_, None) => {}
    }
    for op in &effect.ops {
        validate_op(index, effect.trigger, op)?;
    }
    let picks = effect.expected_picks(is_spell).len();
    if picks > MAX_EFFECT_PICKS {
        return Err(CardsError::TooManyExpectedPicks {
            effect: index,
            found: picks,
            max: MAX_EFFECT_PICKS,
        });
    }
    if immobile && effect.ops.iter().any(|op| matches!(op, Op::Attack { .. })) {
        return Err(CardsError::ImmobileWithAttackEffect { effect: index });
    }
    Ok(())
}

/// A value that is a count or an amount: at least 1, at most [`MAX_STAT`].
fn op_value(
    effect: usize,
    op: &'static str,
    field: &'static str,
    value: u32,
) -> Result<(), CardsError> {
    if value < 1 {
        return Err(CardsError::OpValueBelowOne { effect, op, field });
    }
    too_high(effect, field, value)
}

fn validate_op(effect: usize, trigger: Trigger, op: &Op) -> Result<(), CardsError> {
    let kind = op.kind();
    let name = kind.name();
    if !kind.triggers().contains(&trigger) {
        return Err(CardsError::TriggerNotAllowed {
            effect,
            op: name,
            trigger: trigger.name(),
        });
    }
    let target = op.target();
    if !kind.selectors().contains(&target.kind()) {
        return Err(CardsError::SelectorNotAllowed {
            effect,
            op: name,
            select: target.kind().name(),
        });
    }
    match op {
        Op::Attack {
            target,
            amount,
            attackers,
            mode,
            ..
        } => {
            // Wave 1: an attack never targets its own side.
            if target.side() != Side::Opposing {
                return Err(CardsError::SideNotAllowed { effect, op: name });
            }
            op_value(effect, name, "amount", *amount)?;
            if !(1..=2).contains(attackers) {
                return Err(CardsError::AttackersOutOfRange {
                    effect,
                    found: *attackers,
                });
            }
            match (attackers, mode) {
                (2, None) => Err(CardsError::AttackModeMissing { effect }),
                (1, Some(AttackMode::Simultaneous | AttackMode::Chained)) => {
                    Err(CardsError::AttackModeUnexpected { effect })
                }
                _ => Ok(()),
            }
        }
        Op::DealDamage { amount, .. } => op_value(effect, name, "amount", *amount),
        Op::Draw { count, .. } => op_value(effect, name, "count", *count),
        Op::ModifyStat { delta, .. } => {
            if *delta == 0 {
                return Err(CardsError::DeltaZero { effect, op: name });
            }
            too_high(effect, "delta", delta.unsigned_abs())
        }
        Op::GrantKeyword { .. } => Ok(()),
    }
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

/// Checks the optional display names of a deck: none at all, or exactly one per card, each at
/// most [`MAX_NAME_LEN`] characters (`chars().count()`, not bytes).
pub fn validate_card_names(names: &[String], cards_len: usize) -> Result<(), CardsError> {
    if !names.is_empty() && names.len() != cards_len {
        return Err(CardsError::CardNamesMismatch {
            names: names.len(),
            cards: cards_len,
        });
    }
    for name in names {
        let len = name.chars().count();
        if len > MAX_NAME_LEN {
            return Err(CardsError::NameTooLong {
                len,
                max: MAX_NAME_LEN,
            });
        }
    }
    Ok(())
}

/// Validates a deck (hero, shape, card references, specs) and returns the resolved
/// specs of its cards, in deck order.
pub fn resolve_deck(deck: &Deck, cards: &[Card]) -> Result<Vec<CardSpec>, CardsError> {
    resolve_deck_named(deck, cards).map(|(specs, _)| specs)
}

/// The single resolution path: like [`resolve_deck`], and also returns the names of the
/// cards, parallel to the specs (deck order). The names are presentation only.
pub fn resolve_deck_named(
    deck: &Deck,
    cards: &[Card],
) -> Result<(Vec<CardSpec>, Vec<String>), CardsError> {
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
            let spec = validate_card(card).map_err(|e| CardsError::InvalidCard {
                id: id.clone(),
                source: Box::new(e),
            })?;
            Ok((spec, card.name().to_owned()))
        })
        .collect()
}
