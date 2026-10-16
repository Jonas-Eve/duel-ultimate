//! Card names: the optional display names of a deck, parallel to its cards (S2-T1 of #52).

use duel_cards::{
    CardsError, MAX_NAME_LEN, parse_cards, parse_decks, resolve_deck, resolve_deck_named,
    validate_card_names,
};

const CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

fn names(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("c{i}")).collect()
}

#[test]
fn the_name_limit_is_64_characters() {
    assert_eq!(MAX_NAME_LEN, 64);
}

#[test]
fn a_card_exposes_its_name() {
    let cards = parse_cards(CARDS).unwrap();
    assert_eq!(cards[0].name(), "Gobelin");
    assert_eq!(cards[1].name(), "Archère");
}

#[test]
fn no_names_or_one_per_card_are_valid() {
    assert_eq!(validate_card_names(&[], 30), Ok(()));
    assert_eq!(validate_card_names(&names(30), 30), Ok(()));
}

#[test]
fn a_different_non_zero_length_is_refused() {
    for len in [1, 29, 31, 60] {
        assert_eq!(
            validate_card_names(&names(len), 30),
            Err(CardsError::CardNamesMismatch {
                names: len,
                cards: 30
            }),
            "{len} names"
        );
    }
}

#[test]
fn the_limit_counts_characters_not_bytes() {
    let at_limit = |c: char| vec![c.to_string().repeat(MAX_NAME_LEN)];
    assert_eq!(validate_card_names(&at_limit('a'), 1), Ok(()));
    // 64 two-byte characters are 128 bytes: still accepted.
    assert_eq!(validate_card_names(&at_limit('é'), 1), Ok(()));
    for c in ['a', 'é'] {
        assert_eq!(
            validate_card_names(&[c.to_string().repeat(MAX_NAME_LEN + 1)], 1),
            Err(CardsError::NameTooLong {
                len: MAX_NAME_LEN + 1,
                max: MAX_NAME_LEN
            })
        );
    }
}

#[test]
fn an_empty_name_is_allowed() {
    assert_eq!(validate_card_names(&[String::new()], 1), Ok(()));
}

#[test]
fn resolve_deck_named_returns_the_stats_and_the_names_in_deck_order() {
    let cards = parse_cards(CARDS).unwrap();
    let decks = parse_decks(DECKS).unwrap();
    for deck in &decks {
        let (stats, names) = resolve_deck_named(deck, &cards).unwrap();
        assert_eq!(stats, resolve_deck(deck, &cards).unwrap());
        assert_eq!(names.len(), deck.cards.len());
        for (id, name) in deck.cards.iter().zip(&names) {
            let card = cards.iter().find(|c| c.id() == id).unwrap();
            assert_eq!(name, card.name(), "card {id}");
        }
    }
    let (_, names) = resolve_deck_named(&decks[0], &cards).unwrap();
    assert_eq!(
        names[0], "Gobelin",
        "the first card of `aggro` is the goblin"
    );
}

#[test]
fn resolve_deck_and_resolve_deck_named_fail_the_same_way() {
    let cards = parse_cards(CARDS).unwrap();
    let decks = parse_decks(DECKS).unwrap();
    let mut broken = Vec::new();
    let mut unknown = decks[0].clone();
    unknown.cards[3] = "ghost".into();
    broken.push(unknown);
    let mut short = decks[0].clone();
    short.cards.pop();
    broken.push(short);
    let mut hero = decks[0].clone();
    hero.hero.max_time = 0;
    broken.push(hero);
    let mut hand = decks[0].clone();
    hand.hero.starting_hand = 31;
    broken.push(hand);
    for deck in &broken {
        let plain = resolve_deck(deck, &cards).unwrap_err();
        let named = resolve_deck_named(deck, &cards).unwrap_err();
        assert_eq!(plain, named);
        assert_eq!(plain.to_string(), named.to_string());
    }
    assert!(matches!(
        resolve_deck_named(&broken[0], &cards),
        Err(CardsError::UnknownCard { .. })
    ));
}
