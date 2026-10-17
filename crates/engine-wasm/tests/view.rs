//! `Duel::view_json` (what `Game.view(seat)` runs): the per-seat view through the JSON
//! boundary (S2-T2 of #52). Every card of the game carries a sentinel name `SECRET-<id>` (the
//! id of an instance is its position in the setup: One's deck, then Two's), so that what a
//! view's text mentions can be compared, name by name and id by id, with what the seat may
//! see. The view of the other seat serves as the independent witness of the hidden hand: the
//! test plays both seats, which is exactly what a local client may do.

use duel_cards::{parse_cards, parse_decks};
use duel_engine::player_setup;
use duel_engine::{GameSetup, PlayerSetup, RuleSet};
use duel_engine_wasm::{Duel, MAX_INPUT_BYTES};
use serde_json::Value;
use std::collections::BTreeSet;

const CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");

fn code(err: &str) -> String {
    let v: Value = serde_json::from_str(err).expect("errors are JSON");
    assert!(
        v["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{err}"
    );
    v["code"].as_str().expect("a code").to_owned()
}

fn side(deck: &str, offset: usize) -> PlayerSetup {
    let cards = parse_cards(CARDS).unwrap();
    let decks = parse_decks(DECKS).unwrap();
    let deck = decks.iter().find(|d| d.id == deck).unwrap();
    let mut side = player_setup(deck, &cards).unwrap();
    side.card_names = (0..30).map(|i| format!("SECRET-{}", offset + i)).collect();
    side
}

fn secret_game(one: &str, two: &str, seed: u32) -> Duel {
    let setup = GameSetup {
        one: side(one, 0),
        two: side(two, 30),
        data_versions: vec!["starter@1.0.0".into()],
        rules: RuleSet::default(),
    };
    Duel::from_json(&serde_json::to_string(&setup).unwrap(), seed).unwrap()
}

fn view(game: &Duel, seat: &str) -> Value {
    serde_json::from_str(&game.view_json(seat).unwrap()).unwrap()
}

fn keys(value: &Value) -> BTreeSet<String> {
    value.as_object().unwrap().keys().cloned().collect()
}

fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| (*s).to_owned()).collect()
}

fn ids_of(cards: &Value) -> BTreeSet<u64> {
    cards
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_u64().unwrap())
        .collect()
}

/// Every number under an `"id"` key, wherever it sits in the document.
fn all_ids(value: &Value, out: &mut BTreeSet<u64>) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                if key == "id" {
                    out.insert(v.as_u64().unwrap());
                } else {
                    all_ids(v, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| all_ids(v, out)),
        _ => {}
    }
}

fn secret_ids(text: &str) -> BTreeSet<u64> {
    text.split("SECRET-")
        .skip(1)
        .map(|rest| {
            rest.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap()
        })
        .collect()
}

/// The checks a view of `seat` must pass, the view of the other seat being the witness.
fn assert_no_leak(game: &Duel, seat: &str, other: &str, context: &str) {
    let mine = view(game, seat);
    let theirs = view(game, other);
    let text = game.view_json(seat).unwrap();

    assert_eq!(
        keys(&mine),
        set(&["seat", "active", "budget", "result", "you", "opponent"]),
        "{context}"
    );
    assert_eq!(
        keys(&mine["you"]),
        set(&["hero", "hand", "deck_count", "board", "discard"]),
        "{context}"
    );
    assert_eq!(
        keys(&mine["opponent"]),
        set(&["hero", "hand_count", "deck_count", "board", "discard"]),
        "{context}"
    );

    // What the seat may know: its hand, both boards, both discards.
    let mut allowed = ids_of(&mine["you"]["hand"]);
    for zone in [
        &mine["you"]["board"],
        &mine["you"]["discard"],
        &mine["opponent"]["board"],
        &mine["opponent"]["discard"],
    ] {
        allowed.extend(ids_of(zone));
    }
    // The public blocks are the same in both views.
    assert_eq!(
        mine["you"]["board"], theirs["opponent"]["board"],
        "{context}"
    );
    assert_eq!(
        mine["opponent"]["discard"], theirs["you"]["discard"],
        "{context}"
    );

    let mut mentioned = BTreeSet::new();
    all_ids(&mine, &mut mentioned);
    assert_eq!(mentioned, allowed, "{context}");
    assert_eq!(secret_ids(&text), allowed, "{context}: names in {text}");

    // The hidden hand, known from the witness, is absent from the text; so is every pile card
    // (all ids that sit in no hand, board or discard).
    let hidden_hand = ids_of(&theirs["you"]["hand"]);
    assert!(
        mentioned.is_disjoint(&hidden_hand),
        "{context}: the other hand leaked"
    );
    assert_eq!(
        mine["opponent"]["hand_count"].as_u64().unwrap() as usize,
        hidden_hand.len(),
        "{context}"
    );
    let mut known = allowed.clone();
    known.extend(&hidden_hand);
    let piles: BTreeSet<u64> = (0..60).filter(|id| !known.contains(id)).collect();
    assert_eq!(
        piles.len() as u64,
        mine["you"]["deck_count"].as_u64().unwrap()
            + mine["opponent"]["deck_count"].as_u64().unwrap(),
        "{context}: the piles are what is left"
    );
    assert!(
        mentioned.is_disjoint(&piles),
        "{context}: a pile card leaked"
    );
}

fn first_legal_non_concede(game: &Duel, pick: usize) -> Option<String> {
    let actions: Vec<Value> = serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
    let playable: Vec<&Value> = actions.iter().filter(|a| **a != "concede").collect();
    (!playable.is_empty()).then(|| playable[pick % playable.len()].to_string())
}

