//! `setup_from`: strings in, a game setup (or an error message) out, no I/O.

use duel_simulator::setup_from;

const CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

fn versions() -> Vec<String> {
    vec!["fixture@1.0.0".to_owned()]
}

#[test]
fn the_fixture_gives_two_decks_of_30_and_the_given_version() {
    let setup = setup_from(CARDS, DECKS, versions()).expect("the fixture is valid");
    assert_eq!(setup.one.cards.len(), 30);
    assert_eq!(setup.two.cards.len(), 30);
    assert_eq!(setup.data_versions, versions());
}

#[test]
fn an_unknown_card_is_an_error_not_a_panic() {
    let decks = DECKS.replacen("\"goblin\"", "\"ghost\"", 1);
    assert_ne!(decks, DECKS, "the mutation applied");
    let message = setup_from(CARDS, &decks, versions()).expect_err("an unknown card");
    assert!(message.contains("ghost"), "{message}");
}

#[test]
fn a_deck_of_29_cards_is_an_error_not_a_panic() {
    let decks = DECKS.replacen("\"goblin\",", "", 1);
    assert_ne!(decks, DECKS, "the mutation applied");
    assert!(setup_from(CARDS, &decks, versions()).is_err());
}

#[test]
fn malformed_json_and_a_missing_second_deck_are_errors() {
    assert!(setup_from("not json", DECKS, versions()).is_err());
    assert!(setup_from(CARDS, "[]", versions()).is_err());
}
