//! Thin WASM binding: the client runs the exact same rules as the server (local preview,
//! replays). The server stays authoritative; the client never decides an outcome.
//!
//! The surface is deliberately small: a constructor from a JSON setup, a constructor from the
//! data (`from_data`: `cards.json` and `decks.json` strings, decks by identifier, each string
//! bounded to 1 MiB, no disk, plus an optional sixth input `rules_json`, a `RuleSet`: `hero_riposte`
//! acts, `board_limit` has no effect before #171), `legal_actions()` and `apply(action)`, all as JSON strings, plus `start_events()`: the events of the setup
//! (`game_started` with the recorded `data_versions` and, when not the engine default, `rules`,
//! possibly `game_won`), which are public
//! (no hand, pile or card id). Still no accessor for the full state; instead, `view(seat)` gives
//! one seat's view (`duel_engine::player_view`: its own hand, the opponent's hand and the piles
//! only as counts, public boards and discards). The view does not authenticate the seat: this is
//! a local adapter, the client asks for the seat it plays (a server calls `player_view` itself,
//! with the seat of the connection). `legal_actions` and the `drew` events still carry the
//! instance ids of the active hand, which a server would have to redact.
//! `legal_actions_with_time()` lists the same legal actions, in the same order, each with its
//! time cost, whether it hands the token over (`hands_over`, to read together with
//! `ends_game`), the budget after it and whether it destroys the attacker (`attacker_destroyed`,
//! a boolean always present, six keys in all), obtained by a dry run on a clone
//! (`duel_engine::preview_legal_actions`); `legal_actions` and `apply` are unchanged.
//! `computer_action(seat, policy, draw)` is the computer's choice (`duel_ai::choose`): a pure function
//! of the state, the active seat, a policy of the closed list (`"random"`, `"greedy"`, each
//! resolved by `duel_ai::policy_named`) and a caller-supplied `u32`, so no generator lives in the
//! binding and nothing is read from the system. `"greedy"` ignores `draw` (a pure function of the
//! state, the same code as the simulator), `"random"` depends on it.
//! [`Duel`] holds the logic and is testable natively; [`Game`] only converts its errors
//! to `JsError` (which cannot be built off the wasm target).

