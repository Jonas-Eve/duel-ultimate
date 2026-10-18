//! The setup JSON of the binding is unchanged by `CardSpec` (#181): a creature with an unknown
//! key is still `invalid_json`, one of four keys is still accepted.

use duel_engine_wasm::Duel;
use serde_json::Value;

const GOLDEN: &str =
    include_str!("../../engine/tests/golden/setup-starter-1.1.0-rush-bulwark.json");

fn code(error: &str) -> String {
    let value: Value = serde_json::from_str(error).unwrap();
    value["code"].as_str().unwrap().to_owned()
}

#[test]
fn an_unknown_card_key_is_invalid_json_at_the_binding() {
    let mut value: Value = serde_json::from_str(GOLDEN).unwrap();
    value["one"]["cards"][0]["bonus"] = Value::from(9);
    let error = Duel::from_json(&value.to_string(), 1).err().unwrap();
    assert_eq!(code(&error), "invalid_json");
}

#[test]
fn the_golden_setup_starts_a_game_at_the_binding() {
    assert!(Duel::from_json(GOLDEN, 1).is_ok());
}

#[test]
fn an_out_of_bounds_card_is_invalid_setup_at_the_binding() {
    let mut value: Value = serde_json::from_str(GOLDEN).unwrap();
    value["one"]["cards"][0]["health"] = Value::from(10_001);
    let error = Duel::from_json(&value.to_string(), 1).err().unwrap();
    assert_eq!(code(&error), "invalid_setup");
}

#[test]
fn an_immobile_card_in_a_setup_is_invalid_setup_for_now() {
    // Temporary (#206, lifted by task 3 of #137): the engine refuses what it cannot play yet.
    let mut value: Value = serde_json::from_str(GOLDEN).unwrap();
    let card = value["one"]["cards"][0].as_object_mut().unwrap();
    card.remove("force");
    card.remove("attack_cost");
    card.insert("immobile".into(), Value::Bool(true));
    let error = Duel::from_json(&value.to_string(), 1).err().unwrap();
    assert_eq!(code(&error), "invalid_setup");
}

#[test]
fn a_tagged_card_in_a_setup_is_invalid_setup_for_now() {
    let mut value: Value = serde_json::from_str(GOLDEN).unwrap();
    value["one"]["cards"][0]["tags"] = serde_json::json!(["bâtiment"]);
    let error = Duel::from_json(&value.to_string(), 1).err().unwrap();
    assert_eq!(code(&error), "invalid_setup");
}
