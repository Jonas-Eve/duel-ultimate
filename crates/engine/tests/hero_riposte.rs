//! #170: the "infinite" riposte of the hero (`RuleSet::hero_riposte`), through the public API.
//!
//! A hit on the opposing hero that does not kill it destroys the attacker whatever its health;
//! a lethal hit (`hero health <= force`, read before the attack) is `HeroDown` as ever, with no
//! riposte, no death and no handover. The rules are always written out: the engine default is
//! not what these tests are about.
//!
//! Card ids are assigned in setup order, One's deck first (0..30) then Two's (30..60). Every
//! card starts in its owner's hand (`starting_hand` 30), so a test plays any card by id. The
//! journals are compared as JSON, the form the clients read.

use duel_cards::{CreatureStats, Hero, MAX_STAT};
use duel_engine::{
    Action, CardInstanceId, Event, GameSetup, GameState, PlayerId, PlayerSetup, RuleSet, Target,
    WinReason, apply, legal_actions, new_game, player_view, preview_legal_actions,
};
use serde_json::{Value, json};

const ON: RuleSet = RuleSet {
    hero_riposte: true,
    ..RuleSet::legacy()
};

/// Summoning cost 1, no riposte.
fn unit(force: u32, health: u32, attack_cost: u32) -> CreatureStats {
    CreatureStats {
        cost: 1,
        force,
        attack_cost,
        health,
        provocation: false,
        riposte: 0,
    }
}

fn id(n: u32) -> CardInstanceId {
    CardInstanceId(n)
}

fn hero(initiative: u32, start_budget: u32, max_time: u32, health: u32) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time,
        starting_hand: 30,
        initiative,
        start_budget,
        health,
        effects: vec![],
    }
}

/// One summons `attacker` (id 0, then `n - 1` fillers), Two summons one creature (id 30), then
/// One is active with `k` time units against a hero of `two_health` health. Two's `max_time`
/// is `two_max`. The remaining cards stay in hand, so nobody is exhausted.
fn arena(
    attacker: CreatureStats,
    k: u32,
    two_health: u32,
    two_max: u32,
    rules: RuleSet,
) -> GameState {
    let mut one_cards = vec![attacker];
    one_cards[0].cost = 1; // Two then receives 1
    one_cards.resize(30, unit(1, 3, 1));
    let mut two_cards = vec![unit(1, 3, 1)];
    two_cards[0].cost = k; // One then receives k
    two_cards.resize(30, unit(1, 3, 1));
    let (mut state, _) = new_game(
        GameSetup {
            one: PlayerSetup {
                hero: hero(2, 1, 30, 10),
                cards: one_cards,
                card_names: Vec::new(),
            },
            two: PlayerSetup {
                hero: hero(1, 1, two_max, two_health),
                cards: two_cards,
                card_names: Vec::new(),
            },
            data_versions: vec!["fixture@1.0.0".into()],
            rules,
        },
        7,
    )
    .unwrap();
    apply(&mut state, Action::play(id(0))).unwrap();
    apply(&mut state, Action::play(id(30))).unwrap();
    assert_eq!((state.active(), state.budget()), (PlayerId::One, k));
    state
}

fn hit() -> Action {
    Action::Attack {
        attacker: id(0),
        target: Target::Hero,
    }
}

fn journal(events: &[Event]) -> Value {
    serde_json::to_value(events).unwrap()
}

fn kinds(events: &[Event]) -> Vec<String> {
    journal(events)
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_object().unwrap().keys().next().unwrap().clone())
        .collect()
}

#[test]
fn a_non_lethal_hit_on_the_hero_kills_the_attacker() {
    // Force 2, health 5, attack cost 1, budget 3: the hero (10) takes 2 and the attacker dies.
    let mut state = arena(unit(2, 5, 1), 3, 10, 30, ON);
    let events = apply(&mut state, hit()).unwrap();
    assert_eq!(
        journal(&events),
        json!([
            {"attacked": {"player": "One", "attacker": 0, "cost": 1}},
            {"damaged": {"player": "Two", "amount": 2}},
            {"hero_riposted": {"player": "One", "creature": 0}},
            {"died": {"player": "One", "creature": 0}},
            {"token_moved": {"player": "One", "budget": 2}}
        ])
    );
    assert_eq!(state.hero_health(PlayerId::Two), 8);
    assert_eq!(state.winner(), None);
}

