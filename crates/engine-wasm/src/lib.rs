//! Thin WASM binding: the client runs the exact same rules as the server (local preview,
//! replays). The server stays authoritative; the client never decides an outcome.

use duel_engine::{Action, GameState, apply};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Game(GameState);

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Game {
        Game(GameState::new(seed.into()))
    }

    pub fn turn(&self) -> u32 {
        self.0.turn
    }

    pub fn end_turn(&mut self) -> Result<(), JsError> {
        apply(&mut self.0, Action::EndTurn)
            .map(|_| ())
            .map_err(|e| JsError::new(&e.to_string()))
    }
}
