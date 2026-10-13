//! End-to-end tests of stories S1 ("a duel playable to victory") and S2a ("targets, death and
//! discard of creatures"), driven exclusively through the JSON boundary of `engine-wasm`
//! (`Duel::from_json` / `legal_actions_json` / `apply_json`), the same one the future web
//! client uses, on the `fixture` data set.
//!
//! There is no state accessor on this boundary, so nothing here peeks at the engine:
//! card ids, the active player and the budget are read from what the JSON exposes (legal
//! actions and events). "State identical after a refusal" is checked with twins: two
//! `Duel`s of the same seed fed the same actions, the refused action going to one only.
//!
//! S1 (cases 1 to 11): setup validation, who starts, the time rule, summoning, attacking the
//! enemy hero, a duel to victory, concession, journal order, defeat by exhaustion (not
//! reachable in play between the real fixture decks, since every hero keeps a draw effect and
//! a pile, but reachable through the JSON boundary by mutating the setup: a hero with no
//! effect and no hand, at setup and as the consequence of a handover).
//!
//! S2a (cases 12 to 20, at the end of the file): an attack aims at the hero or at an opposing
//! creature, a creature at 0 health dies and leaves the board (it is then neither a target nor
//! an attacker), excess damage is lost, hostile targets are all refused alike, the time rule
//! applies to creature attacks, a kill can exhaust the receiver of the token, and whole duels
//! fought creatures first end by themselves. The discard pile is not observable in JSON (its
//! order is covered by the engine's unit tests): the order of the successive `died` events and
//! the absence of the creature from every later action stand for it.

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
    uniform_stats(setup, cost, 1, attack_cost, 1);
}

/// Every card of both decks gets the same four stats.
fn uniform_stats(setup: &mut Value, cost: u32, force: u32, attack_cost: u32, health: u32) {
    for side in ["one", "two"] {
        set_side_stats(setup, side, cost, force, attack_cost, health);
    }
}

