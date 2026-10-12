//! Thin WASM binding: the client runs the exact same rules as the server (local preview,
//! replays). The server stays authoritative; the client never decides an outcome.
//!
//! The surface is deliberately small: a constructor from a JSON setup, `legal_actions()`
//! and `apply(action)`, all as JSON strings. There is no accessor for the full state, so
//! nothing hidden (the opponent's hand, the piles) can leak through this binding.
//! [`Duel`] holds the logic and is testable natively; [`Game`] only converts its errors
//! to `JsError` (which cannot be built off the wasm target).

// Red-phase skeleton: bodies are `todo!()` until the feat commit.
#![allow(dead_code)]

use duel_engine::GameState;
use wasm_bindgen::prelude::*;

/// Inputs larger than this (in bytes) are rejected before any parsing.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

/// The binding's logic, with errors as `{"code","message"}` JSON strings.
pub struct Duel(GameState);

impl Duel {
    /// Starts a game from a JSON `GameSetup`.
    pub fn from_json(_setup_json: &str, _seed: u32) -> Result<Self, String> {
        todo!()
    }

    /// The legal actions of the active player, as a JSON array.
    pub fn legal_actions_json(&self) -> Result<String, String> {
        todo!()
    }

    /// Applies a JSON action; returns the produced events as a JSON array.
    pub fn apply_json(&mut self, _action_json: &str) -> Result<String, String> {
        todo!()
    }
}

#[wasm_bindgen]
pub struct Game(Duel);

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new(setup_json: &str, seed: u32) -> Result<Game, JsError> {
        Duel::from_json(setup_json, seed)
            .map(Game)
            .map_err(|e| JsError::new(&e))
    }

    pub fn legal_actions(&self) -> Result<String, JsError> {
        self.0.legal_actions_json().map_err(|e| JsError::new(&e))
    }

    pub fn apply(&mut self, action_json: &str) -> Result<String, JsError> {
        self.0.apply_json(action_json).map_err(|e| JsError::new(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use duel_cards::{parse_cards, parse_decks, resolve_deck};
    use duel_engine::{GameSetup, PlayerSetup};
    use serde_json::Value;

    fn setup_value() -> Value {
        let cards = parse_cards(include_str!("../../../data/fixture/1.0.0/cards.json")).unwrap();
        let decks = parse_decks(include_str!("../../../data/fixture/1.0.0/decks.json")).unwrap();
        let side = |i: usize| PlayerSetup {
            hero: decks[i].hero.clone(),
            cards: resolve_deck(&decks[i], &cards).unwrap(),
        };
        serde_json::to_value(GameSetup {
            one: side(0),
            two: side(1),
            data_versions: vec!["fixture@1.0.0".into()],
        })
        .unwrap()
    }

    fn duel() -> Duel {
        Duel::from_json(&setup_value().to_string(), 1).unwrap()
    }

    fn code(err: &str) -> String {
        let v: Value = serde_json::from_str(err).expect("errors are JSON");
        assert!(
            v["message"].as_str().is_some_and(|m| !m.is_empty()),
            "{err}"
        );
        v["code"].as_str().expect("a code").to_owned()
    }

    #[test]
    fn a_valid_setup_starts_a_game_with_legal_actions() {
        let game = duel();
        let actions: Value = serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
        let actions = actions.as_array().unwrap();
        assert!(actions.contains(&serde_json::json!({"activate": {"effect": 0}})));
        assert_eq!(actions.last(), Some(&serde_json::json!("concede")));
    }

    #[test]
    fn applying_an_action_returns_the_events() {
        let mut game = duel();
        let events: Value =
            serde_json::from_str(&game.apply_json(r#"{"activate":{"effect":0}}"#).unwrap())
                .unwrap();
        assert!(events[0].get("effect_activated").is_some(), "{events}");
    }

    #[test]
    fn same_seed_same_events() {
        let run = |seed| {
            let mut g = Duel::from_json(&setup_value().to_string(), seed).unwrap();
            g.apply_json(r#"{"activate":{"effect":0}}"#).unwrap()
        };
        assert_eq!(run(5), run(5));
    }

    #[test]
    fn broken_json_is_an_error_not_a_panic() {
        for bad in ["", "{", "null", "[]", r#"{"one":1}"#, "\u{0}", "{\"one\":"] {
            assert_eq!(
                code(&Duel::from_json(bad, 1).err().unwrap()),
                "invalid_json"
            );
        }
        let mut game = duel();
        for bad in [
            "",
            "{",
            "null",
            "42",
            r#"{"activate":{"effect":-1}}"#,
            r#""jump""#,
        ] {
            assert_eq!(code(&game.apply_json(bad).unwrap_err()), "invalid_json");
        }
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let mut v = setup_value();
        v["bonus"] = 1.into();
        assert_eq!(
            code(&Duel::from_json(&v.to_string(), 1).err().unwrap()),
            "invalid_json"
        );
        let mut v = setup_value();
        v["one"]["hero"]["draw_cost"] = 1.into();
        assert_eq!(
            code(&Duel::from_json(&v.to_string(), 1).err().unwrap()),
            "invalid_json"
        );
        let mut game = duel();
        let err = game
            .apply_json(r#"{"activate":{"effect":0,"extra":true}}"#)
            .unwrap_err();
        assert_eq!(code(&err), "invalid_json");
    }

    #[test]
    fn invalid_game_data_is_reported_with_a_stable_code() {
        let mut v = setup_value();
        v["one"]["hero"]["effects"][0]["cost"] = 0.into();
        let err = Duel::from_json(&v.to_string(), 1).err().unwrap();
        assert_eq!(code(&err), "invalid_setup");
        assert!(err.contains("cost"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected_before_parsing() {
        let huge = " ".repeat(MAX_INPUT_BYTES + 1);
        assert_eq!(
            code(&Duel::from_json(&huge, 1).err().unwrap()),
            "input_too_large"
        );
        let mut game = duel();
        assert_eq!(
            code(&game.apply_json(&huge).unwrap_err()),
            "input_too_large"
        );
        // Exactly at the limit it is the parser's turn.
        let at_limit = " ".repeat(MAX_INPUT_BYTES);
        assert_eq!(
            code(&game.apply_json(&at_limit).unwrap_err()),
            "invalid_json"
        );
    }

    #[test]
    fn a_refused_action_reports_its_code_and_changes_nothing() {
        let mut game = duel();
        let before = game.legal_actions_json().unwrap();
        let err = game
            .apply_json(r#"{"activate":{"effect":99}}"#)
            .unwrap_err();
        assert_eq!(code(&err), "unknown_effect");
        assert_eq!(game.legal_actions_json().unwrap(), before);
    }

    #[test]
    fn after_the_game_is_over_actions_are_refused_and_none_are_legal() {
        let mut game = duel();
        game.apply_json(r#""concede""#).unwrap();
        assert_eq!(game.legal_actions_json().unwrap(), "[]");
        assert_eq!(
            code(&game.apply_json(r#""concede""#).unwrap_err()),
            "game_over"
        );
    }
}
