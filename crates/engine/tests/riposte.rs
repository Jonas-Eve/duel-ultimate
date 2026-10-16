//! S2b-T3: simultaneous riposte, through the public API only. Cases needing private state
//! (exhaustion branches, ids out of setup order, the pure plan) live in the crate.
//!
//! Card ids are assigned in setup order, One's deck first (0..30) then Two's (30..60). Every
//! card starts in its owner's hand (`starting_hand` 30), so a test plays any card by id.

use duel_cards::{CreatureStats, Hero, MAX_STAT};
use duel_engine::{
    Action, CardInstanceId, Event, GameSetup, GameState, PlayerId, PlayerSetup, RuleError, Target,
    WinReason, apply, new_game,
};

/// Summoning cost 1.
fn unit(force: u32, health: u32, riposte: u32, attack_cost: u32) -> CreatureStats {
    CreatureStats {
        cost: 1,
        force,
        attack_cost,
        health,
        provocation: false,
        riposte,
    }
}

fn id(n: u32) -> CardInstanceId {
    CardInstanceId(n)
}

fn hero(initiative: u32, start_budget: u32, max_time: u32) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time,
        starting_hand: 30,
        initiative,
        start_budget,
        health: 10,
        effects: vec![],
    }
}

/// One summons `one` (ids 0..), Two summons `two` (ids 30..), then One is active with `k`
/// time units. The other cards are plain creatures left in hand, so nobody is exhausted.
/// Two's `max_time` is `two_max` (at least `two.len()`).
fn arena(one: &[CreatureStats], two: &[CreatureStats], k: u32, two_max: u32) -> GameState {
    let (n, m) = (one.len() as u32, two.len() as u32);
    assert!(n >= 1 && m >= 1 && n < 30 && m < 30 && m <= two_max);
    let mut one_cards = one.to_vec();
    one_cards[(n - 1) as usize].cost = m; // Two then receives m
    one_cards.resize(30, unit(1, 3, 0, 1));
    let mut two_cards = two.to_vec();
    two_cards[(m - 1) as usize].cost = k; // One then receives k
    two_cards.resize(30, unit(1, 3, 0, 1));
    let (mut state, _) = new_game(
        GameSetup {
            one: PlayerSetup {
                hero: hero(2, n, 30),
                cards: one_cards,
                card_names: Vec::new(),
            },
            two: PlayerSetup {
                hero: hero(1, 1, two_max),
                cards: two_cards,
                card_names: Vec::new(),
            },
            data_versions: vec!["fixture@1.0.0".into()],
        },
        7,
    )
    .unwrap();
    for n in 0..n {
        apply(&mut state, Action::Play { card: id(n) }).unwrap();
    }
    for n in 0..m {
        apply(&mut state, Action::Play { card: id(30 + n) }).unwrap();
    }
    assert_eq!((state.active(), state.budget()), (PlayerId::One, k));
    state
}

/// One creature (id 0) facing one creature (id 30), One active with `k` time units.
fn duel(a: CreatureStats, x: CreatureStats, k: u32) -> GameState {
    arena(&[a], &[x], k, 30)
}

fn strike(attacker: CardInstanceId, victim: CardInstanceId) -> Action {
    Action::Attack {
        attacker,
        target: Target::Creature(victim),
    }
}

fn hit(attacker: CardInstanceId) -> Action {
    Action::Attack {
        attacker,
        target: Target::Hero,
    }
}

fn attacked(player: PlayerId, attacker: u32, cost: u32) -> Event {
    Event::Attacked {
        player,
        attacker: id(attacker),
        cost,
    }
}

fn creature_damaged(player: PlayerId, creature: u32, amount: u32) -> Event {
    Event::CreatureDamaged {
        player,
        creature: id(creature),
        amount,
    }
}

fn riposted(player: PlayerId, creature: u32, amount: u32) -> Event {
    Event::Riposted {
        player,
        creature: id(creature),
        amount,
    }
}

fn died(player: PlayerId, creature: u32) -> Event {
    Event::Died {
        player,
        creature: id(creature),
    }
}

fn token(player: PlayerId, budget: u32) -> Event {
    Event::TokenMoved { player, budget }
}

fn handed_to(player: PlayerId) -> Event {
    Event::ActivePlayerChanged { player }
}

// ---- the journal ---------------------------------------------------------------------------

#[test]
fn a_surviving_target_ripostes_after_its_damage_and_the_token_stays() {
    // Force 2 against 5 health and riposte 3; the attack costs 1 of 3.
    let mut state = duel(unit(2, 5, 0, 1), unit(1, 5, 3, 1), 3);
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::One, 0, 1),
            creature_damaged(PlayerId::Two, 30, 2),
            riposted(PlayerId::One, 0, 3),
            token(PlayerId::One, 2),
        ]
    );
    assert_eq!(state.creature_health(id(30)), Some(3));
    assert_eq!(state.creature_health(id(0)), Some(2));
}