/// Every card of one deck (`"one"` or `"two"`) gets the same four stats.
fn set_side_stats(
    setup: &mut Value,
    side: &str,
    cost: u32,
    force: u32,
    attack_cost: u32,
    health: u32,
) {
    let stats = json!({"cost": cost, "force": force, "attack_cost": attack_cost, "health": health});
    setup[side]["cards"] = Value::Array(vec![stats; 30]);
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
fn a_four_key_setup_still_starts_and_riposte_at_the_maximum_is_accepted() {
    // The stats of every card carry only the S1/S2a keys: `provocation` and `riposte` default.
    let mut setup = fixture();
    uniform_stats(&mut setup, 1, 1, 1, 1);
    assert!(setup["one"]["cards"][0].get("riposte").is_none());
    let game = start(&setup, 1);
    assert!(!plays(&legal(&game)).is_empty());
    // The bound is inclusive: `MAX_STAT` itself starts, `MAX_STAT + 1` is `invalid_setup`.
    setup["one"]["cards"][0]["riposte"] = 10_000.into();
    start(&setup, 1);
    setup["one"]["cards"][0]["riposte"] = 10_001.into();
    assert_eq!(setup_error(&setup, 1), "invalid_setup");
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
        ("card riposte above the maximum", |v| {
            v["two"]["cards"][4]["riposte"] = 10_001.into()
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
    let hand = plays(&legal(&game));
    apply(&mut game, &play(hand[0]));
    let before = game.legal_actions_json().unwrap();
    // The dead creature, its own creature, a card still in the opponent's hand, an unknown id.
    for victim in [x, a, hand[1], 9999] {
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

// ---------------------------------------------------------------------------------------
// Targets offered by legal_actions
// ---------------------------------------------------------------------------------------

/// The attack actions of a list of legal actions, as `(attacker, target)` pairs.
fn attacks(actions: &[Value]) -> Vec<(u64, Value)> {
    actions
        .iter()
        .filter(|a| kind(a) == "attack")
        .map(|a| {
            (
                a["attack"]["attacker"].as_u64().unwrap(),
                a["attack"]["target"].clone(),
            )
        })
        .collect()
}

#[test]
fn legal_actions_offer_the_hero_then_each_opposing_creature_and_forget_the_dead() {
    // Every card costs 1 to play and to attack with, has 1 force and 1 health; every action
    // crosses the budget of 1 time unit.
    let mut setup = fixture();
    uniform_cards(&mut setup, 1, 1);
    setup["one"]["hero"]["initiative"] = 9.into();
    setup["one"]["hero"]["start_budget"] = 1.into();
    setup["two"]["hero"]["initiative"] = 1.into();
    let mut game = start(&setup, 3);

    let a = plays(&legal(&game))[0];
    apply(&mut game, &play(a));
    // Two has no creature yet, so it has no attack to offer (only a play and an effect).
    assert!(attacks(&legal(&game)).is_empty());
    let x = plays(&legal(&game))[0];
    apply(&mut game, &play(x));
    assert_eq!(
        attacks(&legal(&game)),
        [(a, json!("hero")), (a, json!({"creature": x}))]
    );

    // One summons a second creature (the token crosses), Two does the same.
    let b = plays(&legal(&game))[0];
    apply(&mut game, &play(b));
    let y = plays(&legal(&game))[0];
    apply(&mut game, &play(y));
    let offered = legal(&game);
    assert_eq!(
        attacks(&offered),
        [
            (a, json!("hero")),
            (a, json!({"creature": x})),
            (a, json!({"creature": y})),
            (b, json!("hero")),
            (b, json!({"creature": x})),
            (b, json!({"creature": y})),
        ],
        "per attacker: the hero first, then the opposing board in order"
    );
    assert!(
        offered.contains(&json!({"attack": {"attacker": a, "target": {"creature": x}}})),
        "the exact JSON shape a client sends back"
    );
    assert_eq!(offered.last(), Some(&json!("concede")));

    // A kill removes the victim from the targets, and a dead creature stops attacking.
    apply(&mut game, &strike(a, x));
    assert_eq!(count(&apply(&mut game, &strike(y, b)), "died"), 1);
    let after = legal(&game);
    for (attacker, target) in attacks(&after) {
        assert_ne!(attacker, b, "dead creatures do not attack");
        assert_ne!(
            target,
            json!({"creature": b}),
            "dead creatures are not targets"
        );
        assert_ne!(target, json!({"creature": x}));
    }
    assert!(!attacks(&after).is_empty());
}

// =======================================================================================
// S2a: targets, death and discard of creatures (cases 12 to 20)
// =======================================================================================

fn strike_hero(attacker: u64) -> Value {
    json!({"attack": {"attacker": attacker, "target": "hero"}})
}

/// The creature ids `attacker` is offered as targets, in order.
fn creature_targets(actions: &[Value], attacker: u64) -> Vec<u64> {
    attacks(actions)
        .into_iter()
        .filter(|(a, _)| *a == attacker)
        .filter_map(|(_, target)| target.get("creature").and_then(Value::as_u64))
        .collect()
}

/// Both heroes always hand the token over: every play, effect or attack costs 1 and every
/// budget is 1, so the players alternate action after action. One (the higher initiative)
/// starts. Cards: cost 1, force 1, attack cost 1, health 1. Both heroes have a "draw 1 for 1"
/// effect (the idle move of a player with nothing to do), 8 cards in hand and 100 health.
fn crossing_setup() -> Value {
    let mut setup = fixture();
    uniform_stats(&mut setup, 1, 1, 1, 1);
    for (side, initiative) in [("one", 9), ("two", 1)] {
        let hero = &mut setup[side]["hero"];
        hero["effects"] = json!([draw_effect(1, 1)]);
        hero["initiative"] = initiative.into();
        hero["max_time"] = 10.into();
        hero["start_budget"] = 1.into();
        hero["starting_hand"] = 8.into();
        hero["health"] = 100.into();
    }
    setup
}

const SIDES: [&str; 2] = ["One", "Two"];

/// Index (0 = One, 1 = Two) of the player named by a JSON event field.
fn side_of(player: &Value) -> usize {
    player
        .as_str()
        .and_then(|name| SIDES.iter().position(|s| *s == name))
        .expect("a player")
}

/// A duel plus what a client can know about it from the JSON alone: who holds the token, the
/// boards (from `played` and `died`), the hands (from the legal `play` actions of each player
/// at its last move, and `drew`) and the dead creatures. Nothing here reads the engine.
/// One must start.
struct Table {
    game: Duel,
    setup: Value,
    seed: u32,
    log: Vec<Value>,
    active: usize,
    boards: [Vec<u64>; 2],
    hands: [Vec<u64>; 2],
    discards: [Vec<u64>; 2],
}

impl Table {
    fn new(setup: &Value, seed: u32) -> Self {
        let initiative = |side: &str| setup[side]["hero"]["initiative"].as_u64().unwrap();
        assert!(initiative("one") > initiative("two"), "One must start");
        Table {
            game: start(setup, seed),
            setup: setup.clone(),
            seed,
            log: Vec::new(),
            active: 0,
            boards: Default::default(),
            hands: Default::default(),
            discards: Default::default(),
        }
    }

    /// Applies `action` for the active player and follows its events.
    fn act(&mut self, action: &Value) -> Vec<Value> {
        let me = self.active;
        self.hands[me] = plays(&legal(&self.game));
        let events = apply(&mut self.game, action);
        self.log.push(action.clone());
        for event in &events {
            match kind(event).as_str() {
                "played" => {
                    let card = event["played"]["card"].as_u64().unwrap();
                    self.hands[me].retain(|c| *c != card);
                    self.boards[me].push(card);
                }
                "drew" => {
                    let side = side_of(&event["drew"]["player"]);
                    self.hands[side].push(event["drew"]["card"].as_u64().unwrap());
                }
                "died" => {
                    let side = side_of(&event["died"]["player"]);
                    let card = event["died"]["creature"].as_u64().unwrap();
                    self.boards[side].retain(|c| *c != card);
                    self.discards[side].push(card);
                }
                "token_moved" => self.active = side_of(&event["token_moved"]["player"]),
                _ => {}
            }
        }
        events
    }

    /// The active player summons the first card of its hand.
    fn summon(&mut self) -> u64 {
        let card = plays(&legal(&self.game))[0];
        self.act(&play(card));
        card
    }

    /// The active player activates its first effect, to give the token away.
    fn idle(&mut self) -> Vec<Value> {
        self.act(&activate(0))
    }

    /// A twin: a new duel of the same setup and seed fed the same actions so far.
    fn fork(&self) -> Table {
        let mut twin = Table::new(&self.setup, self.seed);
        for action in &self.log {
            twin.act(action);
        }
        twin
    }
}

/// A `crossing`-style table where, in the end, One is active with `one` creatures on its board
/// and Two has `two`.
fn deploy(setup: &Value, seed: u32, one: usize, two: usize) -> Table {
    let mut table = Table::new(setup, seed);
    while table.boards[0].len() < one || table.boards[1].len() < two {
        let me = table.active;
        if table.boards[me].len() < [one, two][me] {
            table.summon();
        } else {
            table.idle();
        }
    }
    while table.active != 0 {
        table.idle();
    }
    assert_eq!((table.boards[0].len(), table.boards[1].len()), (one, two));
    table
}

// ---------------------------------------------------------------------------------------
// Case 12: the targets offered by legal_actions
// ---------------------------------------------------------------------------------------

#[test]
fn each_attacker_is_offered_the_hero_then_every_opposing_creature_and_never_its_own_side() {
    // One holds three creatures, Two one.
    let mut t = deploy(&crossing_setup(), 1, 3, 1);
    let (ones, twos) = (t.boards[0].clone(), t.boards[1].clone());
    let actions = legal(&t.game);
    let expected: Vec<(u64, Value)> = ones
        .iter()
        .flat_map(|a| {
            std::iter::once((*a, json!("hero")))
                .chain(twos.iter().map(|x| (*a, json!({"creature": *x}))))
        })
        .collect();
    assert_eq!(expected.len(), 3 * (1 + 1));
    assert_eq!(attacks(&actions), expected);

    // The kinds come in the documented order, with Concede last and alone.
    let rank = |a: &Value| match kind(a).as_str() {
        "play" => 0,
        "activate" => 1,
        "attack" => 2,
        "concede" => 3,
        other => panic!("unexpected action {other}"),
    };
    let ranks: Vec<i32> = actions.iter().map(rank).collect();
    assert!(ranks.windows(2).all(|w| w[0] <= w[1]), "{ranks:?}");
    assert_eq!(ranks.iter().filter(|r| **r == 3).count(), 1);
    assert_eq!(actions.last(), Some(&json!("concede")));

    // Every attack offered is accepted, on a twin each (offered means legal).
    for action in actions.iter().filter(|a| kind(a) == "attack") {
        let mut twin = t.fork();
        let events = twin.act(action);
        assert_eq!(kind(&events[0]), "attacked", "{action}");
    }

    // Two's turn: its single creature sees the hero then One's three, in One's board order.
    t.idle();
    assert_eq!(t.active, 1);
    let mut want = vec![(twos[0], json!("hero"))];
    want.extend(ones.iter().map(|a| (twos[0], json!({"creature": *a}))));
    assert_eq!(attacks(&legal(&t.game)), want);

    // An opposing board that is empty leaves the hero as the only target.
    let alone = deploy(&crossing_setup(), 1, 1, 0);
    assert_eq!(
        attacks(&legal(&alone.game)),
        [(alone.boards[0][0], json!("hero"))]
    );
    // And no creature, no attack at all (a play and an effect, then Concede).
    let fresh = Table::new(&crossing_setup(), 1);
    assert!(attacks(&legal(&fresh.game)).is_empty());
}

// ---------------------------------------------------------------------------------------
// Case 13: a kill, its journal, and what the dead creature can no longer do
// ---------------------------------------------------------------------------------------

#[test]
fn a_killed_creature_is_journaled_then_is_neither_a_target_nor_an_attacker() {
    let mut t = deploy(&crossing_setup(), 3, 2, 2);
    let (a, b) = (t.boards[0][0], t.boards[0][1]);
    let (x, y) = (t.boards[1][0], t.boards[1][1]);
    let mut twin = t.fork();

    // The kill: the attack, the damage, the death, then the time and the handover.
    let events = t.act(&strike(a, x));
    twin.act(&strike(a, x));
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
        events[0],
        json!({"attacked": {"player": "One", "attacker": a, "cost": 1}})
    );
    assert_eq!(
        events[1],
        json!({"creature_damaged": {"player": "Two", "creature": x, "amount": 1}})
    );
    assert_eq!(events[2], json!({"died": {"player": "Two", "creature": x}}));
    assert_eq!(
        events[3],
        json!({"token_moved": {"player": "Two", "budget": 1}})
    );
    assert_eq!(
        events[4],
        json!({"active_player_changed": {"player": "Two"}})
    );
    assert_eq!(t.boards[1], [y]);

    // Two's turn: the dead creature is no longer an attacker (offered or accepted)...
    assert_eq!(
        attacks(&legal(&t.game)),
        [
            (y, json!("hero")),
            (y, json!({"creature": a})),
            (y, json!({"creature": b}))
        ]
    );
    assert_eq!(refusal(&mut t.game, &strike(x, a)), "attacker_not_on_board");
    assert_eq!(
        refusal(&mut t.game, &strike_hero(x)),
        "attacker_not_on_board"
    );

    // ...and not a target either, once the token is back with One.
    t.idle();
    twin.idle();
    assert_eq!(
        attacks(&legal(&t.game)),
        [
            (a, json!("hero")),
            (a, json!({"creature": y})),
            (b, json!("hero")),
            (b, json!({"creature": y}))
        ]
    );
    assert_eq!(refusal(&mut t.game, &strike(a, x)), "target_not_on_board");
    assert_eq!(refusal(&mut t.game, &strike(b, x)), "target_not_on_board");
    assert_eq!(refusal(&mut t.game, &strike(x, y)), "attacker_not_on_board");

    // A second kill: deaths come in the order of the attacks, each announced exactly once.
    let events = t.act(&strike(b, y));
    twin.act(&strike(b, y));
    assert_eq!(
        find(&events, "died"),
        Some(&json!({"player": "Two", "creature": y}))
    );
    assert_eq!(t.discards[1], [x, y], "x died first, then y");
    assert!(t.boards[1].is_empty());
    assert!(
        attacks(&legal(&t.game)).is_empty(),
        "Two has no creature left to attack with"
    );
    t.idle();
    twin.idle();
    assert_eq!(
        attacks(&legal(&t.game)),
        [(a, json!("hero")), (b, json!("hero"))]
    );
    for dead in [x, y] {
        assert_eq!(
            refusal(&mut t.game, &strike(a, dead)),
            "target_not_on_board"
        );
    }

    // None of these refusals changed anything: the twin that never received them agrees.
    assert_twins_agree(&mut t.game, &mut twin.game, 8);
}

// ---------------------------------------------------------------------------------------
// Case 14: a non-lethal hit
// ---------------------------------------------------------------------------------------

#[test]
fn a_non_lethal_hit_wounds_without_killing_and_the_target_dies_on_a_later_hit() {
    let mut setup = crossing_setup();
    set_side_stats(&mut setup, "two", 1, 1, 1, 3); // Two's creatures hold three hits
    let mut t = deploy(&setup, 4, 1, 1);
    let (a, x) = (t.boards[0][0], t.boards[1][0]);

    for (hit, lethal) in [(1, false), (2, false), (3, true)] {
        let events = t.act(&strike(a, x));
        let mut expected = vec!["attacked", "creature_damaged"];
        if lethal {
            expected.push("died");
        }
        expected.extend(["token_moved", "active_player_changed"]);
        assert_eq!(kinds(&events), expected, "hit {hit}");
        assert_eq!(
            find(&events, "creature_damaged"),
            Some(&json!({"player": "Two", "creature": x, "amount": 1})),
            "hit {hit}"
        );
        assert_eq!(count(&events, "damaged"), 0, "the hero is not touched");
        assert_eq!(count(&events, "game_won"), 0);
        if lethal {
            assert_eq!(
                find(&events, "died"),
                Some(&json!({"player": "Two", "creature": x}))
            );
            break;
        }
        // The wounded creature stays in the game: it can attack, and it is still a target.
        assert_eq!(t.boards[1], [x], "hit {hit}");
        assert!(attacks(&legal(&t.game)).contains(&(x, json!("hero"))));
        t.idle();
        assert!(attacks(&legal(&t.game)).contains(&(a, json!({"creature": x}))));
    }

    // Dead, it is offered no more.
    assert!(t.boards[1].is_empty());
    t.idle();
    assert_eq!(attacks(&legal(&t.game)), [(a, json!("hero"))]);
}

// ---------------------------------------------------------------------------------------
// Case 15: the excess damage is lost
// ---------------------------------------------------------------------------------------

#[test]
fn excess_damage_on_a_creature_is_lost_and_never_reaches_the_hero() {
    // One's creatures hit for 5, Two's have 1 health, and Two's hero has 1 health too.
    let mut setup = crossing_setup();
    set_side_stats(&mut setup, "one", 1, 5, 1, 1);
    setup["two"]["hero"]["health"] = 1.into();
    let mut t = deploy(&setup, 6, 1, 1);
    let (a, x) = (t.boards[0][0], t.boards[1][0]);
    let mut control = t.fork();

    let events = t.act(&strike(a, x));
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
    // The log carries the nominal force; the hero takes nothing of the excess.
    assert_eq!(
        find(&events, "creature_damaged"),
        Some(&json!({"player": "Two", "creature": x, "amount": 5}))
    );
    assert_eq!(count(&events, "damaged"), 0);
    assert_eq!(count(&events, "game_won"), 0);
    assert!(!legal(&t.game).is_empty(), "the game goes on");

    // The hero is intact at 1 health: the next hit on it ends the game, and only that hit.
    t.idle();
    let events = t.act(&strike_hero(a));
    assert_eq!(kinds(&events), ["attacked", "damaged", "game_won"]);
    assert_eq!(
        events[2],
        json!({"game_won": {"winner": "One", "reason": "hero_down"}})
    );

    // Control: this very position is lethal for the hero when it is the target.
    let events = control.act(&strike_hero(a));
    assert_eq!(kinds(&events), ["attacked", "damaged", "game_won"]);
}

// ---------------------------------------------------------------------------------------
// Case 16: hostile targets
// ---------------------------------------------------------------------------------------

#[test]
fn every_hostile_target_gets_one_identical_refusal_and_malformed_ones_never_reach_the_rules() {
    let mut t = deploy(&crossing_setup(), 5, 2, 2);
    let (a, b) = (t.boards[0][0], t.boards[0][1]);
    let (x, y) = (t.boards[1][0], t.boards[1][1]);
    t.act(&strike(a, y)); // y dies, Two is active
    t.idle(); // Two draws a card, One is active again
    assert_eq!(t.active, 0);
    let two_hand = t.hands[1][0];
    let drawn_by_two = *t.hands[1].last().unwrap();
    let own_hand = plays(&legal(&t.game))[0];
    assert_eq!(creature_targets(&legal(&t.game), a), [x]);
    let mut twin = t.fork();

    // Every kind of id a client could aim at: all of them are refused with the same code.
    let hostile = [
        ("its own other creature", b),
        ("itself", a),
        ("a card of its own hand", own_hand),
        ("a card of the opponent's hand", two_hand),
        ("the card the opponent just drew", drawn_by_two),
        ("a dead creature", y),
        ("an unknown id", 9999),
        ("u32::MAX", u64::from(u32::MAX)),
    ];
    let mut errors = Vec::new();
    for (what, id) in hostile {
        let err = t.game.apply_json(&strike(a, id).to_string()).unwrap_err();
        assert_eq!(error_code(&err), "target_not_on_board", "{what}");
        errors.push(err);
    }
    // Sweep: every id that is not the one live opposing creature, whichever zone it is in
    // (hands, piles, boards, discards of both players: the 60 cards have ids below 200).
    let mut swept = 0;
    for id in (0..200u64).filter(|id| *id != x) {
        let err = t.game.apply_json(&strike(a, id).to_string()).unwrap_err();
        assert_eq!(error_code(&err), "target_not_on_board", "id {id}");
        errors.push(err);
        swept += 1;
    }
    assert_eq!(swept, 199);
    // Byte for byte the same answer whatever the zone: no oracle on hidden cards.
    assert!(
        errors.windows(2).all(|w| w[0] == w[1]),
        "refusals differ by zone"
    );

    // Malformed shapes are refused at the boundary, before the rules.
    for target in [
        r#"{"creature":-1}"#,
        r#"{"creature":"7"}"#,
        r#"{"creature":4294967296}"#,
        r#"{"creature":7,"extra":1}"#,
        r#"{"creature":null}"#,
        r#"{"creature":1.5}"#,
        r#"{"Creature":7}"#,
        r#""7""#,
        "4294967296",
        "-1",
        "{}",
        "[]",
        "null",
    ] {
        let action = format!(r#"{{"attack":{{"attacker":{a},"target":{target}}}}}"#);
        assert_eq!(
            error_code(&t.game.apply_json(&action).unwrap_err()),
            "invalid_json",
            "{target}"
        );
    }
    for action in [
        format!(r#"{{"attack":{{"attacker":{a}}}}}"#),
        format!(r#"{{"attack":{{"target":{{"creature":{x}}}}}}}"#),
        r#"{"attack":{"attacker":-1,"target":"hero"}}"#.to_owned(),
        r#"{"attack":{"attacker":4294967296,"target":"hero"}}"#.to_owned(),
        format!(r#"{{"attack":{{"attacker":{a},"target":"hero","extra":1}}}}"#),
    ] {
        assert_eq!(
            error_code(&t.game.apply_json(&action).unwrap_err()),
            "invalid_json",
            "{action}"
        );
    }

    // After all of this, nothing moved: the twin that saw none of it agrees.
    assert_twins_agree(&mut t.game, &mut twin.game, 8);
}

// ---------------------------------------------------------------------------------------
// Case 17: the time rule on a creature attack
// ---------------------------------------------------------------------------------------

#[test]
fn a_creature_attack_follows_the_time_rule_whether_it_kills_or_not() {
    // Every card costs 3. One starts with 3 time units; Two caps at 5. One's summoning
    // crosses (3 = 3) and gives Two 1 unit; Two's costs 3 > 1 and gives One 3 again.
    // The attack of One's creature then costs `attack_cost`.
    for health in [1u32, 9] {
        for (attack_cost, (receiver, budget)) in [
            (1u32, ("One", 2u64)), // c < b: One keeps the token with 3 - 1
            (3, ("Two", 1)),       // c = b: the token crosses, 1 unit
            (5, ("Two", 3)),       // c > b: 5 - 3 + 1
            (9, ("Two", 5)),       // c > b: 9 - 3 + 1 = 7, capped by Two's max_time
        ] {
            let what = format!("health {health}, attack cost {attack_cost}");
            let mut setup = timing_setup(3, vec![draw_effect(1, 1)]);
            uniform_stats(&mut setup, 3, 1, attack_cost, 1);
            set_side_stats(&mut setup, "two", 3, 1, attack_cost, health);
            setup["two"]["hero"]["health"] = 100.into();
            let mut t = Table::new(&setup, 8);
            t.summon();
            let x = plays(&legal(&t.game))[0];
            let events = t.act(&play(x));
            assert_eq!(token(&events), ("One".into(), 3), "{what}");
            let a = t.boards[0][0];
            let mut hero_twin = t.fork();

            let events = t.act(&strike(a, x));
            let mut expected = vec!["attacked", "creature_damaged"];
            if health == 1 {
                expected.push("died");
            }
            expected.push("token_moved");
            if receiver == "Two" {
                expected.push("active_player_changed");
            }
            assert_eq!(kinds(&events), expected, "{what}");
            assert_eq!(find(&events, "attacked").unwrap()["cost"], attack_cost);
            assert_eq!(token(&events), (receiver.to_owned(), budget), "{what}");
            assert_eq!(SIDES[t.active], receiver, "{what}");

            // The very same time rule as an attack on the hero (S1).
            let hero_events = hero_twin.act(&strike_hero(a));
            assert_eq!(token(&hero_events), token(&events), "{what}");
            assert_eq!(
                count(&hero_events, "active_player_changed"),
                count(&events, "active_player_changed"),
                "{what}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Case 18: defeat by exhaustion when a kill hands the token over
// ---------------------------------------------------------------------------------------

/// One summons, Two summons `x` (leaving it `two_hand - 1` cards), then One kills `x`. Two has
/// the draw effect only if `two_effect`. Returns the events of the kill.
fn kill_the_only_creature_of_two(two_hand: u32, two_effect: bool) -> (Table, Vec<Value>) {
    let mut setup = crossing_setup();
    setup["two"]["hero"]["starting_hand"] = two_hand.into();
    if !two_effect {
        setup["two"]["hero"]["effects"] = json!([]);
    }
    let mut t = Table::new(&setup, 3);
    let a = t.summon();
    let x = t.summon();
    assert_eq!(t.active, 0);
    let events = t.act(&strike(a, x));
    (t, events)
}

#[test]
fn killing_the_last_creature_of_a_stripped_receiver_ends_the_game_by_exhaustion() {
    // Two: a single card in hand, no effect. Once x is dead and the token crosses to Two, it has
    // nothing left to do: the game ends on the spot, the very last event of the kill.
    let (mut t, events) = kill_the_only_creature_of_two(1, false);
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "died",
            "token_moved",
            "active_player_changed",
            "game_won"
        ]
    );
    assert_eq!(
        events.last(),
        Some(&json!({"game_won": {"winner": "One", "reason": "exhaustion"}}))
    );
    every_action_is_refused_as_game_over(&mut t.game);

    // Control 1: the same position without the kill. x is alive, so it keeps Two in the game
    // (a living creature can always hit the hero).
    let mut setup = crossing_setup();
    setup["two"]["hero"]["starting_hand"] = 1.into();
    setup["two"]["hero"]["effects"] = json!([]);
    let mut alive = Table::new(&setup, 3);
    alive.summon();
    alive.summon();
    let events = alive.idle();
    assert_eq!(count(&events, "game_won"), 0);
    assert_eq!(alive.active, 1);
    assert!(
        attacks(&legal(&alive.game))
            .iter()
            .any(|(att, _)| *att == alive.boards[1][0])
    );

    // Control 2: with a draw effect, the same kill leaves Two something to do.
    let (t, events) = kill_the_only_creature_of_two(1, true);
    assert_eq!(count(&events, "died"), 1);
    assert_eq!(count(&events, "game_won"), 0);
    assert!(has(&legal(&t.game), "activate"));

    // Control 3: with a second card in hand and no effect, it can still summon.
    let (t, events) = kill_the_only_creature_of_two(2, false);
    assert_eq!(count(&events, "died"), 1);
    assert_eq!(count(&events, "game_won"), 0);
    assert_eq!(plays(&legal(&t.game)).len(), 1);
}

// ---------------------------------------------------------------------------------------
// Case 19: whole duels fought creatures first
// ---------------------------------------------------------------------------------------

/// Attack a creature if one can be attacked, else the S1 policy (hero, summon, effect).
/// Never concedes.
fn creature_first(actions: &[Value]) -> Value {
    actions
        .iter()
        .find(|a| kind(a) == "attack" && a["attack"]["target"].get("creature").is_some())
        .cloned()
        .unwrap_or_else(|| pick(actions))
}

/// A deterministic pseudo-random choice among the non-concede actions (a generator of the
/// test, unrelated to the engine's seed).
fn pseudo_random(seed: u32) -> impl FnMut(&[Value]) -> Value {
    let mut state = u64::from(seed).wrapping_mul(2_862_933_555_777_941_757);
    move |actions| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let playable: Vec<&Value> = actions.iter().filter(|a| kind(a) != "concede").collect();
        playable[(state >> 33) as usize % playable.len()].clone()
    }
}

/// Plays a whole duel from `setup` with `choose`, until no action is legal.
fn run_policy(setup: &Value, seed: u32, mut choose: impl FnMut(&[Value]) -> Value) -> Vec<Step> {
    let mut game = start(setup, seed);
    let mut journal = Vec::new();
    loop {
        let offered = game.legal_actions_json().unwrap();
        let actions: Vec<Value> = serde_json::from_str(&offered).unwrap();
        if actions.is_empty() {
            assert_eq!(offered, "[]");
            return journal;
        }
        let action = choose(&actions);
        let events = apply(&mut game, &action);
        journal.push((offered, action, events));
        assert!(journal.len() < 1000, "seed {seed}: the duel does not end");
    }
}

/// What every finished duel of S2a must look like, whoever played it.
/// Returns how many creatures died.
fn assert_a_creature_duel_is_sound(what: &str, journal: &[Step]) -> usize {
    let (mut summoned, mut dead): (Vec<u64>, Vec<u64>) = (Vec::new(), Vec::new());
    for (i, (offered, action, events)) in journal.iter().enumerate() {
        let actions: Vec<Value> = serde_json::from_str(offered).unwrap();
        assert!(actions.contains(action), "{what} step {i}: not offered");
        assert_ne!(action, &json!("concede"), "{what} step {i}: no concession");
        // A dead creature is never offered again, as an attacker, a target or a card to play.
        for offered_action in &actions {
            let ids: Vec<u64> = match kind(offered_action).as_str() {
                "play" => vec![offered_action["play"]["card"].as_u64().unwrap()],
                "attack" => {
                    let attack = &offered_action["attack"];
                    let mut ids = vec![attack["attacker"].as_u64().unwrap()];
                    ids.extend(attack["target"].get("creature").and_then(Value::as_u64));
                    ids
                }
                _ => vec![],
            };
            assert!(
                ids.iter().all(|id| !dead.contains(id)),
                "{what} step {i}: a dead creature is offered in {offered_action}"
            );
        }
        // A death follows the damage that caused it, and a game never ends before its last step.
        let names = kinds(events);
        for (at, event) in events.iter().enumerate() {
            match kind(event).as_str() {
                "played" => summoned.push(event["played"]["card"].as_u64().unwrap()),
                "died" => {
                    let creature = event["died"]["creature"].as_u64().unwrap();
                    assert!(summoned.contains(&creature), "{what}: died before summoned");
                    assert!(!dead.contains(&creature), "{what}: died twice");
                    dead.push(creature);
                    let wound = events[..at].iter().position(|e| {
                        kind(e) == "creature_damaged"
                            && e["creature_damaged"]["creature"] == event["died"]["creature"]
                    });
                    assert!(wound.is_some(), "{what} step {i}: died without a wound");
                    assert!(!names[..at].contains(&"token_moved".to_owned()));
                }
                "creature_damaged" => {
                    assert!(event["creature_damaged"]["amount"].as_u64().unwrap() >= 1);
                }
                _ => {}
            }
        }
        if i + 1 < journal.len() {
            assert_eq!(count(events, "game_won"), 0, "{what} step {i}: ended early");
        }
    }
    let (_, _, last) = journal.last().expect("a duel has moves");
    let won = last.last().unwrap();
    assert_eq!(kind(won), "game_won", "{what}");
    let reason = won["game_won"]["reason"].as_str().unwrap();
    assert!(
        ["hero_down", "exhaustion"].contains(&reason),
        "{what}: {reason}"
    );
    dead.len()
}

/// The fixture with the initiatives swapped, so that Two starts.
fn swapped_fixture() -> Value {
    let mut setup = fixture();
    setup["one"]["hero"]["initiative"] = 1.into();
    setup["two"]["hero"]["initiative"] = 2.into();
    setup
}

#[test]
fn creature_first_duels_are_deterministic_end_by_themselves_and_never_reoffer_the_dead() {
    for (name, setup) in [("fixture", fixture()), ("swapped", swapped_fixture())] {
        for seed in 0..25 {
            let what = format!("{name} seed {seed}");
            let journal = run_policy(&setup, seed, creature_first);
            assert_eq!(
                journal,
                run_policy(&setup, seed, creature_first),
                "{what}: not deterministic"
            );
            // The policy goes for creatures first: every duel sees at least one death.
            let deaths = assert_a_creature_duel_is_sound(&what, &journal);
            assert!(deaths > 0, "{what}: no creature ever died");
        }
    }
}

#[test]
fn pseudo_random_duels_always_end_and_never_reoffer_the_dead() {
    let mut deaths = 0;
    for (name, setup) in [("fixture", fixture()), ("swapped", swapped_fixture())] {
        for seed in 0..25 {
            let what = format!("{name} seed {seed}");
            let journal = run_policy(&setup, seed, pseudo_random(seed));
            assert_eq!(
                journal,
                run_policy(&setup, seed, pseudo_random(seed)),
                "{what}: not deterministic"
            );
            deaths += assert_a_creature_duel_is_sound(&what, &journal);
        }
    }
    // A random duel may end on the hero before any creature dies, but not all of them.
    assert!(deaths > 0);
}

// ---------------------------------------------------------------------------------------
// Case 20: S1 still holds while the attacker aims at the hero
// ---------------------------------------------------------------------------------------

#[test]
fn a_hero_only_duel_never_touches_a_creature() {
    // The S1 policy (`run_duel`: attack, else summon, else activate) always takes the hero,
    // offered first, as its target: no creature is ever wounded, killed or discarded, and
    // the duel ends the S1 way. (Cases 1 to 11 above run unchanged.)
    for seed in 0..20 {
        let journal = run_duel(seed);
        for (_, action, events) in &journal {
            if kind(action) == "attack" {
                assert_eq!(action["attack"]["target"], "hero", "seed {seed}");
            }
            assert_eq!(count(events, "creature_damaged"), 0, "seed {seed}");
            assert_eq!(count(events, "died"), 0, "seed {seed}");
        }
        let last = &journal.last().unwrap().2;
        assert_eq!(
            kinds(last),
            ["attacked", "damaged", "game_won"],
            "seed {seed}"
        );
    }
}
