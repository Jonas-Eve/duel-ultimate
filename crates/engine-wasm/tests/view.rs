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

// ---- #171: `unavailable`, the reason a card of the hand cannot be played --------------------

/// A game on the starter decks with a board limit of one creature.
fn limited_game(seed: u32) -> Duel {
    let setup = GameSetup {
        one: side("rush", 0),
        two: side("bulwark", 30),
        data_versions: vec!["starter@1.0.0".into()],
        rules: RuleSet {
            board_limit: Some(1),
            ..RuleSet::legacy()
        },
    };
    Duel::from_json(&serde_json::to_string(&setup).unwrap(), seed).unwrap()
}

/// Whether the key appears anywhere in the document.
fn mentions(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(map) => map.contains_key(key) || map.values().any(|v| mentions(v, key)),
        Value::Array(items) => items.iter().any(|v| mentions(v, key)),
        _ => false,
    }
}

/// Plays the first summoning offered to the active seat; `false` when there is none.
fn summon(game: &mut Duel) -> bool {
    let actions: Vec<Value> = serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
    let Some(play) = actions.iter().find(|a| a.get("play").is_some()) else {
        return false;
    };
    game.apply_json(&play.to_string()).unwrap();
    true
}

#[test]
fn a_full_board_says_why_for_its_own_hand_only_in_the_json_of_the_view() {
    let mut game = limited_game(3);
    let first = view(&game, "One")["active"].as_str().unwrap().to_owned();
    let other = if first == "One" { "Two" } else { "One" };
    // Control: nobody is full yet, the key is nowhere.
    for seat in ["One", "Two"] {
        assert!(!mentions(&view(&game, seat), "unavailable"), "{seat}");
    }
    assert!(summon(&mut game));
    // The seat that summoned is full: every card of its hand says why, in the last key.
    let full = view(&game, &first);
    let hand = full["you"]["hand"].as_array().unwrap();
    assert!(!hand.is_empty());
    for card in hand {
        assert_eq!(card["unavailable"], "board_full", "{card}");
    }
    assert!(
        game.view_json(&first).unwrap().contains(concat!(
            r#""polarity":"bonus"}],"picks":[{"role":"target","kind":"hero_or_unit","side":"opposing"}]}],"#,
            r#""unavailable":"board_full"}"#
        )),
        "the reason is the last key, after the effects"
    );
    // The other seat is not full: no key, in its view nor in the opponent block of the first.
    assert!(!mentions(&view(&game, other), "unavailable"));
    assert!(!mentions(&full["opponent"], "unavailable"));
    assert!(!mentions(&full["you"]["board"], "unavailable"));
}

#[test]
fn the_reason_is_a_function_of_the_view_not_of_whose_turn_it_is() {
    let mut game = limited_game(3);
    assert!(summon(&mut game));
    // Whoever holds the token now, the summoner's hand says why in its own view.
    let first = if view(&game, "One")["you"]["board"]
        .as_array()
        .unwrap()
        .is_empty()
    {
        "Two"
    } else {
        "One"
    };
    assert!(mentions(&view(&game, first)["you"]["hand"], "unavailable"));
}

// ---- #209: kind, immobile, tags, effects and the reason of a board unit ----------------------

/// The starter decks of One and Two, plus a spell, an immobile unit and a tagged unit at the top
/// of each side; every tag is `SECRET-<instance id>`, spells carry a unique amount.
fn kinds_game(seed: u32) -> Duel {
    let mut setup = GameSetup {
        one: side("rush", 0),
        two: side("bulwark", 30),
        data_versions: vec!["starter@1.0.0".into()],
        rules: RuleSet::default(),
    };
    let mut value = serde_json::to_value(&setup).unwrap();
    for (seat, offset, amount) in [("one", 0, 4001), ("two", 30, 4242)] {
        value[seat]["hero"]["starting_hand"] = 30.into();
        let cards = value[seat]["cards"].as_array_mut().unwrap();
        cards[0] = serde_json::json!({"kind": "spell", "cost": 1, "tags": [format!("SECRET-{offset}")],
            "effects": [{"trigger": "on_play", "ops": [{"op": "attack",
                "target": {"select": "hero_or_unit", "side": "opposing"}, "amount": amount}]}]});
        cards[1] = serde_json::json!({"cost": 1, "health": 4, "immobile": true,
            "tags": [format!("SECRET-{}", offset + 1)]});
        cards[2]["tags"] = serde_json::json!([format!("SECRET-{}", offset + 2)]);
    }
    setup = serde_json::from_value(value).unwrap();
    Duel::from_json(&serde_json::to_string(&setup).unwrap(), seed).unwrap()
}

