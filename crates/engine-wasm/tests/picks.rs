//! #128 (task #180) through the JSON boundary: `picks` and `source` of an action are bounded by
//! the engine (never by the deserializer, so the codes stay stable), and every refusal leaves the
//! state and the generator untouched. The binding holds no logic of its own for them.

use duel_cards::{parse_cards, parse_decks, resolve_deck};
use duel_engine::{GameSetup, PlayerSetup, RuleSet};
use duel_engine_wasm::{Duel, MAX_INPUT_BYTES};
use serde_json::{Value, json};

const CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

fn duel() -> Duel {
    let cards = parse_cards(CARDS).unwrap();
    let decks = parse_decks(DECKS).unwrap();
    let side = |i: usize| PlayerSetup {
        hero: decks[i].hero.clone(),
        cards: resolve_deck(&decks[i], &cards).unwrap(),
        card_names: Vec::new(),
    };
    let setup = GameSetup {
        one: side(0),
        two: side(1),
        data_versions: vec!["fixture@1.0.0".into()],
        rules: RuleSet::default(),
    };
    Duel::from_json(&serde_json::to_string(&setup).unwrap(), 5).unwrap()
}

fn code(error: &str) -> String {
    let v: Value = serde_json::from_str(error).expect("errors are JSON");
    v["code"].as_str().expect("a code").to_owned()
}

fn first_play(game: &Duel) -> u64 {
    let legal: Vec<Value> = serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
    legal
        .iter()
        .find_map(|a| a["play"]["card"].as_u64())
        .expect("a card to play")
}

/// The refusal of `action`, and the proof that nothing moved: the legal actions are the same and
/// so is the next applied action's outcome on a fresh duel of the same seed.
fn refusal(game: &mut Duel, action: &Value) -> String {
    let before = game.legal_actions_json().unwrap();
    let error = game.apply_json(&action.to_string()).unwrap_err();
    assert_eq!(game.legal_actions_json().unwrap(), before, "{action}");
    code(&error)
}

#[test]
fn empty_picks_are_accepted() {
    let mut game = duel();
    let card = first_play(&game);
    game.apply_json(&json!({"play": {"card": card, "picks": []}}).to_string())
        .unwrap();
}

#[test]
fn hostile_picks_and_sources_get_their_codes() {
    let mut game = duel();
    let card = first_play(&game);
    let nine = vec![json!({"hero": "One"}); 9];
    assert_eq!(
        refusal(
            &mut game,
            &json!({"play": {"card": card, "picks": [{"hero": "One"}]}})
        ),
        "invalid_picks"
    );
    assert_eq!(
        refusal(
            &mut game,
            &json!({"activate": {"effect": 0, "picks": [{"creature": 3}]}})
        ),
        "invalid_picks"
    );
    assert_eq!(
        refusal(&mut game, &json!({"play": {"card": card, "picks": nine}})),
        "too_many_picks"
    );
    assert_eq!(
        refusal(
            &mut game,
            &json!({"activate": {"effect": 0, "source": 10_000}})
        ),
        "source_not_on_board"
    );
    // The game goes on and the same seed plays the same way afterwards.
    let events = game
        .apply_json(&json!({"play": {"card": card}}).to_string())
        .unwrap();
    let mut twin = duel();
    assert_eq!(
        twin.apply_json(&json!({"play": {"card": card}}).to_string())
            .unwrap(),
        events
    );
}

#[test]
fn the_size_limit_still_comes_first() {
    let mut game = duel();
    let huge = format!(
        r#"{{"play":{{"card":1,"picks":[{}]}}}}"#,
        vec![r#"{"hero":"One"}"#; MAX_INPUT_BYTES / 10].join(",")
    );
    assert!(huge.len() > MAX_INPUT_BYTES);
    assert_eq!(
        code(&game.apply_json(&huge).unwrap_err()),
        "input_too_large"
    );
}
