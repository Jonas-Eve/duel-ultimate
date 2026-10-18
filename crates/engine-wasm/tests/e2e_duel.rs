//! End-to-end tests of stories S1 ("a duel playable to victory"), S2a ("targets, death and
//! discard of creatures"), S2b ("provocation and riposte") and S3 ("two starter decks, two
//! heroes"), driven exclusively through the JSON boundary of `engine-wasm`
//! (`Duel::from_json` / `start_events_json` / `legal_actions_json` / `apply_json`), the same one
//! the future web client uses, on the `fixture` data set (S1 to S2b) and the `starter` set (S3).
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
//! S2a (cases 12 to 20): an attack aims at the hero or at an opposing creature, a creature at 0 health dies and leaves the board (it is then neither a target nor
//! an attacker), excess damage is lost, hostile targets are all refused alike, the time rule
//! applies to creature attacks, a kill can exhaust the receiver of the token, and whole duels
//! fought creatures first end by themselves. The discard pile is not observable in JSON (its
//! order is covered by the engine's unit tests): the order of the successive `died` events and
//! the absence of the creature from every later action stand for it.
//!
//! S2b (cases 21 to 31, at the end of the file): `provocation` and `riposte` as optional data
//! with hostile forms refused at the boundary, the targets offered and refused under provocation
//! (the hero shielded, hidden zones indistinguishable, recomputed after every action), the
//! simultaneous riposte and its journal, simultaneous deaths in id order, the time rule, defeat
//! by exhaustion when a riposte kills the last creature, and whole duels with both mechanics.
//! Creatures with chosen stats at known ids come from mutating the setup as JSON (case 21 checks
//! that a card's id is its position in its deck); cases 1 to 20 run unchanged as the regression.
//!
//! S3 (cases 32 to 40, at the very end): the `starter` set (Rush against Bulwark) from
//! `data/starter/1.0.0/`, the versions recorded in `game_started` and read back by
//! `start_events_json`, the two heroes' different openings, the costs, attack costs and forces
//! observed in play against the data files, Bulwark's provocation, Rush's riposte, a scripted
//! game to the victory and whole random duels. The health of the cards and the full
//! distributions are not observable here; they are locked by the data tests of the set. Cases 1 to 31 run unchanged as the regression (case 40).

use duel_cards::{CardsError, parse_cards, parse_decks, resolve_deck};
use duel_engine::{GameSetup, PlayerSetup, RuleSet};
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
        cards: resolve_deck(&decks[i], &cards)
            .unwrap()
            .into_iter()
            .map(Into::into)
            .collect(),
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

/// The same setup with the options off (`RuleSet::legacy()` written out in `rules`): for a test
/// that states the behaviour before the options of #127 (a hit on the hero costs the attacker
/// nothing, a board has no limit). The default of the engine is the standard rules.
fn legacy(mut setup: Value) -> Value {
    setup["rules"] = json!({"hero_riposte": false, "board_limit": null});
    setup
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
    run_duel_with(&fixture(), seed)
}

/// `run_duel` on a given setup.
fn run_duel_with(setup: &Value, seed: u32) -> Vec<Step> {
    let mut game = start(setup, seed);
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
    // The options off: the attacker survives the hit on the hero (the journal under the
    // standard rules is pinned by `the_journal_of_a_hit_on_the_hero_ends_with_the_hero_riposte`).
    let mut setup = legacy(fixture());
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

#[test]
fn the_journal_of_a_hit_on_the_hero_ends_with_the_hero_riposte() {
    // The default rules: a non-lethal hit on the hero destroys the attacker, after `damaged`
    // and before the time moves; the JSON shape of `hero_riposted` is the one of `riposted`
    // without an amount.
    let mut setup = fixture();
    uniform_cards(&mut setup, 1, 1);
    setup["one"]["hero"]["effects"] = json!([draw_effect(1, 2)]);
    setup["one"]["hero"]["max_time"] = 10.into();
    setup["one"]["hero"]["start_budget"] = 10.into();
    setup["two"]["hero"]["health"] = 100.into();
    let mut game = start(&setup, 5);
    let card = plays(&legal(&game))[0];
    apply(&mut game, &play(card));
    let events = apply(
        &mut game,
        &json!({"attack": {"attacker": card, "target": "hero"}}),
    );
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "damaged",
            "hero_riposted",
            "died",
            "token_moved"
        ]
    );
    assert_eq!(
        events[2],
        json!({"hero_riposted": {"player": "One", "creature": card}})
    );
    assert_eq!(
        events[3],
        json!({"died": {"player": "One", "creature": card}})
    );
    // The creature is gone: it is not offered again.
    assert!(!legal(&game).iter().any(|a| kind(a) == "attack"));
}

// ---------------------------------------------------------------------------------------
// Non-lethal attack crossing the budget, several attackers
// ---------------------------------------------------------------------------------------

