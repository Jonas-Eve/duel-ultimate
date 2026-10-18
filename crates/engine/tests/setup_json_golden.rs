//! The JSON of a game setup, pinned **before** story #128 replaced `PlayerSetup.cards`
//! (`Vec<CreatureStats>`) by `Vec<CardSpec>`.
//!
//! `golden/setup-starter-1.1.0-rush-bulwark.json` is the `serde_json::to_string` of
//! `game_setup_from_data` for `starter@1.1.0` (`rush` against `bulwark`, with card names), captured
//! on the base. It never changes in the story: the setup JSON of the `engine-wasm` binding stays
//! identical byte for byte.

use duel_engine::{GameSetup, game_setup_from_data};

const CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
const DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");
const GOLDEN: &str = include_str!("golden/setup-starter-1.1.0-rush-bulwark.json");

fn starter_setup() -> GameSetup {
    game_setup_from_data(
        CARDS,
        DECKS,
        "rush",
        "bulwark",
        vec!["starter@1.1.0".into()],
    )
    .expect("the starter set builds a setup")
}

#[test]
fn setup_json_of_starter_1_1_0_is_byte_identical() {
    let json = serde_json::to_string(&starter_setup()).expect("a setup serializes");
    assert_eq!(json, GOLDEN.trim_end_matches('\n'));
}

#[test]
fn setup_json_round_trips() {
    let setup: GameSetup = serde_json::from_str(GOLDEN.trim_end_matches('\n')).expect("reads");
    let again = serde_json::to_string(&setup).expect("a setup serializes");
    assert_eq!(again, GOLDEN.trim_end_matches('\n'));
}