#[test]
fn the_journal_of_a_hit_that_hands_over_keeps_the_order() {
    // Budget 1, attack cost 1: the token crosses, Two receives 1.
    let mut state = arena(unit(2, 5, 1), 1, 10, 30, ON);
    let events = apply(&mut state, hit()).unwrap();
    assert_eq!(
        kinds(&events),
        [
            "attacked",
            "damaged",
            "hero_riposted",
            "died",
            "token_moved",
            "active_player_changed"
        ]
    );
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 1));
}

#[test]
fn the_slain_attacker_is_in_its_owners_public_discard() {
    let mut state = arena(unit(2, 5, 1), 3, 10, 30, ON);
    apply(&mut state, hit()).unwrap();
    assert_eq!(state.discard(PlayerId::One), vec![id(0)]);
    assert!(!state.board(PlayerId::One).contains(&id(0)));
    assert_eq!(state.creature_health(id(0)), None);
    for seat in [PlayerId::One, PlayerId::Two] {
        let view = serde_json::to_value(player_view(&state, seat)).unwrap();
        let side = if seat == PlayerId::One {
            "you"
        } else {
            "opponent"
        };
        let discard = view[side]["discard"].as_array().unwrap();
        assert_eq!(discard.len(), 1, "seat {seat:?}");
        assert_eq!(discard[0]["id"], 0);
        assert_eq!(discard[0]["health"], 0);
    }
}

#[test]
fn a_lethal_hit_has_no_riposte() {
    // Hero health == force.
    let mut state = arena(unit(3, 5, 1), 3, 3, 30, ON);
    let before_budget = state.budget();
    let events = apply(&mut state, hit()).unwrap();
    assert_eq!(
        journal(&events),
        json!([
            {"attacked": {"player": "One", "attacker": 0, "cost": 1}},
            {"damaged": {"player": "Two", "amount": 3}},
            {"game_won": {"winner": "One", "reason": "hero_down"}}
        ])
    );
    assert_eq!(state.winner(), Some((PlayerId::One, WinReason::HeroDown)));
    assert_eq!(state.board(PlayerId::One), vec![id(0)]);
    assert!(state.discard(PlayerId::One).is_empty());
    assert_eq!(
        (state.active(), state.budget()),
        (PlayerId::One, before_budget)
    );
}

#[test]
fn excess_force_is_lethal_and_one_point_short_is_not() {
    // Force 4: health 3 (excess) is lethal, health 4 (== force) is lethal, health 5 is not.
    for (health, lethal) in [(3, true), (4, true), (5, false)] {
        let mut state = arena(unit(4, 5, 1), 3, health, 30, ON);
        let events = apply(&mut state, hit()).unwrap();
        assert_eq!(
            kinds(&events).contains(&"hero_riposted".to_string()),
            !lethal,
            "hero health {health}"
        );
        assert_eq!(state.winner().is_some(), lethal, "hero health {health}");
        assert_eq!(state.board(PlayerId::One).contains(&id(0)), lethal);
    }
}

#[test]
fn the_attacker_dies_whatever_its_health() {
    for health in [1, 10, MAX_STAT] {
        let mut state = arena(unit(1, health, 1), 3, 10, 30, ON);
        let events = apply(&mut state, hit()).unwrap();
        assert!(
            kinds(&events).contains(&"hero_riposted".to_string()),
            "{health}"
        );
        assert_eq!(state.discard(PlayerId::One), vec![id(0)], "health {health}");
    }
}