#[test]
fn a_non_lethal_attack_can_cross_the_budget_and_each_creature_can_attack() {
    // Every card costs 1 to play, 2 to attack with; One has 10 time units, hero health is
    // large enough for non-lethal hits. The options off: the attacker survives the hit.
    let mut setup = legacy(fixture());
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
                    // A wound, or the riposte of the hero that destroys the attacker.
                    let wound = events[..at].iter().position(|e| {
                        ["creature_damaged", "hero_riposted"].iter().any(|k| {
                            kind(e) == *k && e[*k]["creature"] == event["died"]["creature"]
                        })
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
    // With the options off: under the standard rules the hero destroys its attacker (a `died`
    // without a wound), which `the_journal_of_a_hit_on_the_hero_ends_with_the_hero_riposte` pins.
    let setup = legacy(fixture());
    for seed in 0..20 {
        let journal = run_duel_with(&setup, seed);
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

// =======================================================================================
// S2b: provocation and riposte (cases 21 to 31)
// =======================================================================================
//
// These cases need creatures with chosen stats at known places, still without any state
// accessor. The setup is mutated as JSON: a card's id is its position in its deck of the setup
// (One 0..29, Two 30..59, before the shuffle), so a deck of chosen cards plus a hand as big as
// the deck (`starting_hand` 30) puts every card in hand with a known id. Case 21 checks that
// link before the other cases lean on it. In an `arena` every action crosses the budget of 1,
// so the players alternate action after action; the only way to hand the token over without
// touching the position is to summon a harmless filler (`pass`).

const SIDE_KEYS: [&str; 2] = ["one", "two"];

/// The complete JSON stats of a creature, `provocation` and `riposte` included.
fn stats_json(
    cost: u32,
    force: u32,
    attack_cost: u32,
    health: u32,
    provocation: bool,
    riposte: u32,
) -> Value {
    json!({
        "cost": cost, "force": force, "attack_cost": attack_cost, "health": health,
        "provocation": provocation, "riposte": riposte
    })
}

/// A whole deck of one side (`"one"` or `"two"`), card by card: ids follow the positions.
fn set_side_cards(setup: &mut Value, side: &str, cards: Vec<Value>) {
    assert_eq!(cards.len(), 30, "a deck holds exactly 30 cards");
    setup[side]["cards"] = Value::Array(cards);
}

/// The card that fills a deck around the chosen ones: summoned only to give the token away.
/// Its 77 health is unlike any chosen card's, so the two are never confused.
fn filler() -> Value {
    stats_json(1, 1, 1, 77, false, 0)
}

/// A creature of cost 1, force 1, attack cost 1, without provocation nor riposte.
fn plain(health: u32) -> Value {
    stats_json(1, 1, 1, health, false, 0)
}

/// A creature of cost 1, force 1, attack cost 1, with provocation.
fn taunt(health: u32) -> Value {
    stats_json(1, 1, 1, health, true, 0)
}

/// A creature of cost 1, force 1, attack cost 1, with a riposte.
fn riposter(health: u32, riposte: u32) -> Value {
    stats_json(1, 1, 1, health, false, riposte)
}

/// The chosen cards first (ids 0.. for One, 30.. for Two), then fillers up to 30.
fn roster(mut chosen: Vec<Value>) -> Vec<Value> {
    assert!(chosen.len() <= 30 && !chosen.contains(&filler()));
    chosen.resize(30, filler());
    chosen
}

/// Two decks of chosen cards in the fixture's setup, both heroes with the whole deck in hand,
/// no effect, 100 health, `max_time` 10 and a budget of 1: One (initiative 9) starts, and since
/// every card costs 1 to play and 1 to attack with, every action hands the token over.
fn arena(one: Vec<Value>, two: Vec<Value>) -> Value {
    let mut setup = fixture();
    set_side_cards(&mut setup, "one", roster(one));
    set_side_cards(&mut setup, "two", roster(two));
    for (side, initiative) in [("one", 9), ("two", 1)] {
        let hero = &mut setup[side]["hero"];
        hero["effects"] = json!([]);
        hero["initiative"] = initiative.into();
        hero["max_time"] = 10.into();
        hero["start_budget"] = 1.into();
        hero["starting_hand"] = 30.into();
        hero["health"] = 100.into();
    }
    setup
}

/// The stats a setup gives to the card of this id (One 0..29, Two 30..59).
fn card_of(setup: &Value, id: u64) -> &Value {
    &setup[SIDE_KEYS[(id / 30) as usize]]["cards"][(id % 30) as usize]
}

fn is_provocateur(setup: &Value, id: u64) -> bool {
    card_of(setup, id)["provocation"] == true
}

/// The active player summons a filler and so hands the token over.
fn pass(t: &mut Table) -> Vec<Value> {
    let card = plays(&legal(&t.game))
        .into_iter()
        .find(|id| *card_of(&t.setup, *id) == filler())
        .expect("a filler is still in hand");
    t.act(&play(card))
}

/// Summons `one` and `two` in order (ids), the players taking turns, passing when one has
/// nothing left to summon; ends with One active.
fn deploy_ids(setup: &Value, seed: u32, one: &[u64], two: &[u64]) -> Table {
    let mut t = Table::new(setup, seed);
    let lists = [one, two];
    let mut next = [0usize; 2];
    while next[0] < one.len() || next[1] < two.len() {
        let me = t.active;
        if next[me] < lists[me].len() {
            t.act(&play(lists[me][next[me]]));
            next[me] += 1;
        } else {
            pass(&mut t);
        }
    }
    while t.active != 0 {
        pass(&mut t);
    }
    t
}

/// What provocation allows, from the boards a client rebuilt out of the events and from the
/// setup alone: per attacker in board order, the hero then the opposing board when it holds no
/// provocateur, else the provocateurs only, in board order.
fn expected_attacks(setup: &Value, active: usize, boards: &[Vec<u64>; 2]) -> Vec<(u64, Value)> {
    let foe = &boards[1 - active];
    let taunts: Vec<u64> = foe
        .iter()
        .copied()
        .filter(|id| is_provocateur(setup, *id))
        .collect();
    let mut targets: Vec<Value> = Vec::new();
    if taunts.is_empty() {
        targets.push(json!("hero"));
        targets.extend(foe.iter().map(|id| json!({"creature": *id})));
    } else {
        targets.extend(taunts.iter().map(|id| json!({"creature": *id})));
    }
    boards[active]
        .iter()
        .flat_map(|a| targets.iter().map(move |target| (*a, target.clone())))
        .collect()
}

/// The targets one attacker is offered, in order.
fn targets_of(actions: &[Value], attacker: u64) -> Vec<Value> {
    attacks(actions)
        .into_iter()
        .filter(|(a, _)| *a == attacker)
        .map(|(_, target)| target)
        .collect()
}

fn creature(id: u64) -> Value {
    json!({"creature": id})
}

// ---------------------------------------------------------------------------------------
// Case 21: the data, and the card ids the other cases rely on
// ---------------------------------------------------------------------------------------

/// The ids of a deck are the positions of its cards in the setup, whichever side starts:
/// a distinct cost per position, read back from the `played` event of that very id.
#[test]
fn a_card_id_is_its_position_in_its_deck_in_the_setup() {
    for (starter, base) in [("one", 0u64), ("two", 30)] {
        let mut setup = fixture();
        for (side, initiative) in [("one", 1), ("two", 1)] {
            setup[side]["hero"]["initiative"] = initiative.into();
            setup[side]["hero"]["starting_hand"] = 30.into();
            setup[side]["hero"]["effects"] = json!([]);
            setup[side]["hero"]["max_time"] = 100.into();
            setup[side]["hero"]["start_budget"] = 100.into();
            let cards: Vec<Value> = (0..30)
                .map(|i| stats_json(i + 1, 1, 1, 1, false, 0))
                .collect();
            set_side_cards(&mut setup, side, cards);
        }
        setup[starter]["hero"]["initiative"] = 9.into();
        let mut hand = plays(&legal(&start(&setup, 4)));
        hand.sort_unstable();
        assert_eq!(hand, (base..base + 30).collect::<Vec<u64>>(), "{starter}");
        for position in 0..30u64 {
            let mut game = start(&setup, 4);
            let events = apply(&mut game, &play(base + position));
            assert_eq!(
                find(&events, "played").unwrap()["cost"],
                position + 1,
                "{starter} card {}",
                base + position
            );
        }
    }
}

#[test]
fn provocation_and_riposte_are_optional_data_and_the_old_shape_behaves_as_before() {
    // Cards of the S1/S2a shape (four keys) against the same cards with the defaults spelled
    // out: they start, and every kind of duel gives the very same journals.
    let mut legacy = fixture();
    for side in SIDE_KEYS {
        let cards: Vec<Value> = (0..30u32)
            .map(|i| {
                json!({"cost": 1 + i % 3, "force": 1 + i % 2, "attack_cost": 1 + i % 3,
                       "health": 1 + i % 5})
            })
            .collect();
        set_side_cards(&mut legacy, side, cards);
    }
    let mut explicit = legacy.clone();
    for side in SIDE_KEYS {
        for card in explicit[side]["cards"].as_array_mut().unwrap() {
            card["provocation"] = false.into();
            card["riposte"] = 0.into();
        }
    }
    assert!(legacy["one"]["cards"][0].get("provocation").is_none());
    assert_eq!(explicit["two"]["cards"][7]["riposte"], 0);
    for seed in 0..8 {
        let old = run_policy(&legacy, seed, creature_first);
        assert_eq!(
            old,
            run_policy(&explicit, seed, creature_first),
            "seed {seed}"
        );
        assert_eq!(
            run_policy(&legacy, seed, pseudo_random(seed)),
            run_policy(&explicit, seed, pseudo_random(seed)),
            "seed {seed}"
        );
        assert!(
            old.iter()
                .all(|(_, _, events)| count(events, "riposted") == 0),
            "seed {seed}: no riposte without the data"
        );
    }
}

#[test]
fn a_setup_with_provocation_and_riposte_starts_and_hostile_forms_are_refused() {
    let mut base = fixture();
    uniform_stats(&mut base, 1, 1, 1, 1);
    let with = |field: &str, value: Value| {
        let mut setup = base.clone();
        setup["one"]["cards"][4][field] = value;
        setup
    };

    // Valid values start (each hostile form below is first checked against its valid twin).
    for (field, good) in [
        ("provocation", json!(true)),
        ("provocation", json!(false)),
        ("riposte", json!(0)),
        ("riposte", json!(3)),
        ("riposte", json!(10_000)),
    ] {
        start(&with(field, good.clone()), 1);
        assert!(
            Duel::from_json(&with(field, good).to_string(), 1).is_ok(),
            "{field}"
        );
    }
    let setup = arena(
        vec![taunt(2), riposter(2, 5)],
        vec![stats_json(1, 1, 1, 1, true, 9)],
    );
    assert!(!plays(&legal(&start(&setup, 1))).is_empty());

    // Wrong JSON types and shapes never reach the rules.
    let hostile_riposte = [
        ("negative", "-1"),
        ("a float", "1.5"),
        ("a string", r#""3""#),
        ("null", "null"),
        ("2^32", "4294967296"),
        ("a boolean", "true"),
        ("an array", "[3]"),
    ];
    for (what, raw) in hostile_riposte {
        let value: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(
            setup_error(&with("riposte", value), 1),
            "invalid_json",
            "riposte {what}"
        );
    }
    let hostile_provocation = [
        ("1", "1"),
        ("0", "0"),
        ("the string true", r#""true""#),
        ("null", "null"),
        ("an array", "[]"),
        ("an object", "{}"),
    ];
    for (what, raw) in hostile_provocation {
        let value: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(
            setup_error(&with("provocation", value), 1),
            "invalid_json",
            "provocation {what}"
        );
    }
    assert_eq!(
        setup_error(&with("keyword", true.into()), 1),
        "invalid_json",
        "an unknown key is still refused"
    );

    // Well-formed but out of bounds: `invalid_setup`, from `MAX_STAT` + 1 up to `u32::MAX`.
    start(&with("riposte", 10_000.into()), 1);
    for too_big in [10_001u64, 99_999, u64::from(u32::MAX)] {
        assert_eq!(
            setup_error(&with("riposte", too_big.into()), 1),
            "invalid_setup",
            "riposte {too_big}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Case 22: the targets offered under provocation
// ---------------------------------------------------------------------------------------

/// Two's chosen cards: 30 is a provocateur, 31 is not, 32 and 33 are provocateurs.
fn two_with_taunts() -> Vec<Value> {
    vec![taunt(3), plain(3), taunt(3), taunt(3)]
}

/// The attacks of `actions` come before Concede, which is last and alone.
fn assert_attacks_precede_concede(actions: &[Value]) {
    assert_eq!(actions.last(), Some(&json!("concede")));
    assert_eq!(actions.iter().filter(|a| kind(a) == "concede").count(), 1);
    let last_attack = actions.iter().rposition(|a| kind(a) == "attack");
    assert!(last_attack.is_none_or(|i| i < actions.len() - 1));
    assert!(
        actions
            .iter()
            .take_while(|a| kind(a) != "attack")
            .all(|a| ["play", "activate"].contains(&kind(a).as_str())),
        "hand and effects come before the attacks"
    );
}

#[test]
fn without_a_provocateur_every_attacker_is_offered_the_hero_then_each_opposing_creature() {
    for seed in 0..5 {
        // Two holds a ripostant: riposte alone restricts nothing.
        let setup = arena(vec![plain(3), plain(3)], vec![plain(3), riposter(3, 2)]);
        let t = deploy_ids(&setup, seed, &[0, 1], &[30, 31]);
        let actions = legal(&t.game);
        assert_eq!(
            attacks(&actions),
            [
                (0, json!("hero")),
                (0, creature(30)),
                (0, creature(31)),
                (1, json!("hero")),
                (1, creature(30)),
                (1, creature(31)),
            ],
            "seed {seed}"
        );
        assert_attacks_precede_concede(&actions);
    }
}

#[test]
fn with_provocateurs_only_they_are_offered_in_board_order_and_never_the_hero() {
    for seed in 0..5 {
        // (summoning order of Two's cards, the provocateurs among them in board order)
        let cases: [(&[u64], &[u64]); 3] = [
            (&[30, 31], &[30]),
            (&[30, 31, 32], &[30, 32]),
            (&[33, 31, 32, 30], &[33, 32, 30]), // board order, not id order
        ];
        for (summoned, taunts) in cases {
            let setup = arena(vec![plain(3), plain(3)], two_with_taunts());
            let t = deploy_ids(&setup, seed, &[0, 1], summoned);
            let actions = legal(&t.game);
            let wanted: Vec<Value> = taunts.iter().map(|id| creature(*id)).collect();
            for attacker in t.boards[0].clone() {
                assert_eq!(
                    targets_of(&actions, attacker),
                    wanted,
                    "seed {seed}, board {summoned:?}, attacker {attacker}"
                );
            }
            assert_eq!(
                attacks(&actions),
                expected_attacks(&t.setup, t.active, &t.boards),
                "the model agrees"
            );
            assert!(attacks(&actions).iter().all(|(_, target)| target != "hero"));
            assert_attacks_precede_concede(&actions);

            // What is offered is accepted (offered means legal), on a twin each.
            for (attacker, target) in attacks(&actions) {
                let mut twin = t.fork();
                let events = twin.act(&json!({"attack": {"attacker": attacker, "target": target}}));
                assert_eq!(kind(&events[0]), "attacked");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Case 23: refusals under provocation
// ---------------------------------------------------------------------------------------

/// The ids of `side`'s cards that sit in none of its zones a client can see: its pile.
fn pile_of(t: &Table, side: usize) -> Vec<u64> {
    let base = 30 * side as u64;
    (base..base + 30)
        .filter(|id| {
            ![&t.hands[side], &t.boards[side], &t.discards[side]]
                .iter()
                .any(|zone| zone.contains(id))
        })
        .collect()
}

#[test]
fn a_target_forbidden_by_provocation_is_identifiable_and_hidden_zones_stay_indistinguishable() {
    // One: creatures 0 and 1. Two: provocateurs 30 (1 health) and 31, plain 32; one card of
    // its deck stays in the pile. 30 dies before the position under test, so a dead
    // provocateur and a live one are both around.
    // The options off: the hero is attacked in the checks below and its attackers must survive.
    let mut setup = legacy(arena(
        vec![plain(3), plain(3)],
        vec![taunt(1), taunt(5), plain(5)],
    ));
    setup["two"]["hero"]["starting_hand"] = 29.into();
    let mut t = Table::new(&setup, 6);
    t.act(&play(0));
    t.act(&play(30));
    t.act(&play(1));
    t.act(&play(31));
    let events = t.act(&strike(0, 30)); // a provocateur may be attacked
    assert_eq!(count(&events, "died"), 1);
    t.act(&play(32));
    assert_eq!(t.active, 0);
    assert_eq!(t.boards, [vec![0, 1], vec![31, 32]]);
    assert_eq!(t.discards[1], [30]);
    let pile = pile_of(&t, 1);
    assert_eq!(
        pile.len(),
        1,
        "Two's deck holds one card in its pile: {pile:?}"
    );
    let two_hand = t.hands[1][0];
    let own_hand = *plays(&legal(&t.game)).last().unwrap();
    let before = t.game.legal_actions_json().unwrap();
    assert_eq!(targets_of(&legal(&t.game), 0), [creature(31)]);
    let mut twin = t.fork();

    // The hero and a plain creature are forbidden: one identifiable code, whoever attacks.
    let mut forbidden = Vec::new();
    for attacker in [0, 1] {
        for target in [json!("hero"), creature(32)] {
            let action = json!({"attack": {"attacker": attacker, "target": target}});
            let err = t.game.apply_json(&action.to_string()).unwrap_err();
            assert_eq!(
                error_code(&err),
                "target_forbidden_by_provocation",
                "{action}"
            );
            forbidden.push(err);
        }
    }
    assert!(forbidden.windows(2).all(|w| w[0] == w[1]));

    // Everything that is not a creature of the opposing board keeps `target_not_on_board`,
    // although a provocateur is in play: a dead provocateur, a creature of its own side,
    // cards of either hand, the pile, unknown ids.
    let hostile = [
        ("a dead provocateur", 30),
        ("its own other creature", 1),
        ("itself", 0),
        ("a card of its own hand", own_hand),
        ("a card of the opponent's hand", two_hand),
        ("a card of the opponent's pile", pile[0]),
        ("an unknown id", 9999),
        ("u32::MAX", u64::from(u32::MAX)),
    ];
    let mut hidden = Vec::new();
    for (what, id) in hostile {
        let err = t.game.apply_json(&strike(0, id).to_string()).unwrap_err();
        assert_eq!(error_code(&err), "target_not_on_board", "{what}");
        hidden.push(err);
    }
    // Sweep of every id but the two live opposing creatures: still no oracle on any zone.
    for id in (0..200u64).filter(|id| ![31, 32].contains(id)) {
        let err = t.game.apply_json(&strike(1, id).to_string()).unwrap_err();
        assert_eq!(error_code(&err), "target_not_on_board", "id {id}");
        hidden.push(err);
    }
    assert!(
        hidden.windows(2).all(|w| w[0] == w[1]),
        "refusals differ by zone"
    );
    assert_ne!(hidden[0], forbidden[0], "the two refusals are told apart");

    // An absent attacker comes first, even for a forbidden target, a hidden one or both.
    // (a dead creature, the opponent's creature, a card of either hand, an unknown id)
    for attacker in [30, 31, two_hand, own_hand, 9999] {
        for target in [json!("hero"), creature(32), creature(9999)] {
            let action = json!({"attack": {"attacker": attacker, "target": target}});
            assert_eq!(
                refusal(&mut t.game, &action),
                "attacker_not_on_board",
                "{action}"
            );
        }
    }
    // Nothing moved: the same actions are offered, and the twin that never saw any of this
    // answers the next moves identically.
    assert_eq!(t.game.legal_actions_json().unwrap(), before);
    assert_twins_agree(&mut t.game, &mut twin.game, 8);

    // The provocateur itself is a valid target.
    let events = twin.act(&strike(0, 31));
    assert_eq!(kind(&events[0]), "attacked");
}

// ---------------------------------------------------------------------------------------
// Case 24: the targets are recomputed after every action
// ---------------------------------------------------------------------------------------

#[test]
fn killing_the_provocateur_between_two_attacks_gives_the_hero_back() {
    for seed in 0..4 {
        let setup = arena(vec![plain(5), plain(5)], vec![taunt(1), plain(5)]);
        let mut t = deploy_ids(&setup, seed, &[0, 1], &[30, 31]);
        assert_eq!(
            attacks(&legal(&t.game)),
            [(0, creature(30)), (1, creature(30))],
            "seed {seed}: the provocateur shields the hero and the other creature"
        );
        assert_eq!(
            refusal(&mut t.game, &strike_hero(0)),
            "target_forbidden_by_provocation"
        );

        let events = t.act(&strike(0, 30));
        assert_eq!(
            find(&events, "died"),
            Some(&json!({"player": "Two", "creature": 30}))
        );
        // Two's turn: One holds no provocateur, so its own attackers see the hero.
        assert_eq!(
            attacks(&legal(&t.game)),
            [(31, json!("hero")), (31, creature(0)), (31, creature(1))]
        );
        pass(&mut t);
        let filler_id = t.boards[1][1];
        let offered = legal(&t.game);
        for attacker in [0, 1] {
            assert_eq!(
                targets_of(&offered, attacker),
                [json!("hero"), creature(31), creature(filler_id)],
                "seed {seed}: the hero is offered again"
            );
        }
        assert_eq!(attacks(&offered), expected_attacks(&t.setup, 0, &t.boards));
        let events = t.act(&strike_hero(1));
        assert_eq!(kinds(&events)[..2], ["attacked", "damaged"]);
    }
}

#[test]
fn summoning_a_provocateur_between_two_attacks_restricts_the_targets_at_once() {
    for seed in 0..4 {
        // The options off: the attackers survive their hits on the hero.
        let setup = legacy(arena(vec![plain(5), plain(5)], vec![plain(5), taunt(5)]));
        let mut t = deploy_ids(&setup, seed, &[0, 1], &[30]);
        let filler_id = t.boards[1][1];
        assert_eq!(
            targets_of(&legal(&t.game), 0),
            [json!("hero"), creature(30), creature(filler_id)]
        );
        let events = t.act(&strike_hero(0));
        assert_eq!(kinds(&events)[..2], ["attacked", "damaged"]);

        t.act(&play(31)); // Two summons its provocateur
        let offered = legal(&t.game);
        for attacker in [0, 1] {
            assert_eq!(
                targets_of(&offered, attacker),
                [creature(31)],
                "seed {seed}"
            );
        }
        assert_eq!(
            refusal(&mut t.game, &strike_hero(0)),
            "target_forbidden_by_provocation"
        );
        assert_eq!(
            refusal(&mut t.game, &strike(1, 30)),
            "target_forbidden_by_provocation"
        );
        assert_eq!(
            refusal(&mut t.game, &strike(1, filler_id)),
            "target_forbidden_by_provocation"
        );
    }
}

#[test]
fn each_side_aims_only_at_the_provocateurs_of_the_other() {
    let setup = arena(vec![taunt(5), plain(5)], vec![taunt(5), plain(5)]);
    let mut t = deploy_ids(&setup, 2, &[0, 1], &[30, 31]);
    assert_eq!(
        attacks(&legal(&t.game)),
        [(0, creature(30)), (1, creature(30))],
        "One's attackers, its own provocateur included, aim at Two's"
    );
    assert_eq!(
        refusal(&mut t.game, &strike_hero(0)),
        "target_forbidden_by_provocation"
    );
    t.act(&strike(0, 30));
    assert_eq!(
        attacks(&legal(&t.game)),
        [(30, creature(0)), (31, creature(0))],
        "Two's attackers aim at One's provocateur and neither at its other creature nor the hero"
    );
    assert_eq!(
        refusal(&mut t.game, &strike(30, 1)),
        "target_forbidden_by_provocation"
    );
    assert_eq!(
        refusal(&mut t.game, &strike_hero(31)),
        "target_forbidden_by_provocation"
    );
}

#[test]
fn a_provocateur_attacks_freely_when_the_opponent_has_none() {
    // The options off: the provocateur survives its hit on the hero.
    let setup = legacy(arena(vec![taunt(5)], vec![plain(5)]));
    let mut t = deploy_ids(&setup, 3, &[0], &[30]);
    assert_eq!(
        attacks(&legal(&t.game)),
        [(0, json!("hero")), (0, creature(30))]
    );
    let events = t.act(&strike_hero(0));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "damaged",
            "token_moved",
            "active_player_changed"
        ]
    );
    // Its own provocation shields it from Two, though: only 0 may be hit.
    assert_eq!(attacks(&legal(&t.game)), [(30, creature(0))]);
}

// ---------------------------------------------------------------------------------------
// Case 25: a riposte that does not kill
// ---------------------------------------------------------------------------------------

#[test]
fn a_ripostant_hits_back_in_the_journal_and_the_attacker_dies_of_a_later_riposte() {
    // A (id 0): 5 health. X (id 30): 5 health, riposte 2. Z (id 31): riposte 0.
    // The options off: the hero fork below hits the hero without losing its attacker.
    let setup = legacy(arena(vec![plain(5)], vec![riposter(5, 2), plain(9)]));
    let mut t = deploy_ids(&setup, 5, &[0], &[30, 31]);
    let zero = t.fork();
    let mut hero = t.fork();

    let events = t.act(&strike(0, 30));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(
        events[0],
        json!({"attacked": {"player": "One", "attacker": 0, "cost": 1}})
    );
    assert_eq!(
        events[1],
        json!({"creature_damaged": {"player": "Two", "creature": 30, "amount": 1}})
    );
    assert_eq!(
        events[2],
        json!({"riposted": {"player": "One", "creature": 0, "amount": 2}}),
        "the riposte hits the attacker, its owner is named"
    );
    assert_eq!(
        events[3],
        json!({"token_moved": {"player": "Two", "budget": 1}})
    );
    assert_eq!(
        events[4],
        json!({"active_player_changed": {"player": "Two"}})
    );
    assert_eq!(t.boards[0][0], 0, "the attacker survived the first riposte");

    // Second hit: still alive (5 - 2 - 2 = 1), the log is the same.
    pass(&mut t);
    let events = t.act(&strike(0, 30));
    assert_eq!(count(&events, "riposted"), 1);
    assert_eq!(count(&events, "died"), 0);
    pass(&mut t);
    assert!(attacks(&legal(&t.game)).contains(&(0, creature(30))));

    // Third hit: 1 health left, the riposte (2, nominal) kills the attacker.
    let events = t.act(&strike(0, 30));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "died",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(
        find(&events, "riposted"),
        Some(&json!({"player": "One", "creature": 0, "amount": 2}))
    );
    assert_eq!(
        find(&events, "died"),
        Some(&json!({"player": "One", "creature": 0}))
    );
    assert_eq!(t.discards[0], [0]);

    // The dead attacker acts no more; the target, hit three times for 1, is alive.
    assert_eq!(
        refusal(&mut t.game, &strike(0, 30)),
        "attacker_not_on_board"
    );
    assert_eq!(
        refusal(&mut t.game, &strike_hero(0)),
        "attacker_not_on_board"
    );
    assert!(t.boards[1].contains(&30));

    // Riposte 0: no `riposted`, however often, and the attacker is left untouched.
    let mut zero = zero;
    for hit in 0..5 {
        let events = zero.act(&strike(0, 31));
        assert_eq!(
            kinds(&events),
            [
                "attacked",
                "creature_damaged",
                "token_moved",
                "active_player_changed"
            ],
            "hit {hit}"
        );
        pass(&mut zero);
    }
    assert!(attacks(&legal(&zero.game)).iter().any(|(a, _)| *a == 0));

    // An attack on the hero is never riposted, though ripostants stand on the board.
    let events = hero.act(&strike_hero(0));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "damaged",
            "token_moved",
            "active_player_changed"
        ]
    );
}

// ---------------------------------------------------------------------------------------
// Case 26: a target killed by the attack still ripostes
// ---------------------------------------------------------------------------------------

#[test]
fn a_target_killed_by_the_attack_ripostes_and_the_wounds_add_up() {
    // A (id 0): 5 health. X (30): 1 health, riposte 2. Y1 (31) and Y2 (32): 9 health, riposte 2.
    let setup = arena(
        vec![plain(5)],
        vec![riposter(1, 2), riposter(9, 2), riposter(9, 2)],
    );
    let mut t = deploy_ids(&setup, 7, &[0], &[30, 31, 32]);

    let events = t.act(&strike(0, 30));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "died",
            "token_moved",
            "active_player_changed"
        ],
        "the riposte comes before the death of its author"
    );
    assert_eq!(
        find(&events, "riposted"),
        Some(&json!({"player": "One", "creature": 0, "amount": 2}))
    );
    assert_eq!(
        find(&events, "died"),
        Some(&json!({"player": "Two", "creature": 30}))
    );

    // The wound is real: A has 3 left, so the next riposte leaves 1 and the one after kills it.
    pass(&mut t);
    let events = t.act(&strike(0, 31));
    assert_eq!(count(&events, "riposted"), 1);
    assert_eq!(count(&events, "died"), 0);
    pass(&mut t);
    let events = t.act(&strike(0, 32));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "died",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(
        find(&events, "died"),
        Some(&json!({"player": "One", "creature": 0}))
    );
}

// ---------------------------------------------------------------------------------------
// Case 27: simultaneous deaths
// ---------------------------------------------------------------------------------------

/// A 1-health creature of riposte 1 on each side: any attack on the other kills both.
fn mutual_kill_setup() -> Value {
    arena(vec![riposter(1, 1)], vec![riposter(1, 1)])
}

fn assert_neither_is_ever_offered_nor_accepted(t: &mut Table, dead: [u64; 2]) {
    let mut checked = 0;
    while checked < 2 {
        // A player without a creature has no attack to examine: it summons a filler.
        if t.boards[t.active].is_empty() {
            pass(t);
            continue;
        }
        let offered = legal(&t.game);
        assert!(!attacks(&offered).is_empty());
        for (attacker, target) in attacks(&offered) {
            assert!(
                !dead.contains(&attacker),
                "a dead creature attacks: {attacker}"
            );
            assert!(target == "hero" || !dead.iter().any(|d| target == creature(*d)));
        }
        // The refusals go to one twin only; both then play on identically.
        let (mut refused, mut spared) = (t.fork(), t.fork());
        let own = t.boards[t.active][0];
        for d in dead {
            assert_eq!(
                refusal(&mut refused.game, &strike(d, own)),
                "attacker_not_on_board"
            );
            assert_eq!(
                refusal(&mut refused.game, &strike_hero(d)),
                "attacker_not_on_board"
            );
            assert_eq!(
                refusal(&mut refused.game, &strike(own, d)),
                "target_not_on_board"
            );
        }
        assert_twins_agree(&mut refused.game, &mut spared.game, 6);
        checked += 1;
        pass(t);
    }
}

#[test]
fn an_attacker_and_its_target_that_die_together_are_buried_in_id_order_both_ways() {
    // The attacker has the smaller id: One's 0 attacks Two's 30.
    let mut t = deploy_ids(&mutual_kill_setup(), 1, &[0], &[30]);
    let events = t.act(&strike(0, 30));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "died",
            "died",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(events[3], json!({"died": {"player": "One", "creature": 0}}));
    assert_eq!(
        events[4],
        json!({"died": {"player": "Two", "creature": 30}})
    );
    assert_eq!(
        (t.discards[0].clone(), t.discards[1].clone()),
        (vec![0], vec![30])
    );
    assert_neither_is_ever_offered_nor_accepted(&mut t, [0, 30]);

    // The attacker has the larger id: Two's 30 attacks One's 0; the target is buried first.
    let mut t = deploy_ids(&mutual_kill_setup(), 1, &[0], &[30]);
    pass(&mut t); // One gives the token to Two
    assert_eq!(t.active, 1);
    let events = t.act(&strike(30, 0));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "died",
            "died",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(
        events[0],
        json!({"attacked": {"player": "Two", "attacker": 30, "cost": 1}})
    );
    assert_eq!(
        events[2],
        json!({"riposted": {"player": "Two", "creature": 30, "amount": 1}})
    );
    assert_eq!(events[3], json!({"died": {"player": "One", "creature": 0}}));
    assert_eq!(
        events[4],
        json!({"died": {"player": "Two", "creature": 30}})
    );
    assert_neither_is_ever_offered_nor_accepted(&mut t, [0, 30]);
}

// ---------------------------------------------------------------------------------------
// Case 28: the time rule on a ripostable attack
// ---------------------------------------------------------------------------------------

/// One (id 0: cost 3, `attack_cost`, `health`) holds 3 time units when its turn comes again,
/// against Two's ripostant (id 30: 9 health, riposte 1; Two caps at 5).
fn timing_arena(attack_cost: u32, health: u32) -> Table {
    let mut setup = arena(
        vec![stats_json(3, 1, attack_cost, health, false, 0)],
        vec![stats_json(3, 1, 1, 9, false, 1)],
    );
    setup["one"]["hero"]["start_budget"] = 3.into();
    setup["two"]["hero"]["max_time"] = 5.into();
    let mut t = Table::new(&setup, 8);
    t.act(&play(0)); // 3 = 3: the token crosses, Two holds 1
    let events = t.act(&play(30)); // 3 > 1: One gets 3 - 1 + 1
    assert_eq!(token(&events), ("One".into(), 3));
    t
}

#[test]
fn a_riposted_attack_pays_its_time_whether_or_not_the_attacker_dies() {
    for health in [1u32, 9] {
        for (attack_cost, (receiver, budget)) in [
            (1u32, ("One", 2u64)), // c < b: One keeps the token with 3 - 1
            (3, ("Two", 1)),       // c = b: the token crosses
            (5, ("Two", 3)),       // c > b: 5 - 3 + 1
            (9, ("Two", 5)),       // c > b: 9 - 3 + 1 = 7, capped by Two's max_time
        ] {
            let what = format!("attack cost {attack_cost}, attacker health {health}");
            let mut t = timing_arena(attack_cost, health);
            let events = t.act(&strike(0, 30));
            let mut expected = vec!["attacked", "creature_damaged", "riposted"];
            if health == 1 {
                expected.push("died");
            }
            expected.push("token_moved");
            if receiver == "Two" {
                expected.push("active_player_changed");
            }
            assert_eq!(kinds(&events), expected, "{what}");
            assert_eq!(
                find(&events, "attacked").unwrap()["cost"],
                attack_cost,
                "{what}"
            );
            assert_eq!(token(&events), (receiver.to_owned(), budget), "{what}");
            assert_eq!(SIDES[t.active], receiver, "{what}");
            if health == 1 {
                assert_eq!(
                    find(&events, "died"),
                    Some(&json!({"player": "One", "creature": 0})),
                    "{what}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Case 29: exhaustion by riposte
// ---------------------------------------------------------------------------------------

/// One holds `one_hand` cards, all the same (cost 1, force 1, 1 health, `attack_cost`), and no
/// effect; Two's chosen cards come in `two_deck`, `two_hand` of them in hand (no effect either).
/// One summons its first card (Two holds 1 unit), Two answers with a card of cost 3 (id 30 when
/// its whole deck is in hand, whose id is returned): One then holds 3 units and is to move.
fn exhaustion_table(
    one_hand: u32,
    attack_cost: u32,
    two_deck: Vec<Value>,
    two_hand: u32,
) -> (Table, u64) {
    let mut setup = arena(vec![], vec![]);
    set_side_cards(
        &mut setup,
        "one",
        vec![stats_json(1, 1, attack_cost, 1, false, 0); 30],
    );
    set_side_cards(&mut setup, "two", two_deck);
    setup["one"]["hero"]["starting_hand"] = one_hand.into();
    setup["two"]["hero"]["starting_hand"] = two_hand.into();
    let mut t = Table::new(&setup, 3);
    t.summon();
    // With the whole deck in hand the card is the one at 30; else the only one there is.
    let x = if two_hand == 30 {
        30
    } else {
        plays(&legal(&t.game))[0]
    };
    let events = t.act(&play(x));
    assert_eq!(token(&events), ("One".into(), 3));
    (t, x)
}

/// A big ripostant at 30, then fillers.
fn big_ripostant_deck() -> Vec<Value> {
    roster(vec![stats_json(3, 1, 1, 9, false, 1)])
}

/// Two's only card type: dies to any hit, ripostes 1.
fn glass_deck() -> Vec<Value> {
    vec![stats_json(3, 1, 1, 1, false, 1); 30]
}

#[test]
fn a_riposte_that_kills_the_last_creature_of_the_player_who_stays_active_ends_the_game() {
    // One's only card dies of the riposte; One keeps the token (1 < 3) with nothing to do.
    let (mut t, x) = exhaustion_table(1, 1, big_ripostant_deck(), 30);
    let a = t.boards[0][0];
    let events = t.act(&strike(a, x));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "died",
            "token_moved",
            "game_won"
        ]
    );
    assert_eq!(
        events[4],
        json!({"token_moved": {"player": "One", "budget": 2}})
    );
    assert_eq!(
        events.last(),
        Some(&json!({"game_won": {"winner": "Two", "reason": "exhaustion"}}))
    );
    every_action_is_refused_as_game_over(&mut t.game);

    // Mutual death, the attacker stays active: it is its own exhaustion that counts.
    let (mut t, x) = exhaustion_table(1, 1, glass_deck(), 1);
    let a = t.boards[0][0];
    let events = t.act(&strike(a, x));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "died",
            "died",
            "token_moved",
            "game_won"
        ]
    );
    assert_eq!(
        events.last(),
        Some(&json!({"game_won": {"winner": "Two", "reason": "exhaustion"}}))
    );
    every_action_is_refused_as_game_over(&mut t.game);
}

#[test]
fn with_a_handover_the_exhaustion_is_the_receivers() {
    // The attacker dies of the riposte but the token crosses (3 = 3) to Two, which has its
    // ripostant and its hand: nobody is exhausted, the game goes on.
    let (mut t, x) = exhaustion_table(1, 3, big_ripostant_deck(), 30);
    let a = t.boards[0][0];
    let events = t.act(&strike(a, x));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "died",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(t.active, 1);
    assert!(!legal(&t.game).is_empty());
    assert!(attacks(&legal(&t.game)).iter().any(|(att, _)| *att == 30));

    // Both die and the token crosses to Two, now with no card and no creature: its defeat.
    let (mut t, x) = exhaustion_table(1, 3, glass_deck(), 1);
    let a = t.boards[0][0];
    let events = t.act(&strike(a, x));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "riposted",
            "died",
            "died",
            "token_moved",
            "active_player_changed",
            "game_won"
        ]
    );
    assert_eq!(
        events[5],
        json!({"token_moved": {"player": "Two", "budget": 1}})
    );
    assert_eq!(
        events.last(),
        Some(&json!({"game_won": {"winner": "One", "reason": "exhaustion"}}))
    );
    every_action_is_refused_as_game_over(&mut t.game);
}

#[test]
fn a_card_in_hand_or_another_creature_keeps_the_player_in_the_game() {
    // A card left in hand: One can still summon.
    let (mut t, x) = exhaustion_table(2, 1, big_ripostant_deck(), 30);
    let a = t.boards[0][0];
    let events = t.act(&strike(a, x));
    assert_eq!(count(&events, "died"), 1);
    assert_eq!(count(&events, "game_won"), 0);
    assert_eq!(t.active, 0);
    assert_eq!(plays(&legal(&t.game)).len(), 1);

    // Another creature on the board: One can still attack with it.
    let (mut t, x) = exhaustion_table(2, 1, big_ripostant_deck(), 30);
    let (a, b) = (t.boards[0][0], t.summon()); // the second summoning costs 1 of the 3 units
    assert_eq!(t.active, 0);
    let events = t.act(&strike(a, x));
    assert_eq!(count(&events, "died"), 1, "a dies of the riposte");
    assert_eq!(count(&events, "game_won"), 0);
    assert_eq!(t.active, 0);
    assert!(attacks(&legal(&t.game)).iter().any(|(att, _)| *att == b));
    assert!(
        plays(&legal(&t.game)).is_empty(),
        "the hand is empty: only the board keeps it alive"
    );
}

// ---------------------------------------------------------------------------------------
// Case 30: whole duels with provocation and riposte
// ---------------------------------------------------------------------------------------

/// 30 different creatures: costs, forces and attack costs from 1 to 3, health from 1 to 6, a
/// provocateur every fourth card, riposte from 0 to 3. `salt` shifts the pattern.
fn mixed_deck(salt: u32) -> Vec<Value> {
    (0..30u32)
        .map(|i| {
            let n = i + salt;
            stats_json(
                1 + n % 3,
                1 + (n / 2) % 3,
                1 + (n / 3) % 3,
                1 + (n * 7) % 6,
                n % 4 == 1,
                (n / 5 + n) % 4,
            )
        })
        .collect()
}

/// The fixture's heroes (their draw effects, their health) around two mixed decks.
fn mixed_setup(one_starts: bool) -> Value {
    let mut setup = fixture();
    set_side_cards(&mut setup, "one", mixed_deck(0));
    set_side_cards(&mut setup, "two", mixed_deck(11));
    if !one_starts {
        setup["one"]["hero"]["initiative"] = 1.into();
        setup["two"]["hero"]["initiative"] = 2.into();
    }
    setup
}

/// What a finished duel taught about provocation and riposte.
#[derive(Default)]
struct Observed {
    ripostes: usize,
    deaths: usize,
    /// Steps where a provocateur hid the hero from an attacker.
    shielded: usize,
    hero_down: usize,
    exhaustion: usize,
}

/// Replays a journal from the events alone (boards rebuilt from `played` and `died`) and checks
/// every step against the rules of provocation and riposte.
fn assert_a_provocation_duel_is_sound(what: &str, setup: &Value, journal: &[Step]) -> Observed {
    let initiative = |side: &str| setup[side]["hero"]["initiative"].as_u64().unwrap();
    let mut active = usize::from(initiative("one") < initiative("two"));
    let mut boards: [Vec<u64>; 2] = Default::default();
    let mut dead: Vec<u64> = Vec::new();
    let mut seen = Observed::default();
    for (i, (offered, action, events)) in journal.iter().enumerate() {
        let actions: Vec<Value> = serde_json::from_str(offered).unwrap();
        assert!(actions.contains(action), "{what} step {i}: not offered");
        assert_ne!(action, &json!("concede"), "{what} step {i}: no concession");

        // An offered attack never contravenes provocation: the offer is exactly the model's.
        let wanted = expected_attacks(setup, active, &boards);
        assert_eq!(
            attacks(&actions),
            wanted,
            "{what} step {i}: offered attacks"
        );
        let foe_taunts = boards[1 - active]
            .iter()
            .any(|id| is_provocateur(setup, *id));
        if foe_taunts && !boards[active].is_empty() {
            seen.shielded += 1;
        }
        // No dead creature is offered again, to attack, to be attacked or to be played.
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

        // The attack taken: a riposte exactly when it aims at a creature with riposte >= 1,
        // damage and cost read straight from the data.
        if kind(action) == "attack" {
            let attacker = action["attack"]["attacker"].as_u64().unwrap();
            let target = action["attack"]["target"]
                .get("creature")
                .and_then(Value::as_u64);
            let riposte = target.map_or(0, |t| card_of(setup, t)["riposte"].as_u64().unwrap());
            assert_eq!(
                count(events, "riposted"),
                usize::from(riposte >= 1),
                "{what} step {i}"
            );
            assert_eq!(
                events[0]["attacked"]["cost"],
                card_of(setup, attacker)["attack_cost"],
                "{what} step {i}"
            );
            if let Some(riposted) = find(events, "riposted") {
                assert_eq!(riposted["creature"], attacker, "{what} step {i}");
                assert_eq!(riposted["amount"], riposte, "{what} step {i}");
                assert_eq!(side_of(&riposted["player"]), active, "{what} step {i}");
                seen.ripostes += 1;
            }
        }

        // The journal: deaths in id order, each preceded by the damage that caused it, time last.
        let mut last_death: Option<u64> = None;
        for (at, event) in events.iter().enumerate() {
            match kind(event).as_str() {
                "played" => {
                    assert_eq!(side_of(&event["played"]["player"]), active);
                    boards[active].push(event["played"]["card"].as_u64().unwrap());
                }
                "died" => {
                    let id = event["died"]["creature"].as_u64().unwrap();
                    let side = side_of(&event["died"]["player"]);
                    assert!(
                        boards[side].contains(&id),
                        "{what} step {i}: died off the board"
                    );
                    assert!(
                        last_death.is_none_or(|d| d < id),
                        "{what} step {i}: death order"
                    );
                    last_death = Some(id);
                    boards[side].retain(|c| *c != id);
                    dead.push(id);
                    seen.deaths += 1;
                    let wounded = events[..at].iter().any(|e| {
                        ["creature_damaged", "riposted", "hero_riposted"]
                            .iter()
                            .any(|k| {
                                kind(e) == *k && e[*k]["creature"] == event["died"]["creature"]
                            })
                    });
                    assert!(wounded, "{what} step {i}: died without a wound");
                    assert!(!kinds(&events[..at]).contains(&"token_moved".to_owned()));
                }
                "token_moved" => active = side_of(&event["token_moved"]["player"]),
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
    match won["game_won"]["reason"].as_str().unwrap() {
        "hero_down" => seen.hero_down += 1,
        "exhaustion" => seen.exhaustion += 1,
        other => panic!("{what}: ended by {other}"),
    }
    seen
}

#[test]
fn provocation_and_riposte_duels_are_deterministic_end_by_themselves_and_respect_the_rules() {
    type Policy = (&'static str, fn(u32) -> Box<dyn FnMut(&[Value]) -> Value>);
    let policies: [Policy; 3] = [
        ("creatures first", |_| Box::new(creature_first)),
        ("hero first", |_| Box::new(pick)),
        ("pseudo-random", |seed| Box::new(pseudo_random(seed))),
    ];
    let mut total = Observed::default();
    let mut duels = 0;
    for (starter, one_starts) in [("One", true), ("Two", false)] {
        let setup = mixed_setup(one_starts);
        for (policy, make) in &policies {
            for seed in 0..25 {
                let what = format!("{starter} starts, {policy}, seed {seed}");
                let journal = run_policy(&setup, seed, make(seed));
                assert_eq!(
                    journal,
                    run_policy(&setup, seed, make(seed)),
                    "{what}: not deterministic"
                );
                let seen = assert_a_provocation_duel_is_sound(&what, &setup, &journal);
                total.ripostes += seen.ripostes;
                total.deaths += seen.deaths;
                total.shielded += seen.shielded;
                total.hero_down += seen.hero_down;
                total.exhaustion += seen.exhaustion;
                duels += 1;
            }
        }
    }
    assert_eq!(duels, 150);
    assert!(total.ripostes > 0, "no riposte ever happened");
    assert!(total.deaths > 0);
    assert!(total.shielded > 0, "no provocateur ever shielded a hero");
    assert_eq!(total.hero_down + total.exhaustion, duels);
}

#[test]
fn creature_first_duels_with_ripostants_always_see_a_riposte() {
    let setup = mixed_setup(true);
    for seed in 0..25 {
        let journal = run_policy(&setup, seed, creature_first);
        let ripostes: usize = journal.iter().map(|(_, _, e)| count(e, "riposted")).sum();
        assert!(ripostes > 0, "seed {seed}: no riposte");
    }
}

// ---------------------------------------------------------------------------------------
// Case 31: S1 and S2a are unchanged
// ---------------------------------------------------------------------------------------

#[test]
fn the_fixture_decks_never_riposte_nor_meet_provocation() {
    // The fixture has neither: no `riposted`, and every attack offered is the S2a one (the hero,
    // then the whole opposing board). Cases 1 to 20 above run untouched, as the regression.
    for (name, setup) in [("fixture", fixture()), ("swapped", swapped_fixture())] {
        for seed in 0..10 {
            for journal in [
                run_policy(&setup, seed, creature_first),
                run_policy(&setup, seed, pseudo_random(seed)),
            ] {
                let initiative = |side: &str| setup[side]["hero"]["initiative"].as_u64().unwrap();
                let mut active = usize::from(initiative("one") < initiative("two"));
                let mut boards: [Vec<u64>; 2] = Default::default();
                for (offered, _, events) in &journal {
                    let actions: Vec<Value> = serde_json::from_str(offered).unwrap();
                    assert_eq!(
                        attacks(&actions),
                        expected_attacks(&setup, active, &boards),
                        "{name} seed {seed}"
                    );
                    assert_eq!(count(events, "riposted"), 0, "{name} seed {seed}");
                    for event in events {
                        match kind(event).as_str() {
                            "played" => {
                                boards[active].push(event["played"]["card"].as_u64().unwrap())
                            }
                            "died" => {
                                let side = side_of(&event["died"]["player"]);
                                let id = event["died"]["creature"].as_u64().unwrap();
                                boards[side].retain(|c| *c != id);
                            }
                            "token_moved" => active = side_of(&event["token_moved"]["player"]),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

// =======================================================================================
// S3: two starter decks, two heroes (cases 32 to 40)
// =======================================================================================
//
// The same JSON boundary, on the `data/starter/1.0.0/` set (Rush "rush" against Bulwark
// "bulwark"), loaded with `include_str!` like the fixture. What this file cannot observe is
// locked elsewhere: there is no state accessor, so a card's health, a deck's whole
// distribution and the averages of health are covered by the data tests of the `starter` set
// (T2), not here. What it does observe is observable through the JSON: costs (`played.cost`),
// attack costs (`attacked.cost`), forces (`damaged.amount`), the opening hands and effects of
// the heroes, provocation and riposte in play, the version record of `game_started`, and whole
// duels. The values below come from the data set: if they change, these cases change with it.

const STARTER_CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const STARTER_DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");
const STARTER_VERSION: &str = "starter@1.0.0";

/// The raw JSON decks of the starter set.
fn starter_decks_json() -> Vec<Value> {
    serde_json::from_str(STARTER_DECKS).unwrap()
}

/// The definition ids of a starter deck in deck order: a card's position here is its id's offset
/// in the setup (case 21).
fn deck_cards(deck: &str) -> Vec<String> {
    let decks = starter_decks_json();
    let found = decks.iter().find(|d| d["id"] == deck).expect("a deck");
    found["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect()
}

/// A card definition of the starter `cards.json`, as written in the data.
fn card_def(id: &str) -> Value {
    let cards: Vec<Value> = serde_json::from_str(STARTER_CARDS).unwrap();
    cards
        .into_iter()
        .find(|c| c["id"] == id)
        .expect("a card of the set")
}

/// The ids, in the setup, of every copy of `def` in `deck` once it sits at `base` (One 0, Two 30).
fn ids_of(deck: &str, base: u64, def: &str) -> Vec<u64> {
    deck_cards(deck)
        .iter()
        .enumerate()
        .filter(|(_, c)| *c == def)
        .map(|(i, _)| base + i as u64)
        .collect()
}

/// The starter set as the JSON `GameSetup` a client sends: deck `one` for One, deck `two` for Two.
fn starter_of(one: &str, two: &str) -> Value {
    let cards = parse_cards(STARTER_CARDS).unwrap();
    let decks = parse_decks(STARTER_DECKS).unwrap();
    let side = |id: &str| {
        let index = starter_decks_json()
            .iter()
            .position(|d| d["id"] == id)
            .expect("a deck");
        PlayerSetup {
            hero: decks[index].hero.clone(),
            cards: resolve_deck(&decks[index], &cards)
                .unwrap()
                .into_iter()
                .map(Into::into)
                .collect(),
            card_names: Vec::new(),
        }
    };
    serde_json::to_value(GameSetup {
        one: side(one),
        two: side(two),
        data_versions: vec![STARTER_VERSION.into()],
        rules: RuleSet::default(),
    })
    .unwrap()
}

/// Rush for One (initiative 3, it always starts), Bulwark for Two.
fn starter() -> Value {
    starter_of("rush", "bulwark")
}

/// Bulwark for One, Rush for Two (Rush still starts, whatever its side).
fn starter_swapped() -> Value {
    starter_of("bulwark", "rush")
}

fn started(first: &str, versions: &str) -> String {
    format!(r#"[{{"game_started":{{"first":"{first}","budget":3{versions}}}}}]"#)
}

fn starter_versions() -> String {
    format!(r#","data_versions":["{STARTER_VERSION}"]"#)
}

/// Both starter heroes alternate action after action: no effect, the whole deck in hand, a
/// `max_time` and a budget of 1 so that every action crosses and hands over a budget of 1, and
/// a health no test reaches. One (initiative 9) starts.
fn alternating_starter(one: &str, two: &str) -> Value {
    let mut setup = starter_of(one, two);
    for (side, initiative) in [("one", 9), ("two", 1)] {
        let hero = &mut setup[side]["hero"];
        hero["effects"] = json!([]);
        hero["starting_hand"] = 30.into();
        hero["max_time"] = 1.into();
        hero["start_budget"] = 1.into();
        hero["health"] = 5000.into();
        hero["initiative"] = initiative.into();
    }
    setup
}

/// What the offered attacks must be right now, from what the table knows.
fn check_offer(t: &Table) {
    assert_eq!(
        attacks(&legal(&t.game)),
        expected_attacks(&t.setup, t.active, &t.boards)
    );
}

// ---------------------------------------------------------------------------------------
// Case 32: start and versions
// ---------------------------------------------------------------------------------------

#[test]
fn the_starter_set_starts_with_rush_whatever_the_seed_and_records_its_version() {
    let exact = started("One", &starter_versions());
    assert_eq!(
        exact,
        r#"[{"game_started":{"first":"One","budget":3,"data_versions":["starter@1.0.0"]}}]"#
    );
    for seed in 0..20 {
        let game = start(&starter(), seed);
        assert_eq!(game.start_events_json().unwrap(), exact, "seed {seed}");
    }
    // Rush in the second place still starts, with the same budget.
    let exact = started("Two", &starter_versions());
    for seed in 0..20 {
        let game = start(&starter_swapped(), seed);
        assert_eq!(
            game.start_events_json().unwrap(),
            exact,
            "swapped seed {seed}"
        );
    }
}

#[test]
fn the_start_events_are_stable_public_and_the_fixture_has_its_own_version() {
    let mut game = start(&starter(), 3);
    let first = game.start_events_json().unwrap();
    assert_eq!(game.start_events_json().unwrap(), first, "two calls");
    for step in 0..6 {
        let actions = legal(&game);
        apply(&mut game, &play_first(&actions));
        assert_eq!(
            game.start_events_json().unwrap(),
            first,
            "after step {step}"
        );
    }

    // Only public information: one event with three keys, no card id, no hand.
    let events: Vec<Value> = serde_json::from_str(&first).unwrap();
    assert_eq!(events.len(), 1);
    let keys: Vec<&String> = events[0]["game_started"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(keys, ["budget", "data_versions", "first"]);
    assert_eq!(kinds(&events), ["game_started"]);
    for hidden in ["card", "hand", "drew", "played"] {
        assert!(!first.contains(hidden), "{hidden}");
    }

    // The fixture keeps its own record.
    let game = start(&fixture(), 1);
    let events: Vec<Value> = serde_json::from_str(&game.start_events_json().unwrap()).unwrap();
    assert_eq!(
        events[0]["game_started"]["data_versions"],
        json!(["fixture@1.0.0"])
    );
}

// ---------------------------------------------------------------------------------------
// Case 33: the shape of data_versions
// ---------------------------------------------------------------------------------------

#[test]
fn an_empty_list_of_versions_is_omitted_and_several_entries_come_back_as_they_are() {
    for (name, setup, first) in [
        ("starter", starter(), "One"),
        ("swapped", starter_swapped(), "Two"),
    ] {
        let mut setup = setup;
        setup["data_versions"] = json!([]);
        let game = start(&setup, 1);
        // String comparison: the key is absent, not `[]` and not `null`.
        assert_eq!(
            game.start_events_json().unwrap(),
            started(first, ""),
            "{name}"
        );
        let events: Vec<Value> = serde_json::from_str(&game.start_events_json().unwrap()).unwrap();
        assert!(events[0]["game_started"].get("data_versions").is_none());

        // The entries are opaque: duplicates, an empty string, non-ASCII text, all in order.
        let versions = json!([
            STARTER_VERSION,
            STARTER_VERSION,
            "",
            "é",
            "日本語 \u{1F3B2}",
            " x "
        ]);
        setup["data_versions"] = versions.clone();
        let game = start(&setup, 1);
        let events: Vec<Value> = serde_json::from_str(&game.start_events_json().unwrap()).unwrap();
        assert_eq!(
            events[0]["game_started"]["data_versions"], versions,
            "{name}"
        );
        assert_eq!(
            game.start_events_json().unwrap(),
            started(first, &format!(r#","data_versions":{versions}"#)),
            "{name}"
        );
    }
}

#[test]
fn hostile_shapes_of_data_versions_are_refused_one_by_one_and_the_key_is_required() {
    let hostile: Vec<(&str, Value)> = vec![
        ("a string", json!("starter@1.0.0")),
        ("a number", json!(1)),
        ("an object", json!({"v": "starter@1.0.0"})),
        ("null", Value::Null),
        ("a boolean", json!(true)),
        ("an entry that is a number", json!([1])),
        ("an entry that is null", json!([null])),
        ("an entry that is an object", json!([{}])),
        ("an entry that is a list", json!([["x"]])),
        ("a valid entry then a number", json!(["ok", 2])),
    ];
    for (name, versions) in hostile {
        let mut setup = starter();
        setup["data_versions"] = versions;
        // One at a time: each shape gets its own verdict.
        let verdict = Duel::from_json(&setup.to_string(), 1).err();
        assert!(verdict.is_some(), "{name} was accepted");
        assert_eq!(error_code(&verdict.unwrap()), "invalid_json", "{name}");
    }

    // The key is mandatory.
    let mut setup = starter();
    setup.as_object_mut().unwrap().remove("data_versions");
    assert_eq!(setup_error(&setup, 1), "invalid_json", "the key is absent");

    // Control: the same setup with a valid list starts.
    setup["data_versions"] = json!([STARTER_VERSION]);
    start(&setup, 1);
}

#[test]
fn a_setup_exhausted_at_the_start_reports_game_started_then_game_won() {
    let won =
        |winner: &str| format!(r#"{{"game_won":{{"winner":"{winner}","reason":"exhaustion"}}}}"#);
    // Rush starts with nothing to do (no effect, empty hand): Bulwark wins at once.
    let mut setup = starter();
    strip(&mut setup, "one");
    let mut game = start(&setup, 1);
    let first = format!(
        r#"[{{"game_started":{{"first":"One","budget":3,"data_versions":["starter@1.0.0"]}}}},{}]"#,
        won("Two")
    );
    assert_eq!(game.start_events_json().unwrap(), first);
    let events: Vec<Value> = serde_json::from_str(&first).unwrap();
    assert_eq!(kinds(&events), ["game_started", "game_won"]);
    every_action_is_refused_as_game_over(&mut game);
    assert_eq!(
        game.start_events_json().unwrap(),
        first,
        "unchanged by refusals"
    );

    // Rush in the second place starts all the same, and loses the same way.
    let mut setup = starter_swapped();
    strip(&mut setup, "two");
    let game = start(&setup, 1);
    let first = format!(
        r#"[{{"game_started":{{"first":"Two","budget":3,"data_versions":["starter@1.0.0"]}}}},{}]"#,
        won("One")
    );
    assert_eq!(game.start_events_json().unwrap(), first);
    assert_eq!(game.legal_actions_json().unwrap(), "[]");

    // Without versions the first event loses the key, the second is unchanged.
    let mut setup = starter();
    strip(&mut setup, "one");
    setup["data_versions"] = json!([]);
    let game = start(&setup, 1);
    assert_eq!(
        game.start_events_json().unwrap(),
        format!(
            r#"[{{"game_started":{{"first":"One","budget":3}}}},{}]"#,
            won("Two")
        )
    );
}

// ---------------------------------------------------------------------------------------
// Case 34: the heroes play differently
// ---------------------------------------------------------------------------------------

#[test]
fn the_two_heroes_open_differently_with_their_own_hands_effects_and_handovers() {
    for (name, setup, rush_base) in [
        ("rush first as One", starter(), 0u64),
        ("rush first as Two", starter_swapped(), 30),
    ] {
        for seed in 0..10 {
            let what = format!("{name}, seed {seed}");
            let mut game = start(&setup, seed);
            // Rush: 5 playable cards, two effects.
            let offered = legal(&game);
            let hand = plays(&offered);
            assert_eq!(hand.len(), 5, "{what}");
            assert!(hand.iter().all(|c| (rush_base..rush_base + 30).contains(c)));
            let effects: Vec<&Value> = offered.iter().filter(|a| kind(a) == "activate").collect();
            assert_eq!(effects, [&activate(0), &activate(1)], "{what}");

            // "Renforts" (cost 4 > budget 3): three cards, then Bulwark gets min(4 - 3 + 1, 10).
            let rush = if rush_base == 0 { "One" } else { "Two" };
            let bulwark = if rush_base == 0 { "Two" } else { "One" };
            let events = apply(&mut game, &activate(1));
            assert_eq!(
                kinds(&events),
                [
                    "effect_activated",
                    "drew",
                    "drew",
                    "drew",
                    "token_moved",
                    "active_player_changed"
                ],
                "{what}"
            );
            assert_eq!(
                events[0],
                json!({"effect_activated": {"player": rush, "effect": 1, "cost": 4}}),
                "{what}"
            );
            assert_eq!(count(&events, "drew"), 3);
            assert!(events[1..4].iter().all(|e| e["drew"]["player"] == rush));
            assert_eq!(token(&events), (bulwark.to_owned(), 2), "{what}");

            // Bulwark: 4 playable cards, two effects; "Garnison" (cost 3 > budget 2) hands the
            // token back with min(3 - 2 + 1, 6) = 2.
            let offered = legal(&game);
            assert_eq!(plays(&offered).len(), 4, "{what}");
            assert_eq!(active_effects(&game), 2, "{what}");
            assert!(
                plays(&offered)
                    .iter()
                    .all(|c| !(rush_base..rush_base + 30).contains(c))
            );
            let events = apply(&mut game, &activate(0));
            assert_eq!(
                kinds(&events),
                [
                    "effect_activated",
                    "drew",
                    "token_moved",
                    "active_player_changed"
                ],
                "{what}"
            );
            assert_eq!(
                events[0],
                json!({"effect_activated": {"player": bulwark, "effect": 0, "cost": 3}}),
                "{what}"
            );
            assert_eq!(events[1]["drew"]["player"], bulwark);
            assert_eq!(token(&events), (rush.to_owned(), 2), "{what}");
        }
    }
}

#[test]
fn the_heroes_time_caps_and_start_budgets_show_in_the_handovers() {
    // Every card costs 100 (the real heroes are kept), so a handover always hits the cap of the
    // receiver: the `max_time` of its hero.
    let mut setup = starter();
    uniform_cards(&mut setup, 100, 1);
    let mut game = start(&setup, 4);
    let card = plays(&legal(&game))[0];
    let events = apply(&mut game, &play(card));
    assert_eq!(token(&events), ("Two".to_owned(), 10), "Bulwark's cap");
    let card = plays(&legal(&game))[0];
    let events = apply(&mut game, &play(card));
    assert_eq!(token(&events), ("One".to_owned(), 6), "Rush's cap");

    // Bulwark, when it starts (its initiative raised above Rush's), starts with its own budget.
    let mut setup = starter_swapped();
    setup["one"]["hero"]["initiative"] = 9.into();
    let game = start(&setup, 4);
    assert_eq!(
        game.start_events_json().unwrap(),
        started("One", &starter_versions()).replace("\"budget\":3", "\"budget\":5")
    );
    assert_eq!(plays(&legal(&game)).len(), 4);
}

// ---------------------------------------------------------------------------------------
// Case 35: the profiles, as far as the JSON shows them
// ---------------------------------------------------------------------------------------

/// What a deck showed when all of it was played, then each creature attacked the enemy hero.
struct Profile {
    costs: Vec<u64>,
    attack_costs: Vec<u64>,
    forces: Vec<u64>,
}

/// Plays the 30 cards of `deck` (all in hand, a budget that never runs out, no opposing
/// creature so no provocation) and attacks the opposing hero with each, reading the costs and
/// the damage from the events. The foe has no hand, no effect and a health no attack empties.
fn observe(deck: &str, foe: &str) -> Profile {
    // No board limit: the 30 cards of the deck are all summoned (the profile is about the data).
    let mut setup = legacy(starter_of(deck, foe));
    for (side, hand, initiative) in [("one", 30, 9), ("two", 0, 1)] {
        let hero = &mut setup[side]["hero"];
        hero["effects"] = json!([]);
        hero["starting_hand"] = hand.into();
        hero["max_time"] = 1000.into();
        hero["start_budget"] = 1000.into();
        hero["initiative"] = initiative.into();
    }
    setup["two"]["hero"]["health"] = 5000.into();
    let mut game = start(&setup, 3);

    let mut hand = plays(&legal(&game));
    hand.sort_unstable();
    assert_eq!(
        hand,
        (0..30).collect::<Vec<u64>>(),
        "{deck}: the whole deck"
    );
    let mut costs = Vec::new();
    for id in 0..30 {
        let events = apply(&mut game, &play(id));
        let played = find(&events, "played").unwrap();
        assert_eq!(played["card"], id);
        costs.push(played["cost"].as_u64().unwrap());
    }
    let (mut attack_costs, mut forces) = (Vec::new(), Vec::new());
    for id in 0..30 {
        assert_eq!(
            targets_of(&legal(&game), id),
            [json!("hero")],
            "{deck} {id}"
        );
        let events = apply(&mut game, &strike_hero(id));
        assert_eq!(
            count(&events, "game_won"),
            0,
            "{deck}: the hero must survive"
        );
        attack_costs.push(find(&events, "attacked").unwrap()["cost"].as_u64().unwrap());
        let damage = find(&events, "damaged").unwrap();
        assert_eq!(damage["player"], "Two");
        forces.push(damage["amount"].as_u64().unwrap());
    }
    Profile {
        costs,
        attack_costs,
        forces,
    }
}

/// The same three series, read from `decks.json` and `cards.json`.
fn expected_profile(deck: &str) -> Profile {
    let stat = |name: &str| -> Vec<u64> {
        deck_cards(deck)
            .iter()
            .map(|c| card_def(c)[name].as_u64().unwrap())
            .collect()
    };
    Profile {
        costs: stat("cost"),
        attack_costs: stat("attack_cost"),
        forces: stat("force"),
    }
}

fn mean(values: &[u64]) -> f64 {
    values.iter().sum::<u64>() as f64 / values.len() as f64
}

fn sorted(values: &[u64]) -> Vec<u64> {
    let mut v = values.to_vec();
    v.sort_unstable();
    v
}

#[test]
fn the_observed_costs_attack_costs_and_forces_are_the_data_and_the_decks_differ() {
    let rush = observe("rush", "bulwark");
    let bulwark = observe("bulwark", "rush");
    for (name, seen) in [("rush", &rush), ("bulwark", &bulwark)] {
        let wanted = expected_profile(name);
        // Per card (the id is the position) and as multisets.
        assert_eq!(seen.costs, wanted.costs, "{name} costs");
        assert_eq!(
            seen.attack_costs, wanted.attack_costs,
            "{name} attack costs"
        );
        assert_eq!(seen.forces, wanted.forces, "{name} forces");
        assert_eq!(sorted(&seen.costs), sorted(&wanted.costs));
        assert_eq!(sorted(&seen.attack_costs), sorted(&wanted.attack_costs));
        assert_eq!(sorted(&seen.forces), sorted(&wanted.forces));
    }
    // Rush is cheap, hits hard and attacks fast; Bulwark is the opposite (the thresholds of the
    // data tests).
    assert!(mean(&bulwark.costs) - mean(&rush.costs) >= 1.5);
    assert!(mean(&rush.forces) - mean(&bulwark.forces) >= 0.5);
    assert!(mean(&bulwark.attack_costs) - mean(&rush.attack_costs) >= 1.0);
}

// ---------------------------------------------------------------------------------------
// Case 36: Bulwark's provocation in play
// ---------------------------------------------------------------------------------------

const FORBIDDEN: &str = "target_forbidden_by_provocation";

#[test]
fn bulwark_wardens_shield_what_stands_beside_them_and_give_it_back_when_they_fall() {
    let setup = alternating_starter("rush", "bulwark");
    let raiders = ids_of("rush", 0, "raider");
    let wardens = ids_of("bulwark", 30, "warden");
    let pikemen = ids_of("bulwark", 30, "pikeman");
    assert!(wardens.iter().all(|w| is_provocateur(&setup, *w)));
    assert!(pikemen.iter().all(|p| !is_provocateur(&setup, *p)));
    assert_eq!(card_def("warden")["health"], 6);
    let (r1, r2, w, x) = (raiders[0], raiders[1], wardens[0], pikemen[0]);

    for seed in 0..4 {
        let mut t = Table::new(&setup, seed);
        for card in [r1, w, r2, x] {
            t.act(&play(card));
            check_offer(&t);
        }
        assert_eq!((t.active, &t.boards), (0, &[vec![r1, r2], vec![w, x]]));
        let offered = legal(&t.game);
        // Only the warden, for each attacker: neither the hero nor the other creature.
        assert_eq!(
            attacks(&offered),
            [(r1, creature(w)), (r2, creature(w))],
            "seed {seed}"
        );

        // Refusals, on a twin: the state stays the same.
        let (mut a, mut b) = (t.fork(), t.fork());
        for attacker in [r1, r2] {
            assert_eq!(refusal(&mut a.game, &strike(attacker, x)), FORBIDDEN);
            assert_eq!(refusal(&mut a.game, &strike_hero(attacker)), FORBIDDEN);
            // Not on the opposing board: the same answer whatever the id.
            for gone in [59, 9999, r2 + attacker] {
                assert_eq!(
                    refusal(&mut a.game, &strike(attacker, gone)),
                    "target_not_on_board",
                    "seed {seed}, id {gone}"
                );
            }
        }
        assert_twins_agree(&mut a.game, &mut b.game, 6);

        // The warden takes 3 then 3 (6 health): two attacks, Bulwark acting in between.
        let events = t.act(&strike(r1, w));
        assert_eq!(
            kinds(&events),
            [
                "attacked",
                "creature_damaged",
                "token_moved",
                "active_player_changed"
            ]
        );
        assert_eq!(
            events[1],
            json!({"creature_damaged": {"player": "Two", "creature": w, "amount": 3}})
        );
        // Bulwark attacks freely while Rush holds no provocateur: the hero, then each creature.
        check_offer(&t);
        for attacker in [w, x] {
            assert_eq!(
                targets_of(&legal(&t.game), attacker),
                [json!("hero"), creature(r1), creature(r2)],
                "seed {seed}"
            );
        }
        t.act(&play(pikemen[1]));
        check_offer(&t);
        assert_eq!(
            attacks(&legal(&t.game)),
            [(r1, creature(w)), (r2, creature(w))],
            "the wounded warden still shields"
        );
        let events = t.act(&strike(r2, w));
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
            find(&events, "died"),
            Some(&json!({"player": "Two", "creature": w}))
        );
        check_offer(&t);
        t.act(&play(pikemen[2]));

        // At once: the hero and the other creatures are offered again, in board order.
        let offered = legal(&t.game);
        for attacker in [r1, r2] {
            assert_eq!(
                targets_of(&offered, attacker),
                [
                    json!("hero"),
                    creature(x),
                    creature(pikemen[1]),
                    creature(pikemen[2])
                ],
                "seed {seed}"
            );
        }
        check_offer(&t);
        let events = t.act(&strike_hero(r1));
        assert_eq!(kinds(&events)[..2], ["attacked", "damaged"]);
    }
}

#[test]
fn several_wardens_are_offered_in_board_order_around_a_plain_creature() {
    let setup = alternating_starter("rush", "bulwark");
    let raiders = ids_of("rush", 0, "raider");
    let wardens = ids_of("bulwark", 30, "warden");
    let x = ids_of("bulwark", 30, "pikeman")[0];
    let mut t = Table::new(&setup, 1);
    for card in [
        raiders[0], wardens[0], raiders[1], x, raiders[2], wardens[1],
    ] {
        t.act(&play(card));
    }
    assert_eq!(t.boards[1], [wardens[0], x, wardens[1]]);
    check_offer(&t);
    for attacker in &t.boards[0] {
        assert_eq!(
            targets_of(&legal(&t.game), *attacker),
            [creature(wardens[0]), creature(wardens[1])]
        );
    }
}

// ---------------------------------------------------------------------------------------
// Case 37: Rush's riposte in play
// ---------------------------------------------------------------------------------------

#[test]
fn a_spiked_wolf_ripostes_each_attacker_until_one_dies_of_it() {
    let setup = alternating_starter("rush", "bulwark");
    let wolves = ids_of("rush", 0, "spiked_wolf");
    let pike = ids_of("bulwark", 30, "pikeman")[0];
    assert_eq!(wolves.len(), 6);
    for wolf in &wolves {
        assert_eq!(card_of(&setup, *wolf)["riposte"], 1);
    }
    // The data: no Bulwark card and no other Rush card ripostes.
    for id in (0..60).filter(|id| !wolves.contains(id)) {
        assert_eq!(card_of(&setup, id)["riposte"], 0, "id {id}");
    }
    assert_eq!(card_def("pikeman")["health"], 5);

    let mut t = Table::new(&setup, 2);
    t.act(&play(wolves[0]));
    t.act(&play(pike));
    // Pikeman (force 2, attack cost 2) against a wolf (1 health): the wolf dies, but only after
    // its riposte of 1, owed to the owner of the attacker. The pikeman has 5 health.
    for k in 0..5 {
        t.act(&play(wolves[k + 1]));
        check_offer(&t);
        assert_eq!(t.active, 1);
        let events = t.act(&strike(pike, wolves[k]));
        let mut wanted = vec!["attacked", "creature_damaged", "riposted", "died"];
        if k == 4 {
            wanted.push("died");
        }
        wanted.extend(["token_moved", "active_player_changed"]);
        assert_eq!(kinds(&events), wanted, "hit {k}");
        assert_eq!(
            events[0],
            json!({"attacked": {"player": "Two", "attacker": pike, "cost": 2}})
        );
        assert_eq!(
            events[1],
            json!({"creature_damaged": {"player": "One", "creature": wolves[k], "amount": 2}})
        );
        assert_eq!(
            events[2],
            json!({"riposted": {"player": "Two", "creature": pike, "amount": 1}}),
            "hit {k}"
        );
        assert_eq!(
            events[3],
            json!({"died": {"player": "One", "creature": wolves[k]}})
        );
        if k == 4 {
            // The fifth riposte kills the pikeman: both died, in id order (wolf, then pikeman).
            assert_eq!(
                events[4],
                json!({"died": {"player": "Two", "creature": pike}})
            );
        }
    }
    assert!(t.boards[1].is_empty(), "the pikeman is dead");
    assert_eq!(t.boards[0], [wolves[5]]);
    assert_eq!(t.discards, [wolves[..5].to_vec(), vec![pike]]);
    check_offer(&t);
}

#[test]
fn an_attack_on_the_hero_or_on_a_bulwark_creature_is_never_riposted() {
    // The options off: neither hero hits back.
    let setup = legacy(alternating_starter("rush", "bulwark"));
    let wolf = ids_of("rush", 0, "spiked_wolf")[0];
    let pike = ids_of("bulwark", 30, "pikeman")[0];
    let mut t = Table::new(&setup, 5);
    t.act(&play(wolf));
    t.act(&play(pike));
    // The wolf (force 3) hits a Bulwark creature, whose riposte is 0: no `riposted`.
    let events = t.act(&strike(wolf, pike));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "creature_damaged",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(
        events[1],
        json!({"creature_damaged": {"player": "Two", "creature": pike, "amount": 3}})
    );
    // The pikeman hits the Rush hero, while a wolf with a riposte stands on the board: nothing.
    let events = t.act(&strike_hero(pike));
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "damaged",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!(
        events[1],
        json!({"damaged": {"player": "One", "amount": 2}})
    );

    // Every Bulwark creature, hit by anything of Rush, answers nothing (all have riposte 0).
    for victim in ids_of("bulwark", 30, "colossus") {
        let mut t = Table::new(&setup, 6);
        t.act(&play(wolf));
        t.act(&play(victim));
        let events = t.act(&strike(wolf, victim));
        assert_eq!(count(&events, "riposted"), 0, "colossus {victim}");
    }
}

// ---------------------------------------------------------------------------------------
// Case 38: a scripted game to the victory
// ---------------------------------------------------------------------------------------

/// The scripted policy: the first play, else the first attack, else the first activation.
/// Never concedes (it is always the last legal action).
fn play_first(actions: &[Value]) -> Value {
    ["play", "attack", "activate"]
        .iter()
        .find_map(|k| actions.iter().find(|a| kind(a) == *k))
        .expect("an unfinished game always has a non-concede action")
        .clone()
}

/// A fresh duel brought to where a journal ended, by replaying its actions.
fn replayed(setup: &Value, seed: u32, journal: &[Step]) -> Duel {
    let mut game = start(setup, seed);
    for (_, action, events) in journal {
        assert_eq!(&apply(&mut game, action), events);
    }
    game
}

#[test]
fn a_scripted_game_ends_by_a_lethal_attack_in_both_orders_of_the_decks() {
    for (name, setup) in [
        ("rush first", starter()),
        ("bulwark first", starter_swapped()),
    ] {
        for seed in 0..12 {
            let what = format!("{name}, seed {seed}");
            let journal = run_policy(&setup, seed, play_first);
            assert_eq!(
                journal,
                run_policy(&setup, seed, play_first),
                "{what}: not deterministic"
            );
            let seen = assert_a_provocation_duel_is_sound(&what, &setup, &journal);
            assert_eq!((seen.hero_down, seen.exhaustion), (1, 0), "{what}");

            // The last event is the victory by `hero_down`, with no handover after the lethal
            // attack.
            let (_, action, last) = journal.last().unwrap();
            assert_eq!(kind(action), "attack", "{what}");
            assert_eq!(action["attack"]["target"], json!("hero"), "{what}");
            assert_eq!(kinds(last), ["attacked", "damaged", "game_won"], "{what}");
            let won = &last.last().unwrap()["game_won"];
            assert_eq!(won["reason"], "hero_down", "{what}");
            assert_eq!(won["winner"], last[0]["attacked"]["player"], "{what}");
            for (_, _, events) in &journal {
                let names = kinds(events);
                if names.contains(&"game_won".to_owned()) {
                    assert_eq!(names.last().unwrap(), "game_won");
                    assert!(!names.contains(&"token_moved".to_owned()));
                    assert!(!names.contains(&"active_player_changed".to_owned()));
                }
            }

            // Afterwards nothing is legal and everything is refused as `game_over`.
            let mut game = replayed(&setup, seed, &journal);
            every_action_is_refused_as_game_over(&mut game);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Case 39: whole random duels on the starter set
// ---------------------------------------------------------------------------------------

#[test]
fn pseudo_random_duels_on_the_starter_set_respect_every_rule_and_see_ripostes_and_deaths() {
    let mut total = Observed::default();
    let mut duels = 0;
    for (name, setup) in [
        ("rush first", starter()),
        ("bulwark first", starter_swapped()),
    ] {
        for seed in 0..25 {
            let what = format!("{name}, seed {seed}");
            let journal = run_policy(&setup, seed, pseudo_random(seed));
            assert_eq!(
                journal,
                run_policy(&setup, seed, pseudo_random(seed)),
                "{what}: not deterministic"
            );
            // No concession, a victory by hero_down or exhaustion, no dead creature offered
            // again, every attack offered consistent with provocation, riposte as the data says.
            let seen = assert_a_provocation_duel_is_sound(&what, &setup, &journal);
            total.ripostes += seen.ripostes;
            total.deaths += seen.deaths;
            total.shielded += seen.shielded;
            total.hero_down += seen.hero_down;
            total.exhaustion += seen.exhaustion;
            duels += 1;
        }
    }
    assert_eq!(duels, 50);
    assert_eq!(total.hero_down + total.exhaustion, duels);
    assert!(total.ripostes > 0, "no riposte ever happened");
    assert!(total.deaths > 0, "no creature ever died");
    assert!(total.shielded > 0, "no warden ever shielded a hero");
}

// ---------------------------------------------------------------------------------------
// Case 40: S1, S2a and S2b are unchanged
// ---------------------------------------------------------------------------------------

#[test]
fn the_fixture_is_untouched_by_the_starter_set() {
    // Cases 1 to 31 above run as they were. The fixture still has neither provocation nor
    // riposte, keeps its own version, and the starter set is a different thing.
    let setup = fixture();
    for id in 0..60 {
        let card = card_of(&setup, id);
        assert_eq!(card["riposte"], 0, "id {id}");
        assert_eq!(card["provocation"], false, "id {id}");
    }
    assert_eq!(setup["data_versions"], json!(["fixture@1.0.0"]));
    assert_ne!(setup["one"], starter()["one"]);
    assert_ne!(setup["two"], starter()["two"]);
}
