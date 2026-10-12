//! End-to-end test of story S1 ("a duel playable to victory"), driven exclusively through
//! the JSON boundary of `engine-wasm` (`Duel::from_json` / `legal_actions_json` /
//! `apply_json`), the same one the future web client uses, on the `fixture` data set.
//!
//! There is no state accessor on this boundary, so nothing here peeks at the engine:
//! card ids, the active player and the budget are read from what the JSON exposes (legal
//! actions and events). "State identical after a refusal" is checked with twins: two
//! `Duel`s of the same seed fed the same actions, the refused action going to one only.
//!
//! Defeat by exhaustion cannot happen in play between the real fixture decks (every hero
//! keeps a draw effect and a pile, and no creature dies before S2a), but it is reachable
//! through the JSON boundary by mutating the setup (a hero with no effect and no hand): it
//! is exercised both at setup and as the consequence of a handover.

use duel_cards::{CardsError, parse_cards, parse_decks, resolve_deck};
use duel_engine::{GameSetup, PlayerSetup};
use duel_engine_wasm::{Duel, MAX_INPUT_BYTES};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------

const CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

/// The fixture's first deck against its second, as the JSON `GameSetup` a client sends.
fn fixture() -> Value {
    let cards = parse_cards(CARDS).unwrap();
    let decks = parse_decks(DECKS).unwrap();
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

fn start(setup: &Value, seed: u32) -> Duel {
    Duel::from_json(&setup.to_string(), seed).expect("a valid setup starts")
}

/// Every card of both decks gets the same stats, so costs are known whatever the shuffle.
fn uniform_cards(setup: &mut Value, cost: u32, attack_cost: u32) {
    let stats = json!({"cost": cost, "force": 1, "attack_cost": attack_cost, "health": 1});
    for side in ["one", "two"] {
        setup[side]["cards"] = Value::Array(vec![stats.clone(); 30]);
    }
}

fn draw_effect(cost: u32, count: u32) -> Value {
    json!({"name": "draw", "cost": cost, "effect": {"kind": "draw", "count": count}})
}

/// A controlled setup: One (initiative 2) always starts with `start_budget` 3 and
/// `max_time` 8, Two has `max_time` 5. Every card costs `card_cost`.
fn timing_setup(card_cost: u32, one_effects: Vec<Value>) -> Value {
    let mut setup = fixture();
    uniform_cards(&mut setup, card_cost, 1);
    setup["one"]["hero"]["effects"] = Value::Array(one_effects);
    setup["two"]["hero"]["max_time"] = 5.into();
    setup["two"]["hero"]["start_budget"] = 4.into();
    setup
}

fn legal(game: &Duel) -> Vec<Value> {
    serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap()
}

fn apply(game: &mut Duel, action: &Value) -> Vec<Value> {
    serde_json::from_str(&game.apply_json(&action.to_string()).unwrap()).unwrap()
}

fn error_code(err: &str) -> String {
    let v: Value = serde_json::from_str(err).expect("errors are JSON");
    assert!(
        v["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{err}"
    );
    v["code"].as_str().expect("a code").to_owned()
}

fn refusal(game: &mut Duel, action: &Value) -> String {
    error_code(&game.apply_json(&action.to_string()).unwrap_err())
}

fn setup_error(setup: &Value, seed: u32) -> String {
    error_code(&Duel::from_json(&setup.to_string(), seed).err().unwrap())
}

/// The variant name of an externally tagged JSON value (`{"drew": {..}}` -> `drew`).
fn kind(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.keys().next().expect("a variant").clone(),
        other => panic!("not a variant: {other}"),
    }
}

fn kinds(events: &[Value]) -> Vec<String> {
    events.iter().map(kind).collect()
}

fn find<'a>(events: &'a [Value], name: &str) -> Option<&'a Value> {
    events.iter().find(|e| kind(e) == name).map(|e| &e[name])
}

fn count(events: &[Value], name: &str) -> usize {
    events.iter().filter(|e| kind(e) == name).count()
}

fn has(actions: &[Value], name: &str) -> bool {
    actions.iter().any(|a| kind(a) == name)
}

fn plays(actions: &[Value]) -> Vec<u64> {
    actions
        .iter()
        .filter(|a| kind(a) == "play")
        .map(|a| a["play"]["card"].as_u64().unwrap())
        .collect()
}

fn play(card: u64) -> Value {
    json!({"play": {"card": card}})
}

fn activate(effect: u32) -> Value {
    json!({"activate": {"effect": effect}})
}

/// How many hero effects the active player has: tells the fixture's heroes apart (Aggro
/// has 2, Control 1) without any access to the state.
fn active_effects(game: &Duel) -> usize {
    legal(game).iter().filter(|a| kind(a) == "activate").count()
}

