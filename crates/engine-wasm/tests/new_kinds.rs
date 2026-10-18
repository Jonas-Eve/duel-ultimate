//! #208 through the JSON boundary: a setup with a spell, an immobile unit and tags starts a game
//! (the guard of #206 and #207 is lifted), the spell is never offered nor playable
//! (`unsupported_effect`), the immobile unit never attacks (`attack_force_zero`). The golden
//! setup JSON itself is unchanged (`card_spec_setup.rs`).

use duel_engine_wasm::Duel;
use serde_json::{Value, json};

const GOLDEN: &str =
    include_str!("../../engine/tests/golden/setup-starter-1.1.0-rush-bulwark.json");

fn code(error: &str) -> String {
    let value: Value = serde_json::from_str(error).unwrap();
    value["code"].as_str().unwrap().to_owned()
}

/// The golden setup with, on One's side, a spell, an immobile unit and a tagged unit, named so
/// that the hand tells them apart; the whole deck is in the opening hand.
fn setup() -> Value {
    let mut value: Value = serde_json::from_str(GOLDEN).unwrap();
    value["one"]["hero"]["starting_hand"] = json!(30);
    let cards = value["one"]["cards"].as_array_mut().unwrap();
    cards[0] = json!({"kind": "spell", "cost": 1, "effects": [
        {"trigger": "on_play", "ops": [
            {"op": "draw", "target": {"select": "hero", "side": "own"}, "count": 1}]}]});
    cards[1] = json!({"cost": 1, "health": 4, "immobile": true});
    cards[2]["tags"] = json!(["bâtiment"]);
    let names = value["one"]["card_names"].as_array_mut().unwrap();
    names[0] = json!("zz-spell");
    names[1] = json!("zz-immobile");
    value
}

fn hand_id(game: &Duel, name: &str) -> u64 {
    let view: Value = serde_json::from_str(&game.view_json("One").unwrap()).unwrap();
    view["you"]["hand"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("{name} is in the hand"))["id"]
        .as_u64()
        .unwrap()
}

fn legal(game: &Duel) -> Vec<Value> {
    serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap()
}

#[test]
fn a_game_with_a_spell_an_immobile_unit_and_tags_starts_and_plays() {
    let mut game = Duel::from_json(&setup().to_string(), 3).unwrap();
    let spell = hand_id(&game, "zz-spell");
    let immobile = hand_id(&game, "zz-immobile");

    // The spell is not offered, nor playable: the temporary code, and nothing moved.
    let offered = legal(&game);
    assert!(
        !offered
            .iter()
            .any(|a| a["play"]["card"].as_u64() == Some(spell))
    );
    assert!(
        offered
            .iter()
            .any(|a| a["play"]["card"].as_u64() == Some(immobile))
    );
    let error = game
        .apply_json(&json!({"play": {"card": spell}}).to_string())
        .unwrap_err();
    assert_eq!(code(&error), "unsupported_effect");
    assert_eq!(legal(&game), offered);

    // The immobile unit is summoned like any unit, and never attacks.
    game.apply_json(&json!({"play": {"card": immobile}}).to_string())
        .unwrap();
    assert!(
        !legal(&game)
            .iter()
            .any(|a| a["attack"]["attacker"].as_u64() == Some(immobile))
    );
    let before = legal(&game);
    let attack = json!({"attack": {"attacker": immobile, "target": "hero"}});
    let error = game.apply_json(&attack.to_string()).unwrap_err();
    assert_eq!(code(&error), "attack_force_zero");
    assert_eq!(legal(&game), before);
}