use duel_ai::{AiError, choose, policy_named, rng_from_draw};
use duel_engine::{
    Action, Event, GameSetup, GameState, PlayerId, RuleSet, apply, game_setup_from_data,
    legal_actions, new_game, player_view, preview_legal_actions,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Inputs larger than this (in bytes) are rejected before any parsing.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

/// The binding's logic, with errors as `{"code","message"}` JSON strings.
pub struct Duel {
    state: GameState,
    /// What `new_game` emitted, kept to be read back (and only that) at any time.
    start_events: Vec<Event>,
}

impl Duel {
    /// Starts a game from a JSON `GameSetup`.
    pub fn from_json(setup_json: &str, seed: u32) -> Result<Self, String> {
        Self::from_setup(parse(setup_json)?, seed)
    }

    /// Starts a game from `cards.json` and `decks.json` strings, the decks chosen by
    /// identifier (`deck_one` plays first seat, `deck_two` the second; the same twice is a
    /// mirror). `data_versions_json` is a JSON array of strings, recorded as is.
    /// `rules_json` is optional: `None` is the engine default rules, `Some` a JSON `RuleSet`
    /// object (a partial object reads the default for each absent field; `""` and `"null"` are
    /// not "absent" but `invalid_json`, nothing is normalised). `hero_riposte` acts (a
    /// non-lethal hit on the hero destroys the attacker); `board_limit` has no effect before
    /// #171; the options are only recorded in `game_started` when they differ from
    /// the default.
    ///
    /// Order of the checks, the first to fail answers: (1) the size of every string present,
    /// the five and `rules_json`, bounded by [`MAX_INPUT_BYTES`] before anything is analysed
    /// (`input_too_large`); (2) `data_versions_json` (`invalid_json`); (3) `rules_json` read
    /// as a `RuleSet`, unknown fields refused (`invalid_json`); (4) `game_setup_from_data`
    /// (`invalid_json`, `invalid_setup`, `unknown_deck`); (5) `new_game`, which revalidates
    /// the rules (`invalid_setup`, e.g. `board_limit` outside 1..=30). Nothing is read from disk.
    pub fn from_data(
        cards_json: &str,
        decks_json: &str,
        deck_one: &str,
        deck_two: &str,
        data_versions_json: &str,
        seed: u32,
        rules_json: Option<&str>,
    ) -> Result<Self, String> {
        for input in [
            cards_json,
            decks_json,
            deck_one,
            deck_two,
            data_versions_json,
        ]
        .into_iter()
        .chain(rules_json)
        {
            bounded(input)?;
        }
        let data_versions: Vec<String> = parse(data_versions_json)?;
        let rules: Option<RuleSet> = rules_json.map(parse).transpose()?;
        let mut setup =
            game_setup_from_data(cards_json, decks_json, deck_one, deck_two, data_versions)
                .map_err(|e| error_json(e.code(), &e.to_string()))?;
        if let Some(rules) = rules {
            setup.rules = rules;
        }
        Self::from_setup(setup, seed)
    }

    fn from_setup(setup: GameSetup, seed: u32) -> Result<Self, String> {
        let (state, start_events) =
            new_game(setup, seed.into()).map_err(|e| error_json(e.code(), &e.to_string()))?;
        Ok(Self {
            state,
            start_events,
        })
    }

    /// The events of the game's start, as a JSON array: the same at every call, whatever
    /// was applied since.
    pub fn start_events_json(&self) -> Result<String, String> {
        to_json(&self.start_events)
    }

    /// What `seat` may see of the game, as JSON: `"One"` or `"Two"` exactly (any other value,
    /// including another case, is `invalid_seat`; the length is bounded like every input).
    /// The view does not authenticate the seat: this is a local adapter, and the caller
    /// decides whose view it asks for. A server must call `duel_engine::player_view` itself
    /// with the seat of the connection, never expose this method.
    pub fn view_json(&self, seat: &str) -> Result<String, String> {
        bounded(seat)?;
        to_json(&player_view(&self.state, parse_seat(seat)?))
    }

    /// The legal actions of the active player, as a JSON array.
    pub fn legal_actions_json(&self) -> Result<String, String> {
        to_json(&legal_actions(&self.state))
    }

    /// The legal actions with their time facts, as a JSON array of
    /// `{"action","cost","hands_over","budget_after","ends_game","attacker_destroyed"}`: same length and order as
    /// [`Duel::legal_actions_json`], computed by the engine by a dry run on a clone (the state
    /// and its generator are untouched, nothing drawn is revealed). `action` is the action as
    /// `legal_actions` returns it, to send back to `apply` unchanged. Empty once the game is over.
    pub fn legal_actions_with_time_json(&self) -> Result<String, String> {
        to_json(&preview_legal_actions(&self.state))
    }

    /// The action the computer plays for `seat`, as JSON, in the form `apply` accepts (never
    /// `"concede"`). `seat` is `"One"` or `"Two"` exactly and must be the active seat; `policy`
    /// belongs to the closed list [`COMPUTER_POLICIES`]; both are bounded to [`MAX_INPUT_BYTES`]
    /// before anything is compared. Checks, in this order: sizes (seat, then policy), seat,
    /// policy, `game_over`, `not_active_seat`, then the choice (`computer_stuck` when nothing but
    /// `Concede` is playable).
    ///
    /// The choice is a pure function of (state, seat, policy, `draw`) (`"greedy"` ignores `draw`,
    /// `"random"` depends on it): `draw` is supplied by the caller, the policy's generator is built fresh from it at every call
    /// ([`duel_ai::rng_from_draw`]), and neither the state nor the engine's generator moves
    /// (`&self`: `apply` stays the only mutation). No entropy is read.
    ///
    /// Like `view`, this does not authenticate the seat: a local trusted adapter for a client
    /// that plays against the computer. A server will never expose it.
    pub fn computer_action_json(
        &self,
        seat: &str,
        policy: &str,
        draw: u32,
    ) -> Result<String, String> {
        bounded(seat)?;
        bounded(policy)?;
        let seat = parse_seat(seat)?;
        if !COMPUTER_POLICIES.contains(&policy) {
            return Err(error_json(
                "invalid_policy",
                &format!(
                    "the computer policy must be one of: {}",
                    COMPUTER_POLICIES.join(", ")
                ),
            ));
        }
        if self.state.winner().is_some() {
            return Err(error_json("game_over", "the game is over"));
        }
        if seat != self.state.active() {
            return Err(error_json(
                "not_active_seat",
                "the seat is not the active player",
            ));
        }
        // Every entry of the list resolves in the AI registry; `None` is a bug, never a panic.
        let policy_fn = policy_named(policy)
            .ok_or_else(|| error_json("internal_error", "the computer policy is not registered"))?;
        let action =
            choose(&self.state, &policy_fn, &mut rng_from_draw(draw)).map_err(ai_error_json)?;
        to_json(&action)
    }

    /// Applies a JSON action; returns the produced events as a JSON array.
    pub fn apply_json(&mut self, action_json: &str) -> Result<String, String> {
        let action: Action = parse(action_json)?;
        let events =
            apply(&mut self.state, action).map_err(|e| error_json(e.code(), &e.to_string()))?;
        to_json(&events)
    }
}

/// The computer policies a client may ask for: a closed list, matched exactly.
pub const COMPUTER_POLICIES: [&str; 2] = ["random", "greedy"];

/// `"One"` or `"Two"` exactly, anything else (another case included) is `invalid_seat`.
fn parse_seat(seat: &str) -> Result<PlayerId, String> {
    match seat {
        "One" => Ok(PlayerId::One),
        "Two" => Ok(PlayerId::Two),
        _ => Err(error_json(
            "invalid_seat",
            "the seat must be exactly \"One\" or \"Two\"",
        )),
    }
}

/// What a failed AI choice looks like to the client. `computer_stuck` cannot be reached through
/// `apply` (the engine ends the game by exhaustion first); the code stays as a safety net.
fn ai_error_json(error: AiError) -> String {
    match error {
        AiError::NoPlayableAction => error_json(
            "computer_stuck",
            "the computer has no legal action other than conceding",
        ),
        AiError::IndexOutOfRange { .. } => error_json("internal_error", &error.to_string()),
    }
}

/// `{"code","message"}`: what every error of this binding looks like.
fn error_json(code: &str, message: &str) -> String {
    serde_json::json!({ "code": code, "message": message }).to_string()
}

/// Rejects an input larger than [`MAX_INPUT_BYTES`], whatever it is (JSON or identifier).
fn bounded(input: &str) -> Result<(), String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(error_json(
            "input_too_large",
            &format!("input is larger than {MAX_INPUT_BYTES} bytes"),
        ));
    }
    Ok(())
}