/// `(player, budget)` of the `token_moved` event of an action.
fn token(events: &[Value]) -> (String, u64) {
    let t = find(events, "token_moved").expect("every spent action moves the token");
    (
        t["player"].as_str().unwrap().to_owned(),
        t["budget"].as_u64().unwrap(),
    )
}

/// The scripted policy of the full duels: attack if possible, else summon, else activate.
/// Never concedes (it is always the last legal action).
fn pick(actions: &[Value]) -> Value {
    ["attack", "play", "activate"]
        .iter()
        .find_map(|k| actions.iter().find(|a| kind(a) == *k))
        .expect("an unfinished game always has a non-concede action")
        .clone()
}

/// A named corruption of a valid setup.
type Mutation = (&'static str, fn(&mut Value));

/// One step of a journal: the legal actions offered, the action taken, the events it caused.
type Step = (String, Value, Vec<Value>);

/// Plays a whole fixture duel with the scripted policy (after an opening activation of the
/// hero's first effect, so draws are part of every duel), until no action is legal.
fn run_duel(seed: u32) -> Vec<Step> {
    let mut game = start(&fixture(), seed);
    let mut journal = Vec::new();
    loop {
        let offered = game.legal_actions_json().unwrap();
        let actions: Vec<Value> = serde_json::from_str(&offered).unwrap();
        if actions.is_empty() {
            assert_eq!(offered, "[]");
            return journal;
        }
        let action = if journal.is_empty() {
            activate(0)
        } else {
            pick(&actions)
        };
        let events = apply(&mut game, &action);
        journal.push((offered, action, events));
        assert!(journal.len() < 500, "seed {seed}: the duel does not end");
    }
}

// ---------------------------------------------------------------------------------------
// Case 1: valid setup, invalid data rejected with a stable code
// ---------------------------------------------------------------------------------------

#[test]
fn a_valid_setup_starts_a_duel_with_legal_actions() {
    let game = start(&fixture(), 1);
    let actions = legal(&game);
    assert!(!plays(&actions).is_empty());
    assert!(has(&actions, "activate"));
    assert_eq!(actions.last(), Some(&json!("concede")));
    // No `attack` before something is summoned.
    assert!(!has(&actions, "attack"));
}

#[test]
fn semantically_invalid_data_is_refused_with_invalid_setup() {
    let cases: Vec<Mutation> = vec![
        ("card cost 0", |v| v["one"]["cards"][0]["cost"] = 0.into()),
        ("card attack_cost 0", |v| {
            v["two"]["cards"][3]["attack_cost"] = 0.into()
        }),
        ("card force 0", |v| v["one"]["cards"][1]["force"] = 0.into()),
        ("card health 0", |v| {
            v["one"]["cards"][2]["health"] = 0.into()
        }),
        ("card stat above the maximum", |v| {
            v["one"]["cards"][2]["force"] = 10_001.into()
        }),
        ("effect cost 0", |v| {
            v["one"]["hero"]["effects"][0]["cost"] = 0.into()
        }),
        ("effect count 0", |v| {
            v["two"]["hero"]["effects"][0]["effect"]["count"] = 0.into()
        }),
        ("too many effects", |v| {
            let effect = v["one"]["hero"]["effects"][0].clone();
            v["one"]["hero"]["effects"] = Value::Array(vec![effect; 9]);
        }),
        ("max_time 0", |v| v["one"]["hero"]["max_time"] = 0.into()),
        ("start_budget above max_time", |v| {
            v["two"]["hero"]["start_budget"] = 11.into()
        }),
        ("start_budget 0", |v| {
            v["one"]["hero"]["start_budget"] = 0.into()
        }),
        ("hero health 0", |v| v["one"]["hero"]["health"] = 0.into()),
        ("starting_hand above the deck", |v| {
            v["one"]["hero"]["starting_hand"] = 31.into()
        }),
        ("deck of 29 cards", |v| {
            v["two"]["cards"].as_array_mut().unwrap().pop();
        }),
        ("deck of 31 cards", |v| {
            let card = v["one"]["cards"][0].clone();
            v["one"]["cards"].as_array_mut().unwrap().push(card);
        }),
    ];
    for (name, mutate) in cases {
        let mut setup = fixture();
        mutate(&mut setup);
        assert_eq!(setup_error(&setup, 1), "invalid_setup", "{name}");
    }
}

#[test]
fn malformed_or_hostile_json_is_refused_with_invalid_json() {
    let full = fixture().to_string();
    let mut cut = full.len() / 2;
    while !full.is_char_boundary(cut) {
        cut += 1;
    }
    let truncated = &full[..cut];
    assert_eq!(
        error_code(&Duel::from_json(truncated, 1).err().unwrap()),
        "invalid_json",
        "truncated JSON"
    );

    let cases: Vec<Mutation> = vec![
        ("unknown setup field", |v| v["bonus"] = 1.into()),
        ("unknown hero field", |v| {
            v["one"]["hero"]["draw_cost"] = 1.into()
        }),
        ("unknown card field", |v| {
            v["two"]["cards"][0]["rarity"] = "rare".into()
        }),
        ("unknown effect kind", |v| {
            v["one"]["hero"]["effects"][0]["effect"]["kind"] = "fireball".into()
        }),
        ("a card id where stats are expected", |v| {
            v["one"]["cards"][0] = "ghost".into()
        }),
        ("a negative stat", |v| {
            v["one"]["hero"]["health"] = (-3).into()
        }),
        ("a missing side", |v| {
            v.as_object_mut().unwrap().remove("two");
        }),
    ];
    for (name, mutate) in cases {
        let mut setup = fixture();
        mutate(&mut setup);
        assert_eq!(setup_error(&setup, 1), "invalid_json", "{name}");
    }
}

#[test]
fn an_oversized_input_is_refused_before_parsing() {
    let huge = " ".repeat(MAX_INPUT_BYTES + 1);
    assert_eq!(
        error_code(&Duel::from_json(&huge, 1).err().unwrap()),
        "input_too_large"
    );
    let mut game = start(&fixture(), 1);
    assert_eq!(
        error_code(&game.apply_json(&huge).unwrap_err()),
        "input_too_large"
    );
}

#[test]
fn an_unknown_card_in_a_deck_is_refused_when_the_deck_is_resolved() {
    // Unknown ids never reach the JSON boundary: `duel-cards` refuses them first.
    let cards = parse_cards(CARDS).unwrap();
    let mut decks = parse_decks(DECKS).unwrap();
    decks[0].cards[5] = "ghost".into();
    assert!(matches!(
        resolve_deck(&decks[0], &cards),
        Err(CardsError::UnknownCard { card, .. }) if card == "ghost"
    ));
}

// ---------------------------------------------------------------------------------------
// Case 2: who starts
// ---------------------------------------------------------------------------------------

#[test]
fn the_hero_with_the_higher_initiative_starts() {
    // Fixture: Aggro (One, initiative 2, two effects) beats Control (Two, initiative 1).
    for seed in 0..10 {
        assert_eq!(active_effects(&start(&fixture(), seed)), 2, "seed {seed}");
    }
    let mut swapped = fixture();
    swapped["one"]["hero"]["initiative"] = 1.into();
    swapped["two"]["hero"]["initiative"] = 2.into();
    for seed in 0..10 {
        assert_eq!(active_effects(&start(&swapped, seed)), 1, "seed {seed}");
    }
}

#[test]
fn an_initiative_tie_is_broken_by_the_seed_alone() {
    let mut tie = fixture();
    tie["one"]["hero"]["initiative"] = 1.into();
    let first = |seed| active_effects(&start(&tie, seed));
    let outcomes: Vec<usize> = (0..40).map(first).collect();
    // Same seed, same starter (and same whole position).
    for seed in 0..40 {
        assert_eq!(first(seed), outcomes[seed as usize]);
        assert_eq!(
            start(&tie, seed).legal_actions_json().unwrap(),
            start(&tie, seed).legal_actions_json().unwrap()
        );
    }
    // Both heroes do get to start, depending on the seed.
    assert!(
        outcomes.contains(&1) && outcomes.contains(&2),
        "{outcomes:?}"
    );
}

// ---------------------------------------------------------------------------------------
// Case 3: the time rule (c < b, c = b, c > b with the receiver's cap)
// ---------------------------------------------------------------------------------------

#[test]
fn an_effect_activation_follows_the_time_rule() {
    // One has 3 time units, Two caps at 5. Costs 1 / 3 / 5 / 9.
    let effects = vec![
        draw_effect(1, 1),
        draw_effect(3, 1),
        draw_effect(5, 1),
        draw_effect(9, 1),
    ];
    let setup = timing_setup(2, effects);
    let run = |effect| {
        let mut game = start(&setup, 1);
        assert_eq!(active_effects(&game), 4, "One starts");
        let events = apply(&mut game, &activate(effect));
        (events, game)
    };

    // c < b: One keeps the token with b - c.
    let (events, game) = run(0);
    assert_eq!(token(&events), ("One".into(), 2));
    assert_eq!(count(&events, "active_player_changed"), 0);
    assert_eq!(active_effects(&game), 4);

    // c = b: the token crosses and Two receives 1.
    let (events, game) = run(1);
    assert_eq!(token(&events), ("Two".into(), 1));
    assert_eq!(count(&events, "active_player_changed"), 1);
    assert_eq!(
        active_effects(&game),
        1,
        "Two (one effect, not One's four) is active"
    );

    // c > b: the excess plus one (5 - 3 + 1).
    let (events, _) = run(2);
    assert_eq!(token(&events), ("Two".into(), 3));

    // c > b, capped by the receiver's max_time (9 - 3 + 1 = 7 > 5, while One's own cap is 8).
    let (events, _) = run(3);
    assert_eq!(token(&events), ("Two".into(), 5));
}

#[test]
fn a_summoning_follows_the_time_rule() {
    // Every card costs `cost`; One has 3 time units, Two caps at 5.
    let run = |cost| {
        let setup = timing_setup(cost, vec![draw_effect(1, 1)]);
        let mut game = start(&setup, 1);
        let card = plays(&legal(&game))[0];
        let events = apply(&mut game, &play(card));
        assert_eq!(find(&events, "played").unwrap()["cost"], cost);
        events
    };
    let events = run(1); // c < b
    assert_eq!(token(&events), ("One".into(), 2));
    assert_eq!(count(&events, "active_player_changed"), 0);

    let events = run(3); // c = b
    assert_eq!(token(&events), ("Two".into(), 1));
    assert_eq!(count(&events, "active_player_changed"), 1);

    let events = run(6); // c > b: 6 - 3 + 1
    assert_eq!(token(&events), ("Two".into(), 4));

    let events = run(20); // c > b: 18 capped to Two's max_time
    assert_eq!(token(&events), ("Two".into(), 5));
}

// ---------------------------------------------------------------------------------------
// Case 4: a whole duel to victory
// ---------------------------------------------------------------------------------------

#[test]
fn a_full_duel_ends_on_a_hero_down_without_handover_then_nothing_is_legal() {
    for seed in 0..20 {
        let journal = run_duel(seed);
        let all: Vec<&Value> = journal.iter().flat_map(|(_, _, events)| events).collect();
        let names: Vec<String> = all.iter().map(|e| kind(e)).collect();
        for expected in [
            "effect_activated",
            "drew",
            "played",
            "attacked",
            "damaged",
            "token_moved",
            "game_won",
        ] {
            assert!(
                names.iter().any(|n| n == expected),
                "seed {seed}: {expected}"
            );
        }

        // The victory is the very last event, of the very last action, and nothing else is
        // ever won before it.
        assert_eq!(names.iter().filter(|n| *n == "game_won").count(), 1);
        let (_, _, last) = journal.last().unwrap();
        let won = last.last().unwrap();
        assert_eq!(kind(won), "game_won", "seed {seed}");
        assert_eq!(won["game_won"]["reason"], "hero_down", "seed {seed}");

        // The lethal attack stops the game on the spot: no time is spent, no handover.
        assert_eq!(
            kinds(last),
            ["attacked", "damaged", "game_won"],
            "seed {seed}"
        );
        let attacker = last[0]["attacked"]["player"].clone();
        assert_eq!(won["game_won"]["winner"], attacker, "seed {seed}");
        assert_ne!(last[1]["damaged"]["player"], attacker, "seed {seed}");
    }
}

#[test]
fn after_a_full_duel_every_action_is_refused() {
    let mut game = start(&fixture(), 4);
    let mut first = true;
    loop {
        let actions = legal(&game);
        if actions.is_empty() {
            break;
        }
        let action = if first { activate(0) } else { pick(&actions) };
        first = false;
        apply(&mut game, &action);
    }
    assert_eq!(game.legal_actions_json().unwrap(), "[]");
    for action in [
        json!("concede"),
        activate(0),
        play(0),
        json!({"attack": {"attacker": 0, "target": "hero"}}),
    ] {
        assert_eq!(refusal(&mut game, &action), "game_over", "{action}");
    }
    assert_eq!(game.legal_actions_json().unwrap(), "[]");
}

// ---------------------------------------------------------------------------------------
// Case 5: a hero with several effects
// ---------------------------------------------------------------------------------------

/// One: three draw effects (1 card / cost 1, 3 cards / cost 2, 50 cards / cost 4) and a
/// budget large enough never to hand over; `starting_hand` sets the size of the pile.
fn several_effects(starting_hand: u32) -> Value {
    let mut setup = fixture();
    setup["one"]["hero"]["effects"] =
        json!([draw_effect(1, 1), draw_effect(2, 3), draw_effect(4, 50)]);
    setup["one"]["hero"]["max_time"] = 10.into();
    setup["one"]["hero"]["start_budget"] = 10.into();
    setup["one"]["hero"]["starting_hand"] = starting_hand.into();
    setup
}

#[test]
fn each_effect_is_activated_for_its_own_cost_and_draws_its_own_count() {
    for (effect, cost, drawn) in [(0u32, 1u64, 1usize), (1, 2, 3), (2, 4, 26)] {
        let mut game = start(&several_effects(4), 3); // a pile of 26
        let events = apply(&mut game, &activate(effect));
        let activated = find(&events, "effect_activated").unwrap();
        assert_eq!(activated["effect"], effect);
        assert_eq!(activated["cost"], cost);
        assert_eq!(count(&events, "drew"), drawn, "effect {effect}");
        assert_eq!(token(&events), ("One".into(), 10 - cost));
    }
}

#[test]
fn a_draw_larger_than_the_pile_is_truncated_to_what_is_left() {
    let mut game = start(&several_effects(28), 3); // a pile of 2
    let events = apply(&mut game, &activate(1)); // wants 3
    assert_eq!(count(&events, "drew"), 2);
    assert_eq!(find(&events, "effect_activated").unwrap()["cost"], 2);
    // The pile is now empty: no effect is offered or accepted any more.
    assert!(!has(&legal(&game), "activate"));
    assert_eq!(refusal(&mut game, &activate(1)), "effect_not_applicable");
}

#[test]
fn an_unknown_effect_is_refused() {
    let mut game = start(&several_effects(4), 3);
    assert_eq!(refusal(&mut game, &activate(3)), "unknown_effect");
    assert_eq!(refusal(&mut game, &activate(u32::MAX)), "unknown_effect");
    // The effects that do exist are still offered.
    let actions = legal(&game);
    for effect in 0..3 {
        assert!(actions.contains(&activate(effect)));
    }
}

// ---------------------------------------------------------------------------------------
// Case 6: a refused action changes nothing (twins)
// ---------------------------------------------------------------------------------------

/// Applies `action` to both twins and checks they answer the same.
fn apply_both(a: &mut Duel, b: &mut Duel, action: &Value) {
    let ea = a.apply_json(&action.to_string());
    let eb = b.apply_json(&action.to_string());
    assert_eq!(ea, eb);
    ea.unwrap();
}

/// After the refusals served to `a` only, both twins must offer the same actions and then
/// answer the next `n` actions identically (the journal of the following moves).
fn assert_twins_agree(a: &mut Duel, b: &mut Duel, n: usize) {
    assert_eq!(a.legal_actions_json(), b.legal_actions_json());
    for _ in 0..n {
        let actions = legal(b);
        if actions.is_empty() {
            break;
        }
        apply_both(a, b, &pick(&actions));
        assert_eq!(a.legal_actions_json(), b.legal_actions_json());
    }
}

#[test]
fn refused_actions_leave_the_state_identical() {
    let setup = fixture();
    let (mut a, mut b) = (start(&setup, 7), start(&setup, 7));
    let stale = plays(&legal(&a))[0];

    // Activating Aggro's second effect (cost 5 > budget 3) hands the token to Control.
    apply_both(&mut a, &mut b, &activate(1));
    assert_eq!(active_effects(&a), 1, "it is Control's turn");

    // Refused on `a` only, every way an action can be wrong.
    assert_eq!(refusal(&mut a, &activate(99)), "unknown_effect");
    assert_eq!(refusal(&mut a, &play(stale)), "card_not_in_hand"); // the other player's card
    assert_eq!(refusal(&mut a, &play(u64::MAX >> 40)), "card_not_in_hand");
    assert_eq!(
        refusal(
            &mut a,
            &json!({"attack": {"attacker": stale, "target": "hero"}})
        ),
        "attacker_not_on_board"
    );
    for bad in ["", "{", "null", r#"{"play":{"card":-1}}"#, r#""jump""#] {
        assert_eq!(
            error_code(&a.apply_json(bad).unwrap_err()),
            "invalid_json",
            "{bad:?}"
        );
    }
    let huge = " ".repeat(MAX_INPUT_BYTES + 1);
    assert_eq!(
        error_code(&a.apply_json(&huge).unwrap_err()),
        "input_too_large"
    );

    assert_twins_agree(&mut a, &mut b, 8);
}

#[test]
fn a_refused_draw_from_an_empty_pile_leaves_the_state_identical() {
    let mut setup = fixture();
    setup["one"]["hero"]["starting_hand"] = 30.into(); // the whole deck in hand, an empty pile
    let (mut a, mut b) = (start(&setup, 2), start(&setup, 2));
    assert!(!has(&legal(&a), "activate"));
    assert_eq!(refusal(&mut a, &activate(0)), "effect_not_applicable");
    assert_eq!(refusal(&mut a, &activate(1)), "effect_not_applicable");
    assert_twins_agree(&mut a, &mut b, 8);
}

// ---------------------------------------------------------------------------------------
// Case 7: determinism
// ---------------------------------------------------------------------------------------

#[test]
fn the_same_seed_gives_the_same_whole_journal() {
    for seed in [0, 1, 42, u32::MAX] {
        assert_eq!(run_duel(seed), run_duel(seed), "seed {seed}");
    }
}

#[test]
fn different_seeds_give_different_journals() {
    let journals: Vec<Vec<Step>> = (0..6).map(run_duel).collect();
    for (i, a) in journals.iter().enumerate() {
        for b in &journals[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Case 8: Concede
// ---------------------------------------------------------------------------------------

#[test]
fn conceding_gives_the_win_to_the_opponent_and_ends_the_game() {
    // One concedes at the start.
    let mut game = start(&fixture(), 1);
    let events = apply(&mut game, &json!("concede"));
    assert_eq!(
        events,
        vec![json!({"game_won": {"winner": "Two", "reason": "concede"}})]
    );
    assert_eq!(game.legal_actions_json().unwrap(), "[]");
    for action in [
        json!("concede"),
        activate(0),
        play(0),
        json!({"attack": {"attacker": 0, "target": "hero"}}),
    ] {
        assert_eq!(refusal(&mut game, &action), "game_over", "{action}");
    }
    assert_eq!(game.legal_actions_json().unwrap(), "[]");

    // Two concedes once the token has crossed: the cause and the winner follow the player.
    let mut game = start(&fixture(), 1);
    apply(&mut game, &activate(1)); // cost 5 > 3: Two is active
    let events = apply(&mut game, &json!("concede"));
    assert_eq!(
        events,
        vec![json!({"game_won": {"winner": "One", "reason": "concede"}})]
    );
    assert_eq!(game.legal_actions_json().unwrap(), "[]");
    assert_eq!(refusal(&mut game, &activate(0)), "game_over");
}

// ---------------------------------------------------------------------------------------
// Case 9: the order of the journal, per action type
// ---------------------------------------------------------------------------------------

#[test]
fn the_journal_of_each_action_type_is_ordered() {
    // One: max_time 10, budget 10, a "draw 2 for 1" effect; every card costs 1 to play and 1
    // to attack with, so nothing hands the token over.
    let mut setup = fixture();
    uniform_cards(&mut setup, 1, 1);
    setup["one"]["hero"]["effects"] = json!([draw_effect(1, 2)]);
    setup["one"]["hero"]["max_time"] = 10.into();
    setup["one"]["hero"]["start_budget"] = 10.into();
    let mut game = start(&setup, 5);

    // Summoning.
    let card = plays(&legal(&game))[0];
    let events = apply(&mut game, &play(card));
    assert_eq!(kinds(&events), ["played", "token_moved"]);
    assert_eq!(
        events[0],
        json!({"played": {"player": "One", "card": card, "cost": 1}})
    );
    assert_eq!(
        events[1],
        json!({"token_moved": {"player": "One", "budget": 9}})
    );

    // Effect activation: the activation, then each card drawn, then the time.
    let events = apply(&mut game, &activate(0));
    assert_eq!(
        kinds(&events),
        ["effect_activated", "drew", "drew", "token_moved"]
    );
    assert_eq!(
        events[0],
        json!({"effect_activated": {"player": "One", "effect": 0, "cost": 1}})
    );
    assert_ne!(events[1]["drew"]["card"], events[2]["drew"]["card"]);

    // Attack with the summoned creature: the attack, the damage, then the time.
    let attack = json!({"attack": {"attacker": card, "target": "hero"}});
    assert!(legal(&game).contains(&attack));
    let events = apply(&mut game, &attack);
    assert_eq!(kinds(&events), ["attacked", "damaged", "token_moved"]);
    assert_eq!(
        events[0],
        json!({"attacked": {"player": "One", "attacker": card, "cost": 1}})
    );
    assert_eq!(
        events[1],
        json!({"damaged": {"player": "Two", "amount": 1}})
    );
    assert_eq!(
        events[2],
        json!({"token_moved": {"player": "One", "budget": 7}})
    );

    // Concede: the victory alone.
    let events = apply(&mut game, &json!("concede"));
    assert_eq!(kinds(&events), ["game_won"]);
}

#[test]
fn a_handover_is_announced_after_the_time_it_causes() {
    // Cards cost 10 and One has 3 time units: the first summoning crosses the token.
    let setup = timing_setup(10, vec![draw_effect(1, 1)]);
    let mut game = start(&setup, 5);
    let card = plays(&legal(&game))[0];
    let events = apply(&mut game, &play(card));
    assert_eq!(
        kinds(&events),
        ["played", "token_moved", "active_player_changed"]
    );
    assert_eq!(
        events[1],
        json!({"token_moved": {"player": "Two", "budget": 5}})
    );
    assert_eq!(
        events[2],
        json!({"active_player_changed": {"player": "Two"}})
    );
}

// ---------------------------------------------------------------------------------------
// Case 11: defeat by exhaustion (reached through setup mutations)
// ---------------------------------------------------------------------------------------

/// A hero left with nothing to do: no effect and an empty hand (its pile is never used).
fn strip(setup: &mut Value, side: &str) {
    setup[side]["hero"]["effects"] = json!([]);
    setup[side]["hero"]["starting_hand"] = 0.into();
}

fn every_action_is_refused_as_game_over(game: &mut Duel) {
    for action in [
        json!("concede"),
        activate(0),
        play(0),
        json!({"attack": {"attacker": 0, "target": "hero"}}),
    ] {
        assert_eq!(refusal(game, &action), "game_over", "{action}");
    }
    assert_eq!(game.legal_actions_json().unwrap(), "[]");
}

#[test]
fn a_starter_with_nothing_to_do_is_defeated_by_exhaustion_at_setup() {
    // One starts (initiative 2) with no effect and an empty hand: nothing but Concede would
    // be legal, so the game is over before the first action, and Two wins.
    let mut setup = fixture();
    strip(&mut setup, "one");
    let mut game = start(&setup, 1);
    assert_eq!(game.legal_actions_json().unwrap(), "[]");
    every_action_is_refused_as_game_over(&mut game);

    // Same when Two starts (swapped initiative).
    let mut setup = fixture();
    setup["one"]["hero"]["initiative"] = 1.into();
    setup["two"]["hero"]["initiative"] = 2.into();
    strip(&mut setup, "two");
    let mut game = start(&setup, 1);
    assert_eq!(game.legal_actions_json().unwrap(), "[]");
    every_action_is_refused_as_game_over(&mut game);

    // Control: the non-starter being stripped changes nothing, the game is on.
    let mut setup = fixture();
    strip(&mut setup, "two");
    assert!(!legal(&start(&setup, 1)).is_empty());
}

#[test]
fn a_handover_to_a_receiver_with_nothing_to_do_ends_the_game_by_exhaustion() {
    // Two has no effect and an empty hand; One's summoning costs 10 > 3 and crosses the token.
    let mut setup = timing_setup(10, vec![draw_effect(1, 1)]);
    strip(&mut setup, "two");
    let mut game = start(&setup, 5);
    let card = plays(&legal(&game))[0];
    let events = apply(&mut game, &play(card));
    assert_eq!(
        kinds(&events),
        ["played", "token_moved", "active_player_changed", "game_won"]
    );
    assert_eq!(
        events[3],
        json!({"game_won": {"winner": "One", "reason": "exhaustion"}})
    );
    every_action_is_refused_as_game_over(&mut game);

    // A non-crossing action leaves the token with One: nobody is exhausted.
    let mut setup = timing_setup(1, vec![draw_effect(1, 1)]);
    strip(&mut setup, "two");
    let mut game = start(&setup, 5);
    let card = plays(&legal(&game))[0];
    let events = apply(&mut game, &play(card));
    assert_eq!(count(&events, "game_won"), 0);
    assert!(!legal(&game).is_empty());
}

// ---------------------------------------------------------------------------------------
// Opening hands, no automatic draw, no "pass" action
// ---------------------------------------------------------------------------------------

#[test]
fn opening_hands_budgets_and_the_absence_of_auto_draw_or_pass() {
    for (one_starts, starter, other) in [(true, "one", "two"), (false, "two", "one")] {
        let mut setup = fixture();
        if !one_starts {
            setup["one"]["hero"]["initiative"] = 1.into();
            setup["two"]["hero"]["initiative"] = 2.into();
        }
        let hand = |side: &str| setup[side]["hero"]["starting_hand"].as_u64().unwrap() as usize;
        let mut game = start(&setup, 9);

        // Opening hand and starting budget of the starter.
        let actions = legal(&game);
        assert_eq!(plays(&actions).len(), hand(starter), "{starter} hand");
        let budget = setup[starter]["hero"]["start_budget"].as_u64().unwrap();
        let cost = setup[starter]["hero"]["effects"][0]["cost"]
            .as_u64()
            .unwrap();
        let events = apply(&mut game, &activate(0));
        let (who, left) = token(&events);
        if cost < budget {
            assert_eq!(
                (who.to_lowercase().as_str(), left),
                (starter, budget - cost)
            );
        } else {
            let cap = setup[other]["hero"]["max_time"].as_u64().unwrap();
            assert_eq!(
                (who.to_lowercase().as_str(), left),
                (other, (cost - budget + 1).min(cap))
            );
        }

        // Whatever the position, only play / activate / attack / concede are ever offered.
        let allowed = ["play", "activate", "attack", "concede"];
        let mut handover = count(&events, "active_player_changed") == 1;
        let mut last_events = events;
        for _ in 0..60 {
            if handover {
                break;
            }
            let actions = legal(&game);
            assert!(actions.iter().all(|a| allowed.contains(&kind(a).as_str())));
            last_events = apply(&mut game, &pick(&actions));
            handover = count(&last_events, "active_player_changed") == 1;
        }
        // The receiver's hand is untouched by the handover: no automatic draw.
        assert!(handover, "a handover happens");
        // (Any `drew` of the handing-over action belongs to the giver, before the handover.)
        let tail = kinds(&last_events);
        assert_eq!(
            &tail[tail.len() - 2..],
            ["token_moved", "active_player_changed"]
        );
        let actions = legal(&game);
        assert!(actions.iter().all(|a| allowed.contains(&kind(a).as_str())));
        assert_eq!(
            plays(&actions).len(),
            hand(other),
            "{other} starts with exactly its opening hand"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Non-lethal attack crossing the budget, several attackers
// ---------------------------------------------------------------------------------------

#[test]
fn a_non_lethal_attack_can_cross_the_budget_and_each_creature_can_attack() {
    // Every card costs 1 to play, 2 to attack with; One has 10 time units, hero health is
    // large enough for non-lethal hits.
    let mut setup = fixture();
    uniform_cards(&mut setup, 1, 2);
    setup["one"]["hero"]["effects"] = json!([draw_effect(1, 1)]);
    setup["one"]["hero"]["max_time"] = 10.into();
    setup["one"]["hero"]["start_budget"] = 5.into();
    setup["two"]["hero"]["max_time"] = 10.into();
    setup["two"]["hero"]["health"] = 100.into();
    let mut game = start(&setup, 5);

    // Summon three creatures (3 time units), 2 left.
    for _ in 0..3 {
        let card = plays(&legal(&game))[0];
        apply(&mut game, &play(card));
    }
    let actions = legal(&game);
    let attacks: Vec<&Value> = actions.iter().filter(|a| kind(a) == "attack").collect();
    assert_eq!(attacks.len(), 3, "one attack action per creature on board");

    // First attack: 2 - 2 = 0, the budget is spent, the token crosses (non-lethal).
    let events = apply(&mut game, attacks[0]);
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "damaged",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(count(&events, "game_won"), 0);
    assert_eq!(token(&events).0, "Two");
}

// ---------------------------------------------------------------------------------------
// Attacking a creature through the JSON boundary
// ---------------------------------------------------------------------------------------

fn strike(attacker: u64, victim: u64) -> Value {
    json!({"attack": {"attacker": attacker, "target": {"creature": victim}}})
}

#[test]
fn a_creature_attack_kills_logs_its_events_and_hostile_or_misdirected_ones_are_refused() {
    // Every card costs 1 to play and to attack with and has 1 force and 1 health: each
    // action crosses the budget (One holds 1 time unit).
    let mut setup = fixture();
    uniform_cards(&mut setup, 1, 1);
    setup["one"]["hero"]["initiative"] = 9.into();
    setup["one"]["hero"]["start_budget"] = 1.into();
    setup["two"]["hero"]["initiative"] = 1.into();
    let mut game = start(&setup, 3);

    let a = plays(&legal(&game))[0];
    apply(&mut game, &play(a));
    let x = plays(&legal(&game))[0];
    apply(&mut game, &play(x));
    assert!(active_effects(&game) > 0, "One is active again");

    let events = apply(&mut game, &strike(a, x));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "died",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(
        find(&events, "creature_damaged"),
        Some(&json!({"player": "Two", "creature": x, "amount": 1}))
    );
    assert_eq!(
        find(&events, "died"),
        Some(&json!({"player": "Two", "creature": x}))
    );
    assert_eq!(count(&events, "damaged"), 0, "no hero damage");
    assert_eq!(count(&events, "game_won"), 0);
    assert_eq!(token(&events), ("Two".to_owned(), 1));

    // The dead creature cannot attack.
    let before = game.legal_actions_json().unwrap();
    assert_eq!(refusal(&mut game, &strike(x, a)), "attacker_not_on_board");
    assert_eq!(game.legal_actions_json().unwrap(), before);

    // Back to One with a creature of its own: nothing but a creature of Two's board is a
    // target, and every refusal is the same error without touching the game.
    let card = plays(&legal(&game))[0];
    apply(&mut game, &play(card));
    let before = game.legal_actions_json().unwrap();
    for victim in [x, a, card, 9999] {
        assert_eq!(
            refusal(&mut game, &strike(a, victim)),
            "target_not_on_board",
            "{victim}"
        );
    }
    assert_eq!(game.legal_actions_json().unwrap(), before);
    assert_eq!(
        refusal(&mut game, &strike(9999, 9999)),
        "attacker_not_on_board"
    );

    // Hostile target shapes never reach the rules.
    for hostile in [
        r#"{"creature":-1}"#,
        "4294967296",
        r#""7""#,
        "1.5",
        "null",
        r#"{"creature":7,"x":1}"#,
        r#"{"Creature":7}"#,
    ] {
        let action = format!(r#"{{"attack":{{"attacker":{a},"target":{hostile}}}}}"#);
        assert_eq!(
            error_code(&game.apply_json(&action).unwrap_err()),
            "invalid_json",
            "{hostile}"
        );
    }
    assert_eq!(game.legal_actions_json().unwrap(), before);
}