#[test]
fn a_surviving_target_ripostes_and_the_token_may_cross() {
    // The attack costs the whole budget (3 of 3): Two receives 1.
    let mut state = duel(unit(2, 5, 0, 3), unit(1, 5, 3, 1), 3);
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::One, 0, 3),
            creature_damaged(PlayerId::Two, 30, 2),
            riposted(PlayerId::One, 0, 3),
            token(PlayerId::Two, 1),
            handed_to(PlayerId::Two),
        ]
    );
}

#[test]
fn a_target_killed_by_the_attack_ripostes_all_the_same() {
    // Force 3 kills the 3-health target, which still hits back for 2.
    let mut state = duel(unit(3, 5, 0, 1), unit(1, 3, 2, 1), 3);
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::One, 0, 1),
            creature_damaged(PlayerId::Two, 30, 3),
            riposted(PlayerId::One, 0, 2),
            died(PlayerId::Two, 30),
            token(PlayerId::One, 2),
        ]
    );
    assert_eq!(state.creature_health(id(0)), Some(3), "5 - 2");
    assert_eq!(state.discard(PlayerId::Two), vec![id(30)]);
    assert!(state.discard(PlayerId::One).is_empty());
}

#[test]
fn a_riposte_that_kills_the_attacker_alone_buries_it_and_the_attack_is_still_paid() {
    // Riposte 4 against 2 health: the excess is lost, `amount` stays nominal.
    let mut state = duel(unit(1, 2, 0, 1), unit(1, 5, 4, 1), 3);
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::One, 0, 1),
            creature_damaged(PlayerId::Two, 30, 1),
            riposted(PlayerId::One, 0, 4),
            died(PlayerId::One, 0),
            token(PlayerId::One, 2),
        ]
    );
    assert_eq!(state.creature_health(id(30)), Some(4), "the target lives");
    assert_eq!(state.discard(PlayerId::One), vec![id(0)]);
    assert!(state.board(PlayerId::One).is_empty());
    assert_eq!((state.active(), state.budget()), (PlayerId::One, 2));
    // The dead attacker can no longer act.
    let before = state.clone();
    assert_eq!(
        apply(&mut state, strike(id(0), id(30))),
        Err(RuleError::AttackerNotOnBoard)
    );
    assert_eq!(state, before);
}

#[test]
fn simultaneous_deaths_are_logged_in_instance_id_order_and_both_discarded() {
    let mut state = duel(unit(3, 3, 0, 1), unit(1, 3, 3, 1), 3);
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::One, 0, 1),
            creature_damaged(PlayerId::Two, 30, 3),
            riposted(PlayerId::One, 0, 3),
            died(PlayerId::One, 0),
            died(PlayerId::Two, 30),
            token(PlayerId::One, 2),
        ]
    );
    assert_eq!(state.discard(PlayerId::One), vec![id(0)]);
    assert_eq!(state.discard(PlayerId::Two), vec![id(30)]);
    assert!(state.board(PlayerId::One).is_empty());
    assert!(state.board(PlayerId::Two).is_empty());
    // Neither can be used again: refused as attacker and as target.
    for action in [strike(id(0), id(30)), strike(id(30), id(0)), hit(id(0))] {
        let before = state.clone();
        assert_eq!(
            apply(&mut state, action),
            Err(RuleError::AttackerNotOnBoard)
        );
        assert_eq!(state, before);
    }
}

#[test]
fn simultaneous_deaths_with_a_handover_end_the_log_with_the_token_crossing() {
    let mut state = duel(unit(3, 3, 0, 3), unit(1, 3, 3, 1), 3);
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::One, 0, 3),
            creature_damaged(PlayerId::Two, 30, 3),
            riposted(PlayerId::One, 0, 3),
            died(PlayerId::One, 0),
            died(PlayerId::Two, 30),
            token(PlayerId::Two, 1),
            handed_to(PlayerId::Two),
        ]
    );
}

#[test]
fn the_active_player_two_is_riposted_too_and_dies_in_id_order() {
    // One hits the hero with its whole budget (attack cost 1 of 1) to give Two the token.
    let mut state = duel(unit(1, 3, 3, 1), unit(3, 3, 3, 1), 1);
    apply(&mut state, hit(id(0))).unwrap();
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 1));
    let events = apply(&mut state, strike(id(30), id(0))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::Two, 30, 1),
            creature_damaged(PlayerId::One, 0, 3),
            riposted(PlayerId::Two, 30, 3),
            died(PlayerId::One, 0),
            died(PlayerId::Two, 30),
            token(PlayerId::One, 1),
            handed_to(PlayerId::One),
        ]
    );
    assert_eq!(state.discard(PlayerId::One), vec![id(0)]);
    assert_eq!(state.discard(PlayerId::Two), vec![id(30)]);
}

// ---- degenerate values ---------------------------------------------------------------------

#[test]
fn a_riposte_of_zero_emits_nothing_and_costs_the_attacker_nothing() {
    let mut state = duel(unit(2, 5, 0, 1), unit(1, 5, 0, 1), 3);
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::One, 0, 1),
            creature_damaged(PlayerId::Two, 30, 2),
            token(PlayerId::One, 2),
        ]
    );
    assert_eq!(state.creature_health(id(0)), Some(5));
}

