//! `PlayerSetup.cards` is a list of `CardSpec` (story #128, task #181): `new_game` still
//! revalidates every card, and the setup JSON still reads creatures of four or six keys and
//! refuses unknown ones, exactly as it did with `CreatureStats`.

use duel_cards::{CardSpec, CardsError, MAX_STAT};
use duel_engine::{GameSetup, SetupError, game_setup_from_data, new_game};
use serde_json::Value;

const CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
const DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");
const GOLDEN: &str = include_str!("golden/setup-starter-1.1.0-rush-bulwark.json");

fn setup() -> GameSetup {
    game_setup_from_data(CARDS, DECKS, "rush", "bulwark", vec![]).unwrap()
}

fn spec_mut(setup: &mut GameSetup, index: usize) -> &mut duel_cards::CreatureStats {
    match &mut setup.one.cards[index] {
        CardSpec::Creature(stats) => stats,
    }
}

#[test]
fn new_game_revalidates_every_card_spec() {
    // Out of bounds on each field, in the first and in the last card of the second side.
    for (field, expected) in [
        (
            "cost",
            CardsError::StatTooHigh {
                field: "cost",
                value: MAX_STAT + 1,
                max: MAX_STAT,
            },
        ),
        (
            "force",
            CardsError::StatTooHigh {
                field: "force",
                value: MAX_STAT + 1,
                max: MAX_STAT,
            },
        ),
        (
            "attack_cost",
            CardsError::StatTooHigh {
                field: "attack_cost",
                value: MAX_STAT + 1,
                max: MAX_STAT,
            },
        ),
        (
            "health",
            CardsError::StatTooHigh {
                field: "health",
                value: MAX_STAT + 1,
                max: MAX_STAT,
            },
        ),
        (
            "riposte",
            CardsError::StatTooHigh {
                field: "riposte",
                value: MAX_STAT + 1,
                max: MAX_STAT,
            },
        ),
    ] {
        let mut bad = setup();
        let stats = spec_mut(&mut bad, 7);
        match field {
            "cost" => stats.cost = MAX_STAT + 1,
            "force" => stats.force = MAX_STAT + 1,
            "attack_cost" => stats.attack_cost = MAX_STAT + 1,
            "health" => stats.health = MAX_STAT + 1,
            _ => stats.riposte = MAX_STAT + 1,
        }
        let error = new_game(bad, 1).unwrap_err();
        assert_eq!(error, SetupError::Invalid(expected), "{field}");
        assert_eq!(error.code(), "invalid_setup");
    }
    let mut zero = setup();
    spec_mut(&mut zero, 0).force = 0;
    assert_eq!(
        new_game(zero, 1).unwrap_err(),
        SetupError::Invalid(CardsError::NonPositiveStat { field: "force" })
    );
    let mut second_side = setup();
    match &mut second_side.two.cards[29] {
        CardSpec::Creature(stats) => stats.cost = 0,
    }
    assert_eq!(
        new_game(second_side, 1).unwrap_err(),
        SetupError::Invalid(CardsError::CostBelowOne { field: "cost" })
    );
}

#[test]
fn a_setup_json_with_four_key_creatures_is_accepted() {
    let mut value: Value = serde_json::from_str(GOLDEN).unwrap();
    for side in ["one", "two"] {
        for card in value[side]["cards"].as_array_mut().unwrap() {
            let card = card.as_object_mut().unwrap();
            // The legacy starter has no provocation or riposte on some cards; the four-key form
            // is only equivalent for those.
            if card["provocation"] == Value::Bool(false) && card["riposte"] == 0 {
                card.remove("provocation");
                card.remove("riposte");
            }
        }
    }
    let four: GameSetup = serde_json::from_value(value).unwrap();
    assert_eq!(four, setup_with_versions());
    assert!(new_game(four, 3).is_ok());
}

fn setup_with_versions() -> GameSetup {
    game_setup_from_data(
        CARDS,
        DECKS,
        "rush",
        "bulwark",
        vec!["starter@1.1.0".into()],
    )
    .unwrap()
}

#[test]
fn a_setup_json_with_an_unknown_card_key_is_invalid_json_as_before() {
    let mut value: Value = serde_json::from_str(GOLDEN).unwrap();
    value["two"]["cards"][4]["bonus"] = Value::from(9);
    let error = serde_json::from_value::<GameSetup>(value).unwrap_err();
    assert!(
        error.to_string().contains("unknown field `bonus`"),
        "{error}"
    );
}