#[test]
fn neither_seat_learns_the_hidden_hand_or_a_pile_before_and_after_every_apply() {
    let mut opponent_active_seen = false;
    for (one, two) in [("rush", "bulwark"), ("bulwark", "rush"), ("rush", "rush")] {
        for seed in 0..10_u32 {
            let mut game = secret_game(one, two, seed);
            for step in 0..400_usize {
                let active = view(&game, "One")["active"].as_str().unwrap().to_owned();
                opponent_active_seen |= active == "Two";
                assert_no_leak(
                    &game,
                    "One",
                    "Two",
                    &format!("{one}-{two} seed {seed} step {step} One"),
                );
                assert_no_leak(
                    &game,
                    "Two",
                    "One",
                    &format!("{one}-{two} seed {seed} step {step} Two"),
                );
                let Some(action) = first_legal_non_concede(&game, step * 7 + seed as usize) else {
                    break;
                };
                game.apply_json(&action).unwrap();
            }
        }
    }
    assert!(
        opponent_active_seen,
        "views were also taken while the opponent was active"
    );
}

#[test]
fn the_view_follows_the_game_seat_active_budget_and_hero_health() {
    let game = secret_game("rush", "bulwark", 1);
    let (one, two) = (view(&game, "One"), view(&game, "Two"));
    assert_eq!(
        (one["seat"].as_str(), two["seat"].as_str()),
        (Some("One"), Some("Two"))
    );
    assert_eq!(one["active"], two["active"]);
    assert_eq!(one["budget"], two["budget"]);
    assert_eq!(one["result"], Value::Null);
    assert_eq!(one["you"]["hero"]["name"], "Capitaine d'avant-garde");
    assert_eq!(one["you"]["hero"]["health"], 12);
    assert_eq!(one["opponent"]["hero"]["name"], "Gardienne du rempart");
    assert_eq!(one["opponent"]["hero"]["health"], 16);
    assert_eq!(
        one["you"]["hero"]["effects"][0]["effect"],
        serde_json::json!({"kind": "draw", "count": 1})
    );
    assert_eq!(one["you"]["hand"].as_array().unwrap().len(), 5);
    assert_eq!(one["opponent"]["hand_count"], 4);
    assert_eq!(one["you"]["deck_count"], 25);
    assert_eq!(one["opponent"]["deck_count"], 26);
}

#[test]
fn a_conceded_game_has_a_result_in_both_views() {
    let mut game = secret_game("rush", "bulwark", 1);
    game.apply_json(r#""concede""#).unwrap();
    for seat in ["One", "Two"] {
        let result = &view(&game, seat)["result"];
        assert_eq!(result["reason"], "concede");
        assert_eq!(result["winner"], "Two", "Rush starts, so Rush concedes");
    }
}

#[test]
fn any_seat_but_one_or_two_exactly_is_invalid_seat_and_never_a_default() {
    let game = secret_game("rush", "bulwark", 1);
    for bad in [
        "Three", "one", "two", "ONE", "", " One", "One ", "One\u{0}", "1", "null", "Player",
        "\"One\"",
    ] {
        let err = game.view_json(bad).unwrap_err();
        assert_eq!(code(&err), "invalid_seat", "{bad:?}");
    }
    assert!(game.view_json("One").is_ok() && game.view_json("Two").is_ok());
}

#[test]
fn the_seat_is_bounded_like_every_other_input() {
    let game = secret_game("rush", "bulwark", 1);
    let huge = "x".repeat(MAX_INPUT_BYTES + 1);
    assert_eq!(code(&game.view_json(&huge).unwrap_err()), "input_too_large");
    let at_limit = "x".repeat(MAX_INPUT_BYTES);
    assert_eq!(
        code(&game.view_json(&at_limit).unwrap_err()),
        "invalid_seat"
    );
}

#[test]
fn asking_for_a_view_changes_nothing() {
    let mut watched = secret_game("rush", "bulwark", 4);
    let mut blind = secret_game("rush", "bulwark", 4);
    for step in 0..60_usize {
        for seat in ["One", "Two"] {
            let _ = watched.view_json(seat).unwrap();
        }
        assert_eq!(watched.legal_actions_json(), blind.legal_actions_json());
        assert_eq!(watched.start_events_json(), blind.start_events_json());
        let Some(action) = first_legal_non_concede(&blind, step * 3) else {
            break;
        };
        assert_eq!(watched.apply_json(&action), blind.apply_json(&action));
    }
}

#[test]
fn the_view_does_not_carry_the_versions_the_seed_or_the_start_events() {
    let game = secret_game("rush", "bulwark", 9);
    for seat in ["One", "Two"] {
        let text = game.view_json(seat).unwrap();
        for hidden in ["starter@1.0.0", "data_versions", "seed", "game_started"] {
            assert!(!text.contains(hidden), "{hidden} in {text}");
        }
    }
}

#[test]
fn a_view_with_unnamed_cards_has_null_names() {
    let mut setup = GameSetup {
        one: side("rush", 0),
        two: side("bulwark", 30),
        data_versions: vec![],
        rules: RuleSet::default(),
    };
    setup.one.card_names.clear();
    setup.two.card_names.clear();
    let game = Duel::from_json(&serde_json::to_string(&setup).unwrap(), 2).unwrap();
    let v = view(&game, "One");
    let hand = v["you"]["hand"].as_array().unwrap();
    assert!(
        !hand.is_empty() && hand.iter().all(|c| c["name"] == Value::Null),
        "{v}"
    );
}
