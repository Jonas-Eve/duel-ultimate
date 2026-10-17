//! `Duel::legal_actions_with_time_json` (what `Game.legal_actions_with_time()` runs): the
//! time preview of the legal actions through the JSON boundary (T3 of #52).

use duel_engine_wasm::Duel;
use serde_json::{Value, json};

const CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

fn game(seed: u32) -> Duel {
    Duel::from_data(
        CARDS,
        DECKS,
        "aggro",
        "control",
        r#"["fixture@1.0.0"]"#,
        seed,
        None,
    )
    .unwrap()
}

fn json_of(text: &str) -> Vec<Value> {
    serde_json::from_str(text).unwrap()
}

fn legal(game: &Duel) -> Vec<Value> {
    json_of(&game.legal_actions_json().unwrap())
}

fn preview(game: &Duel) -> Vec<Value> {
    json_of(&game.legal_actions_with_time_json().unwrap())
}

#[test]
fn the_preview_lists_the_legal_actions_in_the_same_order() {
    let mut game = game(3);
    for _ in 0..30 {
        let actions = legal(&game);
        let previews = preview(&game);
        assert_eq!(
            previews
                .iter()
                .map(|p| p["action"].clone())
                .collect::<Vec<_>>(),
            actions
        );
        let Some(action) = actions.iter().find(|a| **a != json!("concede")) else {
            break;
        };
        game.apply_json(&action.to_string()).unwrap();
    }
}

#[test]
fn an_entry_carries_the_five_fields_and_nothing_else() {
    let game = game(3);
    let previews = preview(&game);
    assert!(previews.len() > 1);
    for entry in &previews {
        let mut keys: Vec<&str> = entry
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["action", "budget_after", "cost", "ends_game", "hands_over"]
        );
    }
    assert!(
        !game
            .legal_actions_with_time_json()
            .unwrap()
            .contains("drew")
    );
}

#[test]
fn the_costs_and_the_handover_come_from_the_engine() {
    let game = game(3);
    let previews = preview(&game);
    let effect = previews
        .iter()
        .find(|p| p["action"] == json!({"activate": {"effect": 0}}))
        .unwrap();
    assert!(effect["cost"].is_u64(), "{effect}");
    assert!(effect["budget_after"].is_u64(), "{effect}");
    let concede = previews.last().unwrap();
    assert_eq!(
        *concede,
        json!({
            "action": "concede",
            "cost": null,
            "hands_over": false,
            "budget_after": null,
            "ends_game": {"winner": "Two", "reason": "concede"}
        })
    );
}

#[test]
fn the_preview_changes_neither_the_actions_nor_what_follows() {
    let mut with = game(5);
    let mut without = game(5);
    for _ in 0..20 {
        let before = with.legal_actions_json().unwrap();
        let _ = with.legal_actions_with_time_json().unwrap();
        assert_eq!(with.legal_actions_json().unwrap(), before);
        assert_eq!(without.legal_actions_json().unwrap(), before);
        let actions = legal(&with);
        let Some(action) = actions.iter().find(|a| **a != json!("concede")) else {
            break;
        };
        assert_eq!(
            with.apply_json(&action.to_string()).unwrap(),
            without.apply_json(&action.to_string()).unwrap()
        );
    }
}

#[test]
fn a_finished_game_has_an_empty_preview() {
    let mut game = game(3);
    game.apply_json("\"concede\"").unwrap();
    assert_eq!(game.legal_actions_with_time_json().unwrap(), "[]");
    assert_eq!(game.legal_actions_json().unwrap(), "[]");
}