fn tag_ids(text: &str) -> BTreeSet<u64> {
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

#[test]
fn view_json_carries_kind_effects_and_tags_for_visible_cards() {
    let game = kinds_game(3);
    let first = view(&game, "One")["active"].as_str().unwrap().to_owned();
    let mine = view(&game, &first);
    let hand = mine["you"]["hand"].as_array().unwrap();
    let spell = hand.iter().find(|c| c["kind"] == "spell").expect("a spell");
    assert!(
        spell["tags"].as_array().unwrap()[0]
            .as_str()
            .unwrap()
            .starts_with("SECRET-")
    );
    assert!(
        spell["effects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|l| l.get("index").is_some())
    );
    assert!(
        hand.iter()
            .any(|c| c["immobile"] == true && c["effects"] == serde_json::json!([]))
    );
    assert!(
        hand.iter()
            .all(|c| c["kind"].is_string() && c["effects"].is_array())
    );
    let plain = hand
        .iter()
        .find(|c| c["kind"] == "unit" && c.get("tags").is_none())
        .expect("a plain unit");
    assert!(
        plain["effects"][0].get("index").is_none(),
        "the attack line has no index"
    );
    assert_eq!(plain["effects"][0]["ops"][0]["op"], "attack");
}

#[test]
fn view_json_of_the_other_seat_has_no_secret_tag() {
    let mut game = kinds_game(3);
    let first = view(&game, "One")["active"].as_str().unwrap().to_owned();
    let other = if first == "One" { "Two" } else { "One" };
    // The active seat summons (a plain legal play); its board is public from then on.
    assert!(summon(&mut game));
    let mine = view(&game, &first);
    let board_ids = ids_of(&mine["you"]["board"]);
    assert!(!board_ids.is_empty());
    let theirs = game.view_json(other).unwrap();
    // The other seat reads the tags of the public cards of the first only, never its hand.
    assert_eq!(
        tag_ids(&theirs)
            .intersection(&ids_of(&mine["you"]["hand"]))
            .count(),
        0
    );
    let public: BTreeSet<u64> = board_ids.iter().copied().collect();
    let seen_of_first: BTreeSet<u64> = tag_ids(&theirs)
        .into_iter()
        .filter(|id| (*id < 30) == (first == "One"))
        .collect();
    assert!(
        seen_of_first.is_subset(&public),
        "{seen_of_first:?} vs {public:?}"
    );
    let first_text = game.view_json(&first).unwrap();
    let other_amount = if first == "One" { "4242" } else { "4001" };
    assert!(
        !first_text.contains(other_amount),
        "a hidden spell of the other seat"
    );
    let own_amount = if first == "One" { "4001" } else { "4242" };
    assert!(first_text.contains(own_amount), "witness: its own hand");
}

#[test]
fn board_unit_unavailable_at_the_binding() {
    // Through the data no unit has a force of 0 without being immobile, so the reason itself is
    // pinned at the engine; here the negative cases cross the JSON boundary.
    let mut game = kinds_game(3);
    assert!(summon(&mut game));
    for seat in ["One", "Two"] {
        let v = view(&game, seat);
        for zone in [&v["you"]["board"], &v["opponent"]["board"]] {
            for card in zone.as_array().unwrap() {
                assert!(card.get("unavailable").is_none(), "{card}");
            }
        }
    }
    // The hand, per card: the spell has its temporary reason, a unit has none (no full board).
    let first = view(&game, "One")["active"].as_str().unwrap().to_owned();
    let mine = view(&game, &first);
    let mut spells = 0;
    for card in mine["you"]["hand"].as_array().unwrap() {
        let expected = (card["kind"] == "spell").then_some("unsupported_effect");
        spells += usize::from(expected.is_some());
        assert_eq!(card["unavailable"].as_str(), expected, "{card}");
    }
    assert_eq!(spells, 1, "the hand holds the spell of the setup");
}
