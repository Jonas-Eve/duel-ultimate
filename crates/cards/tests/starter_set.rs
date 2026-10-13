//! Data tests for `data/starter/1.0.0/` (S3-T2): two provisional decks, `rush` then `bulwark`.
//! The numbers are provisional and not balanced; these tests only pin the properties the
//! story asks for (two distinct decks, heroes, distributions and exclusive mechanics).
use std::collections::BTreeSet;

use duel_cards::{Card, CreatureStats, DECK_SIZE, Deck, parse_cards, parse_decks, resolve_deck};

const CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");
const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");

fn cards() -> Vec<Card> {
    parse_cards(CARDS).expect("starter cards parse")
}

fn decks() -> Vec<Deck> {
    parse_decks(DECKS).expect("starter decks parse")
}

fn deck<'a>(decks: &'a [Deck], id: &str) -> &'a Deck {
    decks.iter().find(|d| d.id == id).expect("deck present")
}

/// Resolved stats of the 30 cards of a deck.
fn resolved(id: &str) -> Vec<CreatureStats> {
    let decks = decks();
    resolve_deck(deck(&decks, id), &cards()).expect("deck resolves")
}

fn sum(stats: &[CreatureStats], f: fn(&CreatureStats) -> u32) -> u32 {
    stats.iter().map(f).sum()
}

/// Both decks have `DECK_SIZE` cards, so comparing averages means comparing sums: the average
/// gap is `(a - b) / DECK_SIZE`, and `gap >= tenths / 10` becomes pure integer arithmetic.
fn assert_gap(name: &str, rush: u32, bulwark: u32, tenths: u32, rush_is_lower: bool) {
    let (low, high) = if rush_is_lower {
        (rush, bulwark)
    } else {
        (bulwark, rush)
    };
    assert!(
        high > low,
        "{name}: expected the other direction (rush {rush}, bulwark {bulwark})"
    );
    assert!(
        (high - low) * 10 >= tenths * DECK_SIZE as u32,
        "{name}: average gap too small (rush sum {rush}, bulwark sum {bulwark})"
    );
}

#[test]
fn starter_data_parses_with_two_decks_rush_then_bulwark() {
    cards();
    let decks = decks();
    let ids: Vec<&str> = decks.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids, ["rush", "bulwark"]);
}

#[test]
fn each_deck_resolves_to_thirty_creatures() {
    assert_eq!(DECK_SIZE, 30);
    for id in ["rush", "bulwark"] {
        assert_eq!(resolved(id).len(), DECK_SIZE, "{id}");
    }
}

#[test]
fn card_ids_are_unique_and_each_card_is_used_by_a_deck() {
    let cards = cards();
    let ids: Vec<&str> = cards.iter().map(Card::id).collect();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "duplicate card id");
    let decks = decks();
    let used: BTreeSet<&str> = decks
        .iter()
        .flat_map(|d| d.cards.iter().map(String::as_str))
        .collect();
    assert_eq!(unique, used, "every card of the file is used by a deck");
}

#[test]
fn decks_share_no_card() {
    let decks = decks();
    let rush: BTreeSet<&str> = deck(&decks, "rush")
        .cards
        .iter()
        .map(String::as_str)
        .collect();
    let bulwark: BTreeSet<&str> = deck(&decks, "bulwark")
        .cards
        .iter()
        .map(String::as_str)
        .collect();
    assert!(
        rush.is_disjoint(&bulwark),
        "shared: {:?}",
        rush.intersection(&bulwark)
    );
}

#[test]
fn starter_cards_do_not_reuse_fixture_ids_with_other_content() {
    // The starter set is distinct from the fixture: the two files are not the same data.
    assert_ne!(CARDS, FIXTURE_CARDS);
}

#[test]
fn heroes_differ_on_at_least_two_parameters() {
    let decks = decks();
    let (a, b) = (&deck(&decks, "rush").hero, &deck(&decks, "bulwark").hero);
    let differing = [
        a.max_time != b.max_time,
        a.effects != b.effects,
        a.starting_hand != b.starting_hand,
        a.initiative != b.initiative,
        a.start_budget != b.start_budget,
        a.health != b.health,
    ]
    .into_iter()
    .filter(|d| *d)
    .count();
    assert!(
        differing >= 2,
        "only {differing} differing hero parameter(s)"
    );
}

#[test]
fn hero_initiatives_differ_and_rush_starts() {
    let decks = decks();
    let (rush, bulwark) = (
        deck(&decks, "rush").hero.initiative,
        deck(&decks, "bulwark").hero.initiative,
    );
    assert_ne!(
        rush, bulwark,
        "an initiative tie would make the first player seed-dependent"
    );
    assert!(rush > bulwark, "rush always starts");
}

#[test]
fn cost_distribution_differs_rush_cheaper() {
    let (r, b) = (resolved("rush"), resolved("bulwark"));
    assert_gap("cost", sum(&r, |s| s.cost), sum(&b, |s| s.cost), 15, true);
}

#[test]
fn force_distribution_differs_rush_stronger() {
    let (r, b) = (resolved("rush"), resolved("bulwark"));
    assert_gap(
        "force",
        sum(&r, |s| s.force),
        sum(&b, |s| s.force),
        5,
        false,
    );
}

#[test]
fn health_distribution_differs_rush_more_fragile() {
    let (r, b) = (resolved("rush"), resolved("bulwark"));
    assert_gap(
        "health",
        sum(&r, |s| s.health),
        sum(&b, |s| s.health),
        30,
        true,
    );
}

#[test]
fn attack_cost_distribution_differs_rush_attacks_cheaper() {
    let (r, b) = (resolved("rush"), resolved("bulwark"));
    assert_gap(
        "attack_cost",
        sum(&r, |s| s.attack_cost),
        sum(&b, |s| s.attack_cost),
        10,
        true,
    );
}

#[test]
fn bulwark_has_provocation() {
    assert!(resolved("bulwark").iter().any(|s| s.provocation));
}

#[test]
fn rush_has_riposte() {
    assert!(resolved("rush").iter().any(|s| s.riposte >= 1));
}

#[test]
fn provocation_and_riposte_are_exclusive_to_one_deck_each() {
    assert!(
        resolved("bulwark").iter().all(|s| s.riposte == 0),
        "bulwark has a riposte card"
    );
    assert!(
        resolved("rush").iter().all(|s| !s.provocation),
        "rush has a provocation card"
    );
}

// Mutations of the JSON text: the parsers must keep refusing what they refuse, which also shows
// the property tests above do not pass vacuously on whatever the parsers accept.

#[test]
fn a_deck_of_29_cards_is_refused() {
    let mut value: serde_json::Value = serde_json::from_str(DECKS).unwrap();
    value[0]["cards"].as_array_mut().unwrap().pop();
    let mutated = value.to_string();
    assert!(parse_decks(&mutated).is_err());
}

#[test]
fn an_unknown_card_in_a_deck_is_refused() {
    let mut value: serde_json::Value = serde_json::from_str(DECKS).unwrap();
    value[1]["cards"][0] = "no_such_card".into();
    let decks = parse_decks(&value.to_string()).expect("shape is still valid");
    assert!(resolve_deck(&decks[1], &cards()).is_err());
}

#[test]
fn a_cost_of_zero_is_refused() {
    let mut value: serde_json::Value = serde_json::from_str(CARDS).unwrap();
    value[0]["cost"] = 0.into();
    assert!(parse_cards(&value.to_string()).is_err());
}
