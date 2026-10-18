//! `game_setup_from_data` and `player_setup`: data strings in, a game setup out (S2-T1 of
//! #52). Decks are chosen by identifier, seats and decks are independent, errors have stable
//! codes, and no disk is involved (the data are `include_str!`ed here).

use duel_cards::{CardsError, parse_cards, parse_decks, resolve_deck};
use duel_engine::{DataError, PlayerSetup, game_setup_from_data, new_game, player_setup};
use serde_json::Value;

const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");
const STARTER_CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const STARTER_DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");

/// The side of the deck `id`, built by hand from `duel-cards` (independently of the module
/// under test).
fn expected_side(cards: &str, decks: &str, id: &str) -> PlayerSetup {
    let cards = parse_cards(cards).unwrap();
    let decks = parse_decks(decks).unwrap();
    let deck = decks.iter().find(|d| d.id == id).unwrap();
    PlayerSetup {
        hero: deck.hero.clone(),
        cards: resolve_deck(deck, &cards)
            .unwrap()
            .into_iter()
            .map(Into::into)
            .collect(),
        card_names: deck
            .cards
            .iter()
            .map(|card| {
                cards
                    .iter()
                    .find(|c| c.id() == card)
                    .unwrap()
                    .name()
                    .to_owned()
            })
            .collect(),
    }
}

#[test]
fn player_setup_is_the_hero_the_stats_and_the_names_of_the_deck() {
    for (cards, decks, id) in [
        (FIXTURE_CARDS, FIXTURE_DECKS, "aggro"),
        (FIXTURE_CARDS, FIXTURE_DECKS, "control"),
        (STARTER_CARDS, STARTER_DECKS, "rush"),
        (STARTER_CARDS, STARTER_DECKS, "bulwark"),
    ] {
        let parsed_cards = parse_cards(cards).unwrap();
        let parsed_decks = parse_decks(decks).unwrap();
        let deck = parsed_decks.iter().find(|d| d.id == id).unwrap();
        let side = player_setup(deck, &parsed_cards).unwrap();
        assert_eq!(side, expected_side(cards, decks, id), "{id}");
        assert_eq!(side.card_names.len(), 30, "{id}");
    }
    let first = expected_side(FIXTURE_CARDS, FIXTURE_DECKS, "aggro");
    assert_eq!(first.card_names[0], "Gobelin");
}

#[test]
fn player_setup_reports_the_errors_of_resolve_deck() {
    let cards = parse_cards(FIXTURE_CARDS).unwrap();
    let mut deck = parse_decks(FIXTURE_DECKS).unwrap().remove(0);
    deck.cards[2] = "ghost".into();
    assert_eq!(
        player_setup(&deck, &cards).unwrap_err(),
        resolve_deck(&deck, &cards).unwrap_err()
    );
}

#[test]
fn the_decks_are_picked_by_identifier_and_seats_follow_the_arguments() {
    let setup =
        game_setup_from_data(FIXTURE_CARDS, FIXTURE_DECKS, "aggro", "control", vec![]).unwrap();
    assert_eq!(
        setup.one,
        expected_side(FIXTURE_CARDS, FIXTURE_DECKS, "aggro")
    );
    assert_eq!(
        setup.two,
        expected_side(FIXTURE_CARDS, FIXTURE_DECKS, "control")
    );

    let swapped =
        game_setup_from_data(FIXTURE_CARDS, FIXTURE_DECKS, "control", "aggro", vec![]).unwrap();
    assert_eq!(swapped.one, setup.two);
    assert_eq!(swapped.two, setup.one);
    assert_ne!(
        setup.one, setup.two,
        "the two decks differ: the check is not vacuous"
    );
}

#[test]
fn reordering_decks_json_changes_nothing_because_the_choice_is_by_identifier() {
    for (cards, decks, a, b) in [
        (FIXTURE_CARDS, FIXTURE_DECKS, "aggro", "control"),
        (STARTER_CARDS, STARTER_DECKS, "rush", "bulwark"),
    ] {
        let mut value: Value = serde_json::from_str(decks).unwrap();
        value.as_array_mut().unwrap().reverse();
        let reordered = value.to_string();
        for (one, two) in [(a, b), (b, a), (a, a)] {
            assert_eq!(
                game_setup_from_data(cards, &reordered, one, two, vec![]).unwrap(),
                game_setup_from_data(cards, decks, one, two, vec![]).unwrap(),
                "{one} vs {two}"
            );
        }
    }
}