/// Parses untrusted JSON, after bounding its size.
fn parse<T: serde::de::DeserializeOwned>(input: &str) -> Result<T, String> {
    bounded(input)?;
    serde_json::from_str(input).map_err(|e| error_json("invalid_json", &e.to_string()))
}

fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| error_json("serialization_failed", &e.to_string()))
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

    /// See [`Duel::from_data`]. `rules_json` is optional (`undefined` and `null` both mean
    /// "absent", the engine default rules); `""` is `invalid_json`.
    pub fn from_data(
        cards_json: &str,
        decks_json: &str,
        deck_one: &str,
        deck_two: &str,
        data_versions_json: &str,
        seed: u32,
        rules_json: Option<String>,
    ) -> Result<Game, JsError> {
        Duel::from_data(
            cards_json,
            decks_json,
            deck_one,
            deck_two,
            data_versions_json,
            seed,
            rules_json.as_deref(),
        )
        .map(Game)
        .map_err(|e| JsError::new(&e))
    }

    pub fn start_events(&self) -> Result<String, JsError> {
        self.0.start_events_json().map_err(|e| JsError::new(&e))
    }

    /// What `seat` ("One" or "Two") may see of the game, as JSON. WARNING: this does not
    /// authenticate the seat, whoever calls it chooses whose view it gets. It is a local
    /// adapter for a client that plays that seat; a server must call `player_view` itself with
    /// the seat of the connection and never expose `Game.view`.
    pub fn view(&self, seat: &str) -> Result<String, JsError> {
        self.0.view_json(seat).map_err(|e| JsError::new(&e))
    }

    pub fn legal_actions(&self) -> Result<String, JsError> {
        self.0.legal_actions_json().map_err(|e| JsError::new(&e))
    }

    pub fn legal_actions_with_time(&self) -> Result<String, JsError> {
        self.0
            .legal_actions_with_time_json()
            .map_err(|e| JsError::new(&e))
    }

    /// The action the computer plays for `seat` ("One" or "Two", the active seat), under the
    /// closed policy list ("random", "greedy"), as the JSON `apply` accepts, never "concede". `draw` is the
    /// `u32` the caller supplies for this one action; the same inputs give the same action, and
    /// neither the state nor the engine's generator moves. WARNING: like `view`, this does not
    /// authenticate the seat: a local adapter for a client that plays against the computer, a
    /// server must never expose `Game.computer_action`.
    pub fn computer_action(&self, seat: &str, policy: &str, draw: u32) -> Result<String, JsError> {
        self.0
            .computer_action_json(seat, policy, draw)
            .map_err(|e| JsError::new(&e))
    }

    pub fn apply(&mut self, action_json: &str) -> Result<String, JsError> {
        self.0.apply_json(action_json).map_err(|e| JsError::new(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use duel_cards::{parse_cards, parse_decks, resolve_deck};
    use duel_engine::{GameSetup, PlayerSetup, RuleSet};
    use serde_json::Value;

    fn setup_value() -> Value {
        let cards = parse_cards(include_str!("../../../data/fixture/1.0.0/cards.json")).unwrap();
        let decks = parse_decks(include_str!("../../../data/fixture/1.0.0/decks.json")).unwrap();
        let side = |i: usize| PlayerSetup {
            hero: decks[i].hero.clone(),
            cards: resolve_deck(&decks[i], &cards).unwrap(),
            card_names: Vec::new(),
        };
        serde_json::to_value(GameSetup {
            one: side(0),
            two: side(1),
            data_versions: vec!["fixture@1.0.0".into()],
            rules: RuleSet::default(),
        })
        .unwrap()
    }

    fn duel() -> Duel {
        Duel::from_json(&setup_value().to_string(), 1).unwrap()
    }

    fn message(err: &str) -> String {
        let v: Value = serde_json::from_str(err).unwrap();
        v["message"].as_str().unwrap().to_string()
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
    fn legal_actions_start_with_a_play_per_card_in_hand_and_play_applies() {
        let mut game = duel();
        let actions: Value = serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
        let first = actions[0].clone();
        assert!(first.get("play").is_some(), "{actions}");
        let events: Value =
            serde_json::from_str(&game.apply_json(&first.to_string()).unwrap()).unwrap();
        assert!(events[0].get("played").is_some(), "{events}");
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

    // ---- S3-T1: the start events and the recorded data versions ---------------------------

    fn start_events(game: &Duel) -> Value {
        serde_json::from_str(&game.start_events_json().unwrap()).unwrap()
    }

    #[test]
    fn start_events_hold_game_started_with_the_versions_of_the_setup() {
        let game = duel();
        let events = start_events(&game);
        let started = &events[0]["game_started"];
        assert_eq!(
            started["data_versions"],
            serde_json::json!(["fixture@1.0.0"])
        );
        // `first` and `budget` are the ones the engine produced for this setup and seed.
        let setup: GameSetup = serde_json::from_value(setup_value()).unwrap();
        let (_, expected) = new_game(setup, 1).unwrap();
        assert_eq!(
            game.start_events_json().unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
        assert_eq!(
            started["first"],
            serde_json::to_value(match &expected[0] {
                duel_engine::Event::GameStarted { first, .. } => first,
                other => panic!("{other:?}"),
            })
            .unwrap()
        );
    }

    #[test]
    fn start_events_are_the_same_at_every_call_and_after_later_actions() {
        let mut game = duel();
        let first = game.start_events_json().unwrap();
        assert_eq!(game.start_events_json().unwrap(), first);
        game.apply_json(r#"{"activate":{"effect":0}}"#).unwrap();
        let actions: Value = serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
        game.apply_json(&actions[0].to_string()).unwrap();
        assert_eq!(game.start_events_json().unwrap(), first);
        game.apply_json(r#""concede""#).unwrap();
        assert_eq!(game.start_events_json().unwrap(), first);
        assert!(!start_events(&game).as_array().unwrap().is_empty());
    }

    #[test]
    fn an_empty_version_list_leaves_the_key_out_of_game_started() {
        let mut v = setup_value();
        v["data_versions"] = serde_json::json!([]);
        let game = Duel::from_json(&v.to_string(), 1).unwrap();
        let json = game.start_events_json().unwrap();
        assert!(!json.contains("data_versions"), "{json}");
        assert!(json.starts_with(r#"[{"game_started":{"first":""#), "{json}");
    }

    #[test]
    fn a_game_lost_by_exhaustion_at_setup_shows_game_won_among_the_start_events() {
        let mut v = setup_value();
        v["one"]["hero"]["effects"] = serde_json::json!([]);
        v["one"]["hero"]["starting_hand"] = 0.into();
        v["one"]["hero"]["initiative"] = 99.into();
        let game = Duel::from_json(&v.to_string(), 1).unwrap();
        let events = start_events(&game);
        assert!(events[0].get("game_started").is_some(), "{events}");
        assert!(events[1].get("game_won").is_some(), "{events}");
        assert_eq!(
            events[0]["game_started"]["data_versions"],
            serde_json::json!(["fixture@1.0.0"])
        );
    }

    #[test]
    fn from_json_reads_the_rules_key_of_a_setup() {
        let default = RuleSet::default();
        let rules = RuleSet {
            hero_riposte: !default.hero_riposte,
            board_limit: Some(5),
        };
        let mut v = setup_value();
        v["rules"] = serde_json::to_value(rules).unwrap();
        let events = Duel::from_json(&v.to_string(), 1)
            .unwrap()
            .start_events_json()
            .unwrap();
        let suffix = format!(",\"rules\":{}}}}}]", serde_json::to_string(&rules).unwrap());
        assert!(events.ends_with(&suffix), "{events}");
        // An out-of-range option is revalidated by `new_game`.
        v["rules"] = serde_json::json!({ "board_limit": 31 });
        let err = Duel::from_json(&v.to_string(), 1).err().unwrap();
        assert_eq!(code(&err), "invalid_setup");
    }

    #[test]
    fn the_data_versions_key_stays_mandatory_in_a_setup() {
        let mut v = setup_value();
        v.as_object_mut().unwrap().remove("data_versions");
        assert_eq!(
            code(&Duel::from_json(&v.to_string(), 1).err().unwrap()),
            "invalid_json"
        );
    }

    #[test]
    fn start_events_never_expose_hidden_information() {
        let events = start_events(&duel());
        let text = events.to_string();
        for hidden in ["hand", "deck", "card", "drew"] {
            assert!(!text.contains(hidden), "{text}");
        }
    }

    // ---- #142: the computer's action, chosen by the engine's AI --------------------------

    const STARTER_CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
    const STARTER_DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");

    fn starter(seed: u32) -> Duel {
        Duel::from_data(
            STARTER_CARDS,
            STARTER_DECKS,
            "rush",
            "bulwark",
            r#"["starter@1.1.0"]"#,
            seed,
            None,
        )
        .unwrap()
    }

    fn active_seat(game: &Duel) -> &'static str {
        let view: Value = serde_json::from_str(&game.view_json("One").unwrap()).unwrap();
        match view["active"].as_str().unwrap() {
            "One" => "One",
            "Two" => "Two",
            other => panic!("{other}"),
        }
    }

    fn is_over(game: &Duel) -> bool {
        let view: Value = serde_json::from_str(&game.view_json("One").unwrap()).unwrap();
        !view["result"].is_null()
    }

    fn other(seat: &str) -> &'static str {
        if seat == "One" { "Two" } else { "One" }
    }

    #[test]
    fn computer_action_is_accepted_by_apply_and_is_never_concede_over_200_games() {
        for policy in COMPUTER_POLICIES {
            for seed in 0..200u32 {
                let mut game = starter(seed);
                let mut draw = seed.wrapping_mul(2_654_435_761);
                let mut steps = 0;
                while !is_over(&game) {
                    let seat = active_seat(&game);
                    draw = draw.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let action = game.computer_action_json(seat, policy, draw).unwrap();
                    assert_ne!(action, r#""concede""#, "{policy} seed {seed}");
                    let legal: Value =
                        serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
                    let parsed: Value = serde_json::from_str(&action).unwrap();
                    assert!(
                        legal.as_array().unwrap().contains(&parsed),
                        "{policy} seed {seed}: {action}"
                    );
                    game.apply_json(&action).unwrap();
                    steps += 1;
                    assert!(steps < 2000, "{policy} seed {seed}: the game does not end");
                }
            }
        }
    }

    #[test]
    fn computer_action_is_a_pure_function_of_state_seat_policy_and_draw() {
        let game = starter(3);
        let seat = active_seat(&game);
        for policy in COMPUTER_POLICIES {
            for draw in [0, 1, 7, 99, u32::MAX] {
                let a = game.computer_action_json(seat, policy, draw).unwrap();
                let b = game.computer_action_json(seat, policy, draw).unwrap();
                assert_eq!(a, b, "{policy}");
            }
        }
    }

    #[test]
    fn computer_action_touches_neither_the_state_nor_the_generator() {
        let mut probed = starter(9);
        let mut control = starter(9);
        let seat = active_seat(&probed);
        let (legal, view) = (
            probed.legal_actions_json().unwrap(),
            probed.view_json(seat).unwrap(),
        );
        for policy in COMPUTER_POLICIES {
            for draw in 0..50 {
                probed.computer_action_json(seat, policy, draw).unwrap();
            }
        }
        assert_eq!(probed.legal_actions_json().unwrap(), legal);
        assert_eq!(probed.view_json(seat).unwrap(), view);
        assert_eq!(
            probed.legal_actions_with_time_json().unwrap(),
            control.legal_actions_with_time_json().unwrap()
        );
        // The next events (the draws consume the engine's generator) are identical to a game
        // that never asked the computer anything.
        let draw_effect = r#"{"activate":{"effect":0}}"#;
        assert_eq!(
            probed.apply_json(draw_effect).unwrap(),
            control.apply_json(draw_effect).unwrap()
        );
        assert_eq!(
            probed.view_json(seat).unwrap(),
            control.view_json(seat).unwrap()
        );
    }

    #[test]
    fn computer_action_refuses_a_seat_that_is_not_exactly_one_or_two() {
        let game = starter(1);
        for bad in ["one", "TWO", "", " One", "One ", "Three", "0"] {
            let err = game.computer_action_json(bad, "random", 0).unwrap_err();
            assert_eq!(code(&err), "invalid_seat", "{bad:?}");
        }
    }

    #[test]
    fn computer_action_refuses_a_policy_outside_the_closed_list() {
        let game = starter(1);
        let seat = active_seat(&game);
        for bad in [
            "unknown",
            "Random",
            "Greedy",
            "",
            "random ",
            "greedy\0",
            "noisy",
            "defensive",
        ] {
            let err = game.computer_action_json(seat, bad, 0).unwrap_err();
            assert_eq!(code(&err), "invalid_policy", "{bad:?}");
            let message = message(&err);
            assert!(
                message.contains("random") && message.contains("greedy"),
                "{message}"
            );
        }
    }

    #[test]
    fn computer_action_bounds_the_seat_and_the_policy_before_comparing_them() {
        let game = starter(1);
        let seat = active_seat(&game);
        let huge = "x".repeat(MAX_INPUT_BYTES + 1);
        for policy in COMPUTER_POLICIES {
            assert_eq!(
                code(&game.computer_action_json(&huge, policy, 0).unwrap_err()),
                "input_too_large"
            );
        }
        assert_eq!(
            code(&game.computer_action_json(seat, &huge, 0).unwrap_err()),
            "input_too_large"
        );
        // Both too large, and a huge policy next to an invalid seat: the size comes first.
        assert_eq!(
            code(&game.computer_action_json(&huge, &huge, 0).unwrap_err()),
            "input_too_large"
        );
        assert_eq!(
            code(&game.computer_action_json("nobody", &huge, 0).unwrap_err()),
            "input_too_large"
        );
    }

    #[test]
    fn computer_action_checks_seat_then_policy_then_game_over_then_active_seat() {
        let mut game = starter(1);
        let active = active_seat(&game);
        let idle = other(active);
        // The seat is judged before the policy.
        assert_eq!(
            code(
                &game
                    .computer_action_json("nobody", "unknown", 0)
                    .unwrap_err()
            ),
            "invalid_seat"
        );
        // The policy is judged before whose turn it is.
        assert_eq!(
            code(&game.computer_action_json(idle, "unknown", 0).unwrap_err()),
            "invalid_policy"
        );
        // Whose turn it is, on a running game.
        assert_eq!(
            code(&game.computer_action_json(idle, "random", 0).unwrap_err()),
            "not_active_seat"
        );
        assert_eq!(
            code(&game.computer_action_json(idle, "greedy", 0).unwrap_err()),
            "not_active_seat"
        );
        // A finished game is `game_over` for either seat, before the turn check.
        game.apply_json(r#""concede""#).unwrap();
        for policy in COMPUTER_POLICIES {
            for seat in ["One", "Two"] {
                assert_eq!(
                    code(&game.computer_action_json(seat, policy, 0).unwrap_err()),
                    "game_over",
                    "{policy}"
                );
            }
        }
        // ... but the policy is still judged first.
        assert_eq!(
            code(&game.computer_action_json("One", "unknown", 0).unwrap_err()),
            "invalid_policy"
        );
    }

    #[test]
    fn computer_action_reaches_every_playable_action_over_5000_draws() {
        let game = starter(5);
        let seat = active_seat(&game);
        let legal: Value = serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
        let playable: Vec<String> = legal
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| **a != serde_json::json!("concede"))
            .map(Value::to_string)
            .collect();
        assert!(playable.len() > 1);
        let mut counts = std::collections::BTreeMap::new();
        for draw in 0..5000u32 {
            let action = game.computer_action_json(seat, "random", draw).unwrap();
            *counts.entry(action).or_insert(0u32) += 1;
        }
        assert_eq!(counts.len(), playable.len(), "{counts:?}");
        for action in &playable {
            let n = counts[action];
            // Uniform within a wide margin (5000 / len each).
            let expected = 5000 / u32::try_from(playable.len()).unwrap();
            assert!(
                n > expected / 2 && n < expected * 2,
                "{action}: {n} / {expected}"
            );
        }
    }

    #[test]
    fn computer_policies_all_resolve_in_the_ai_registry() {
        for policy in COMPUTER_POLICIES {
            assert!(duel_ai::policy_named(policy).is_some(), "{policy}");
        }
    }

    /// `n` states of games at different depths (different seeds), all with a playable turn.
    fn sampled_states(n: u32) -> Vec<Duel> {
        let mut states = Vec::new();
        for seed in 0..n {
            let mut game = starter(seed);
            for step in 0..(seed % 17) {
                if is_over(&game) {
                    break;
                }
                let seat = active_seat(&game);
                let action = game
                    .computer_action_json(seat, "random", seed * 31 + step)
                    .unwrap();
                game.apply_json(&action).unwrap();
            }
            if !is_over(&game) {
                states.push(game);
            }
        }
        states
    }

    #[test]
    fn greedy_ignores_the_draw() {
        let states = sampled_states(20);
        assert!(states.len() >= 10);
        for game in &states {
            let seat = active_seat(game);
            let first = game.computer_action_json(seat, "greedy", 0).unwrap();
            for draw in [1, 7, 99, u32::MAX] {
                assert_eq!(
                    game.computer_action_json(seat, "greedy", draw).unwrap(),
                    first
                );
            }
        }
    }

    #[test]
    fn greedy_agrees_with_the_simulator_policy() {
        let states = sampled_states(50);
        assert!(states.len() >= 25);
        for game in &states {
            let seat = active_seat(game);
            let from_binding = game.computer_action_json(seat, "greedy", 0).unwrap();
            let mut rng = rng_from_draw(0);
            let direct = choose(&game.state, &duel_ai::greedy_policy, &mut rng).unwrap();
            assert_eq!(from_binding, to_json(&direct).unwrap());
        }
    }

    /// Manual measurement (#126): `cargo test -p duel-engine-wasm --release -- --ignored --nocapture`.
    #[test]
    #[ignore = "manual timing of the greedy decision"]
    fn measure_greedy_decision_time() {
        let (mut worst, mut total, mut count) =
            (std::time::Duration::ZERO, std::time::Duration::ZERO, 0u32);
        for seed in 0..20u32 {
            let mut game = starter(seed);
            while !is_over(&game) {
                let seat = active_seat(&game);
                let t = std::time::Instant::now();
                let action = game.computer_action_json(seat, "greedy", 0).unwrap();
                let dt = t.elapsed();
                worst = worst.max(dt);
                total += dt;
                count += 1;
                game.apply_json(&action).unwrap();
            }
        }
        println!(
            "greedy: {count} decisions, mean {:?}, worst {worst:?}",
            total / count
        );
    }

    #[test]
    fn ai_errors_map_to_their_binding_codes() {
        // `computer_stuck` is unreachable through `apply` (the engine ends the game by
        // exhaustion as soon as only `Concede` is left), so the mapping is tested on its own.
        assert_eq!(
            code(&ai_error_json(duel_ai::AiError::NoPlayableAction)),
            "computer_stuck"
        );
        assert_eq!(
            code(&ai_error_json(duel_ai::AiError::IndexOutOfRange {
                index: 3,
                len: 2
            })),
            "internal_error"
        );
    }
}