#[test]
fn the_attack_cost_is_paid_even_if_the_attacker_dies() {
    // Stays active: cost 2 < budget 5 -> 3 left.
    let mut state = arena(unit(1, 5, 2), 5, 10, 30, ON);
    let events = apply(&mut state, hit()).unwrap();
    assert_eq!(
        events.last(),
        Some(&Event::TokenMoved {
            player: PlayerId::One,
            budget: 3
        })
    );
    assert_eq!(state.active(), PlayerId::One);
    // Hands over: cost 4 >= budget 2 -> Two receives min(4 - 2 + 1, 30) = 3; and 2 when capped.
    for (two_max, received) in [(30, 3), (2, 2)] {
        let mut state = arena(unit(1, 5, 4), 2, 10, two_max, ON);
        let events = apply(&mut state, hit()).unwrap();
        assert_eq!(
            kinds(&events)[events.len() - 2..],
            ["token_moved", "active_player_changed"]
        );
        assert_eq!((state.active(), state.budget()), (PlayerId::Two, received));
    }
}

#[test]
fn an_attack_on_a_creature_is_unaffected_by_the_option() {
    // Whatever the shapes, the same attacks on a creature give the same journal and state
    // with the option on and off.
    let play = |rules: RuleSet, force: u32, health: u32| {
        let mut state = arena(unit(force, health, 1), 30, 10, 30, rules);
        let attack = Action::Attack {
            attacker: id(0),
            target: Target::Creature(id(30)),
        };
        let mut log = Vec::new();
        while let Ok(events) = apply(&mut state, attack.clone()) {
            log.extend(events);
            if state.winner().is_some() || state.active() != PlayerId::One {
                break;
            }
        }
        assert!(!log.is_empty());
        (
            journal(&log),
            state.board(PlayerId::One),
            state.discard(PlayerId::Two),
        )
    };
    for force in 1..=4 {
        for health in 1..=5 {
            assert_eq!(
                play(ON, force, health),
                play(RuleSet::legacy(), force, health),
                "force {force}, health {health}"
            );
        }
    }
}

#[test]
fn with_the_option_off_a_hit_on_the_hero_leaves_the_attacker_alive() {
    let mut state = arena(unit(2, 5, 1), 3, 10, 30, RuleSet::legacy());
    let events = apply(&mut state, hit()).unwrap();
    assert_eq!(kinds(&events), ["attacked", "damaged", "token_moved"]);
    assert_eq!(state.board(PlayerId::One), vec![id(0)]);
    assert_eq!(state.creature_health(id(0)), Some(5));
}

#[test]
fn the_riposte_lowers_phi_by_more_than_the_hit() {
    // Φ = heroes' health + per card current health (+2 pile, +1 hand, 0 board, nothing in a
    // discard). An attack moves no card between piles or hands, so the zones seen by One's
    // view (both heroes, One's hand and board, Two's board) carry the whole difference.
    let phi = |state: &GameState| -> u64 {
        let view = serde_json::to_value(player_view(state, PlayerId::One)).unwrap();
        let health = |cards: &Value, extra: u64| -> u64 {
            cards
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["health"].as_u64().unwrap() + extra)
                .sum()
        };
        view["you"]["hero"]["health"].as_u64().unwrap()
            + view["opponent"]["hero"]["health"].as_u64().unwrap()
            + health(&view["you"]["hand"], 1)
            + health(&view["you"]["board"], 0)
            + health(&view["opponent"]["board"], 0)
    };
    for (health, force) in [(5, 2), (9, 1), (3, 3)] {
        let mut state = arena(unit(force, health, 1), 3, 10, 30, ON);
        let before = phi(&state);
        apply(&mut state, hit()).unwrap();
        let after = phi(&state);
        assert!(
            before - after >= u64::from(force + health),
            "force {force}, health {health}: {before} -> {after}"
        );
    }
}

#[test]
fn legal_actions_still_list_the_hit_on_the_hero() {
    // The suicide is a legal action like any other: only its consequence changes.
    let state = arena(unit(2, 5, 1), 3, 10, 30, ON);
    assert!(legal_actions(&state).contains(&hit()));
    let preview = preview_legal_actions(&state)
        .into_iter()
        .find(|p| p.action == hit())
        .unwrap();
    assert_eq!(preview.cost, Some(1));
}