#[test]
fn a_mirror_is_allowed() {
    let setup = game_setup_from_data(STARTER_CARDS, STARTER_DECKS, "rush", "rush", vec![]).unwrap();
    assert_eq!(setup.one, setup.two);
    assert!(new_game(setup, 1).is_ok());
}

#[test]
fn the_data_versions_are_copied_as_is_opaque_and_unsorted() {
    let versions = vec!["zeta@2".to_owned(), "alpha@1".to_owned(), String::new()];
    let setup = game_setup_from_data(
        FIXTURE_CARDS,
        FIXTURE_DECKS,
        "aggro",
        "control",
        versions.clone(),
    )
    .unwrap();
    assert_eq!(setup.data_versions, versions);
}

#[test]
fn the_setup_starts_a_game() {
    for seed in 0..5 {
        let setup =
            game_setup_from_data(STARTER_CARDS, STARTER_DECKS, "bulwark", "rush", vec![]).unwrap();
        assert!(new_game(setup, seed).is_ok());
    }
}

#[test]
fn an_unknown_deck_is_refused_whatever_the_seat_with_its_own_code() {
    let ask = |one: &str, two: &str| {
        game_setup_from_data(FIXTURE_CARDS, FIXTURE_DECKS, one, two, vec![]).unwrap_err()
    };
    assert_eq!(
        ask("nope", "control"),
        DataError::UnknownDeck("nope".into())
    );
    assert_eq!(ask("aggro", "nope"), DataError::UnknownDeck("nope".into()));
    // The first one reported when both are unknown.
    assert_eq!(
        ask("first", "second"),
        DataError::UnknownDeck("first".into())
    );
    // Strict equality: no partial match, no case folding, no index, no trimming.
    for id in ["agg", "AGGRO", "aggro ", " aggro", "", "0", "1", "*"] {
        let error = ask(id, "control");
        assert_eq!(error, DataError::UnknownDeck(id.into()), "{id:?}");
        assert_eq!(error.code(), "unknown_deck");
    }
}

#[test]
fn errors_have_stable_codes() {
    let code = |cards: &str, decks: &str| {
        game_setup_from_data(cards, decks, "aggro", "control", vec![])
            .unwrap_err()
            .code()
    };
    // Malformed JSON: invalid_json, for the cards and for the decks.
    assert_eq!(code("not json", FIXTURE_DECKS), "invalid_json");
    assert_eq!(code(FIXTURE_CARDS, "{"), "invalid_json");
    assert_eq!(code("", ""), "invalid_json");
    // Well-formed but invalid data: invalid_setup.
    let unknown_card = FIXTURE_DECKS.replacen("\"goblin\"", "\"ghost\"", 1);
    assert_ne!(unknown_card, FIXTURE_DECKS);
    assert_eq!(code(FIXTURE_CARDS, &unknown_card), "invalid_setup");
    let short_deck = FIXTURE_DECKS.replacen("\"goblin\",", "", 1);
    assert_ne!(short_deck, FIXTURE_DECKS);
    assert_eq!(code(FIXTURE_CARDS, &short_deck), "invalid_setup");
    let doubled = format!(
        "[{0},{0}]",
        FIXTURE_CARDS
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
    );
    assert_eq!(code(&doubled, FIXTURE_DECKS), "invalid_setup");
    // The error of the data survives in the message.
    let error =
        game_setup_from_data(FIXTURE_CARDS, &unknown_card, "aggro", "control", vec![]).unwrap_err();
    assert!(matches!(
        &error,
        DataError::Cards(CardsError::UnknownCard { card, .. }) if card == "ghost"
    ));
    assert!(error.to_string().contains("ghost"), "{error}");
}

#[test]
fn the_cards_are_analysed_first_then_the_decks_then_the_deck_identifiers() {
    let both_wrong = game_setup_from_data("not json", "{", "nope", "nope", vec![]).unwrap_err();
    assert_eq!(both_wrong.code(), "invalid_json");
    assert!(matches!(&both_wrong, DataError::Cards(CardsError::Json(_))));
    // Valid cards, invalid decks and an unknown identifier: the decks come first.
    let error = game_setup_from_data(FIXTURE_CARDS, "{", "nope", "nope", vec![]).unwrap_err();
    assert_eq!(error.code(), "invalid_json");
}