#[test]
fn a_riposte_far_above_the_attackers_health_saturates_without_panicking() {
    let mut state = duel(unit(1, 1, 0, 1), unit(1, MAX_STAT, MAX_STAT, 1), 3);
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert_eq!(events.get(2), Some(&riposted(PlayerId::One, 0, MAX_STAT)));
    assert_eq!(events.get(3), Some(&died(PlayerId::One, 0)));
    assert_eq!(state.creature_health(id(30)), Some(MAX_STAT - 1));
}

#[test]
fn an_attack_on_the_hero_never_provokes_a_riposte() {
    // Two's board is full of ripostants, none with provocation.
    let mut state = arena(
        &[unit(3, 5, 0, 1)],
        &[unit(1, 5, 4, 1), unit(1, 5, 4, 1)],
        3,
        30,
    );
    let events = apply(&mut state, hit(id(0))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::One, 0, 1),
            Event::Damaged {
                player: PlayerId::Two,
                amount: 3
            },
            token(PlayerId::One, 2),
        ]
    );
    assert_eq!(state.creature_health(id(0)), Some(5));
}

#[test]
fn a_lethal_attack_on_the_hero_ends_the_game_without_riposte_or_handover() {
    let mut state = arena(&[unit(10, 5, 0, 1)], &[unit(1, 5, 4, 1)], 3, 30);
    let events = apply(&mut state, hit(id(0))).unwrap();
    assert_eq!(
        events,
        vec![
            attacked(PlayerId::One, 0, 1),
            Event::Damaged {
                player: PlayerId::Two,
                amount: 10
            },
            Event::GameWon {
                winner: PlayerId::One,
                reason: WinReason::HeroDown
            },
        ]
    );
}

#[test]
fn a_provocateur_with_riposte_still_hides_the_hero_and_ripostes_when_hit() {
    let mut taunt = unit(1, 5, 2, 1);
    taunt.provocation = true;
    let mut state = duel(unit(1, 5, 0, 1), taunt, 3);
    let before = state.clone();
    assert_eq!(
        apply(&mut state, hit(id(0))),
        Err(RuleError::TargetForbiddenByProvocation)
    );
    assert_eq!(state, before);
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert!(events.contains(&riposted(PlayerId::One, 0, 2)));
}

// ---- time ----------------------------------------------------------------------------------

/// The attacker (1 health) dies of a riposte of 1; its attack costs `attack_cost` of `k`.
fn token_after_a_fatal_riposte(attack_cost: u32, k: u32, two_max: u32) -> Vec<Event> {
    let mut state = arena(
        &[unit(1, 1, 0, attack_cost)],
        &[unit(1, 5, 1, 1)],
        k,
        two_max,
    );
    let events = apply(&mut state, strike(id(0), id(30))).unwrap();
    assert!(events.contains(&died(PlayerId::One, 0)));
    assert_eq!(
        events.first(),
        Some(&attacked(PlayerId::One, 0, attack_cost))
    );
    events.into_iter().skip(4).collect()
}

#[test]
fn the_time_rule_is_unchanged_when_the_attacker_dies_below_the_budget() {
    assert_eq!(
        token_after_a_fatal_riposte(2, 5, 30),
        vec![token(PlayerId::One, 3)]
    );
}

#[test]
fn the_time_rule_is_unchanged_when_the_attacker_dies_at_exactly_the_budget() {
    assert_eq!(
        token_after_a_fatal_riposte(5, 5, 30),
        vec![token(PlayerId::Two, 1), handed_to(PlayerId::Two)]
    );
}

#[test]
fn the_time_rule_is_unchanged_when_the_attacker_dies_above_the_budget_and_the_receiver_is_capped() {
    // 9 - 5 + 1 = 5, capped at Two's max_time of 4.
    assert_eq!(
        token_after_a_fatal_riposte(9, 5, 4),
        vec![token(PlayerId::Two, 4), handed_to(PlayerId::Two)]
    );
}

// ---- JSON ----------------------------------------------------------------------------------

#[test]
fn riposted_travels_with_the_shape_of_creature_damaged_and_round_trips() {
    let riposted = Event::Riposted {
        player: PlayerId::One,
        creature: id(7),
        amount: 3,
    };
    let json = serde_json::to_string(&riposted).unwrap();
    assert_eq!(
        json,
        r#"{"riposted":{"player":"One","creature":7,"amount":3}}"#
    );
    let damaged = serde_json::to_value(Event::CreatureDamaged {
        player: PlayerId::One,
        creature: id(7),
        amount: 3,
    })
    .unwrap();
    let value = serde_json::to_value(&riposted).unwrap();
    let inner = |v: &serde_json::Value, key: &str| v.get(key).cloned().unwrap();
    let (ri, da) = (
        inner(&value, "riposted"),
        inner(&damaged, "creature_damaged"),
    );
    assert_eq!(ri, da, "same keys, same value types");
    assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), riposted);
}
