//! Public-API tests of the engine: setup, the time rule through hero effects, concede,
//! `legal_actions`, the event log. Cases needing private state live next to `GameState`.

use duel_cards::{CardsError, CreatureStats, Effect, Hero, HeroEffect, MAX_STAT};
use duel_engine::{
    Action, Event, GameSetup, GameState, PlayerId, PlayerSetup, RuleError, SetupError, WinReason,
    apply, legal_actions, new_game,
};
use std::collections::BTreeSet;

fn draw(cost: u32, count: u32) -> HeroEffect {
    HeroEffect {
        name: "Piocher".into(),
        cost,
        effect: Effect::Draw { count },
    }
}

fn hero(initiative: u32, start_budget: u32, max_time: u32, effects: Vec<HeroEffect>) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time,
        starting_hand: 4,
        initiative,
        start_budget,
        health: 10,
        effects,
    }
}

fn cards() -> Vec<CreatureStats> {
    (1..=30)
        .map(|n| CreatureStats {
            cost: n,
            force: 1 + n % 3,
            attack_cost: 1 + n % 2,
            health: 1 + n % 4,
        })
        .collect()
}

fn setup(one: Hero, two: Hero) -> GameSetup {
    GameSetup {
        one: PlayerSetup {
            hero: one,
            cards: cards(),
        },
        two: PlayerSetup {
            hero: two,
            cards: cards(),
        },
        data_versions: vec!["fixture@1.0.0".into()],
    }
}

/// One starts with budget 3 and max_time 8; Two has max_time `two_max`.
fn duel(one_effects: Vec<HeroEffect>, two_max: u32) -> (GameState, Vec<Event>) {
    new_game(
        setup(
            hero(2, 3, 8, one_effects),
            hero(1, 1, two_max, vec![draw(1, 1)]),
        ),
        7,
    )
    .unwrap()
}

fn kinds(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
            Event::GameStarted { .. } => "GameStarted",
            Event::EffectActivated { .. } => "EffectActivated",
            Event::Drew { .. } => "Drew",
            Event::TokenMoved { .. } => "TokenMoved",
            Event::ActivePlayerChanged { .. } => "ActivePlayerChanged",
            Event::GameWon { .. } => "GameWon",
        })
        .collect()
}

/// Order in which One draws its whole pile (a fingerprint of the shuffle).
fn pile_order(seed: u64) -> Vec<u32> {
    let (mut state, _) = new_game(
        setup(
            hero(2, 8, 8, vec![draw(1, 30)]),
            hero(1, 1, 8, vec![draw(1, 1)]),
        ),
        seed,
    )
    .unwrap();
    apply(&mut state, Action::Activate { effect: 0 })
        .unwrap()
        .into_iter()
        .filter_map(|e| match e {
            Event::Drew { card, .. } => Some(card.0),
            _ => None,
        })
        .collect()
}

// ---- setup -------------------------------------------------------------------------

#[test]
fn same_seed_same_state_and_same_log() {
    let make = |seed| {
        new_game(
            setup(
                hero(1, 3, 8, vec![draw(2, 1)]),
                hero(1, 3, 8, vec![draw(2, 1)]),
            ),
            seed,
        )
        .unwrap()
    };
    assert_eq!(make(42), make(42));
}

#[test]
fn different_seeds_shuffle_differently() {
    assert_eq!(pile_order(1), pile_order(1));
    assert_ne!(pile_order(1), pile_order(2));
    let order = pile_order(1);
    assert_eq!(order.len(), 26, "30 cards minus the 4 of the opening hand");
    assert_eq!(
        order.iter().collect::<BTreeSet<_>>().len(),
        26,
        "no card twice"
    );
}

#[test]
fn opening_hands_cost_nothing_and_use_no_time() {
    let (state, _) = duel(vec![draw(2, 1)], 8);
    assert_eq!(state.hand_len(PlayerId::One), 4);
    assert_eq!(state.hand_len(PlayerId::Two), 4);
    assert_eq!(state.deck_len(PlayerId::One), 26);
    assert_eq!(state.deck_len(PlayerId::Two), 26);
    assert_eq!(state.budget(), 3);
}

#[test]
fn the_higher_initiative_starts_with_its_start_budget() {
    for seed in 0..20 {
        let (state, events) = new_game(
            setup(
                hero(1, 2, 8, vec![draw(1, 1)]),
                hero(5, 6, 9, vec![draw(1, 1)]),
            ),
            seed,
        )
        .unwrap();
        assert_eq!(state.active(), PlayerId::Two);
        assert_eq!(state.budget(), 6);
        assert_eq!(
            events.first(),
            Some(&Event::GameStarted {
                first: PlayerId::Two,
                budget: 6
            })
        );
        assert_eq!(state.winner(), None);
    }
}

#[test]
fn an_initiative_tie_is_broken_by_the_seed() {
    let first = |seed| {
        new_game(
            setup(
                hero(3, 2, 8, vec![draw(1, 1)]),
                hero(3, 5, 8, vec![draw(1, 1)]),
            ),
            seed,
        )
        .unwrap()
        .0
    };
    let firsts: BTreeSet<_> = (0..64)
        .map(|s| first(s).active() == PlayerId::One)
        .collect();
    assert_eq!(firsts.len(), 2, "both players start for some seeds");
    assert_eq!(
        first(5),
        first(5),
        "and a seed always gives the same answer"
    );
    // The starter's own start_budget applies.
    let s = first(0);
    assert_eq!(s.budget(), if s.active() == PlayerId::One { 2 } else { 5 });
}

#[test]
fn the_game_records_its_data_versions() {
    let (state, _) = duel(vec![draw(1, 1)], 8);
    assert_eq!(state.data_versions(), ["fixture@1.0.0".to_string()]);
}

#[test]
fn invalid_data_is_rejected_at_setup_without_panicking() {
    let bad = |f: &dyn Fn(&mut GameSetup)| {
        let mut s = setup(
            hero(2, 3, 8, vec![draw(1, 1)]),
            hero(1, 1, 8, vec![draw(1, 1)]),
        );
        f(&mut s);
        new_game(s, 1).unwrap_err()
    };
    let SetupError::Invalid(e) = bad(&|s| s.one.hero.start_budget = 9);
    assert_eq!(
        e,
        CardsError::StartBudgetOutOfRange {
            start_budget: 9,
            max_time: 8
        }
    );
    let SetupError::Invalid(e) = bad(&|s| s.two.hero.max_time = 0);
    assert_eq!(e, CardsError::MaxTimeBelowOne);
    let SetupError::Invalid(e) = bad(&|s| s.one.hero.effects = vec![draw(0, 1)]);
    assert_eq!(e, CardsError::EffectCostBelowOne { effect: 0 });
    let SetupError::Invalid(e) = bad(&|s| s.two.hero.effects = vec![draw(1, 0)]);
    assert_eq!(e, CardsError::EffectCountBelowOne { effect: 0 });
    let SetupError::Invalid(e) = bad(&|s| s.one.hero.health = 0);
    assert_eq!(e, CardsError::NonPositiveStat { field: "health" });
    let SetupError::Invalid(e) = bad(&|s| s.two.cards.pop().map(|_| ()).unwrap_or(()));
    assert!(matches!(e, CardsError::WrongDeckSize { found: 29, .. }));
    let SetupError::Invalid(e) = bad(&|s| s.one.hero.starting_hand = 31);
    assert!(matches!(e, CardsError::StartingHandExceedsDeck { .. }));
    let SetupError::Invalid(e) = bad(&|s| s.one.cards[3].cost = 0);
    assert_eq!(e, CardsError::CostBelowOne { field: "cost" });
    let SetupError::Invalid(e) = bad(&|s| s.two.cards[0].force = 0);
    assert_eq!(e, CardsError::NonPositiveStat { field: "force" });
    let SetupError::Invalid(e) = bad(&|s| s.two.cards[0].health = MAX_STAT + 1);
    assert!(matches!(
        e,
        CardsError::StatTooHigh {
            field: "health",
            ..
        }
    ));
}

#[test]
fn setup_errors_have_a_stable_code_and_a_readable_message() {
    let mut s = setup(hero(2, 3, 8, vec![]), hero(1, 1, 8, vec![]));
    s.one.hero.max_time = 0;
    let err = new_game(s, 1).unwrap_err();
    assert_eq!(err.code(), "invalid_setup");
    assert!(err.to_string().contains("max_time"), "{err}");
}

#[test]
fn setup_parses_from_json_and_rejects_unknown_fields() {
    let s = setup(
        hero(2, 3, 8, vec![draw(2, 1)]),
        hero(1, 1, 8, vec![draw(2, 1)]),
    );
    let json = serde_json::to_string(&s).unwrap();
    assert_eq!(serde_json::from_str::<GameSetup>(&json).unwrap(), s);
    let polluted = json.replacen("\"data_versions\"", "\"bonus\":1,\"data_versions\"", 1);
    assert!(serde_json::from_str::<GameSetup>(&polluted).is_err());
}

// ---- the time rule, through hero effects -----------------------------------------------

/// One (budget 3) activates a single effect of cost `cost` against a Two of `two_max`.
fn after_cost(cost: u32, two_max: u32) -> (GameState, Vec<Event>) {
    let (mut state, _) = duel(vec![draw(cost, 1)], two_max);
    let events = apply(&mut state, Action::Activate { effect: 0 }).unwrap();
    (state, events)
}

#[test]
fn cost_below_budget_keeps_the_token() {
    let (state, events) = after_cost(1, 8);
    assert_eq!((state.active(), state.budget()), (PlayerId::One, 2));
    assert!(events.contains(&Event::TokenMoved {
        player: PlayerId::One,
        budget: 2
    }));
    assert!(!kinds(&events).contains(&"ActivePlayerChanged"));
}

#[test]
fn cost_equal_to_budget_crosses_and_the_opponent_gets_one() {
    let (state, events) = after_cost(3, 8);
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 1));
    assert!(events.contains(&Event::ActivePlayerChanged {
        player: PlayerId::Two
    }));
}

#[test]
fn cost_above_budget_gives_the_excess_plus_one() {
    let (state, _) = after_cost(5, 8);
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 3));
}

#[test]
fn the_receivers_max_time_caps_the_budget() {
    let (state, _) = after_cost(9, 5);
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 5));
    let (state, _) = after_cost(5, 2);
    assert_eq!(state.budget(), 2);
}

#[test]
fn a_cost_above_the_activators_own_max_time_is_legal() {
    // One's max_time is 8; the effect costs 20.
    let (state, _) = after_cost(20, 10);
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 10));
}

// ---- hero effects -------------------------------------------------------------------

#[test]
fn drawing_moves_cards_from_the_pile_to_the_hand() {
    let (mut state, _) = duel(vec![draw(1, 2)], 8);
    let events = apply(&mut state, Action::Activate { effect: 0 }).unwrap();
    assert_eq!(state.hand_len(PlayerId::One), 6);
    assert_eq!(state.deck_len(PlayerId::One), 24);
    assert_eq!(
        kinds(&events),
        ["EffectActivated", "Drew", "Drew", "TokenMoved"]
    );
}

#[test]
fn a_draw_larger_than_the_pile_draws_what_is_left() {
    let mut one = hero(2, 8, 8, vec![draw(1, 5)]);
    one.starting_hand = 28; // 2 cards left in the pile
    let (mut state, _) = new_game(setup(one, hero(1, 1, 8, vec![draw(1, 1)])), 1).unwrap();
    let events = apply(&mut state, Action::Activate { effect: 0 }).unwrap();
    assert_eq!(kinds(&events).iter().filter(|k| **k == "Drew").count(), 2);
    assert_eq!(state.hand_len(PlayerId::One), 30);
    assert_eq!(state.deck_len(PlayerId::One), 0);
}

#[test]
fn an_unknown_effect_index_is_refused_and_changes_nothing() {
    let (mut state, _) = duel(vec![draw(1, 1)], 8);
    let before = state.clone();
    for effect in [1, 8, 100, u32::MAX] {
        assert_eq!(
            apply(&mut state, Action::Activate { effect }),
            Err(RuleError::UnknownEffect)
        );
        assert_eq!(state, before);
    }
}

#[test]
fn a_hero_with_several_effects_can_activate_each_at_its_own_cost() {
    let effects = vec![draw(1, 1), draw(2, 2), draw(3, 3)];
    let (state, _) = new_game(
        setup(hero(2, 8, 8, effects), hero(1, 1, 8, vec![draw(1, 1)])),
        4,
    )
    .unwrap();
    assert_eq!(
        legal_actions(&state),
        vec![
            Action::Activate { effect: 0 },
            Action::Activate { effect: 1 },
            Action::Activate { effect: 2 },
            Action::Concede
        ]
    );
    for (i, (cost, count)) in [(1, 1), (2, 2), (3, 3)].into_iter().enumerate() {
        let mut s = state.clone();
        let effect = i as u32;
        let events = apply(&mut s, Action::Activate { effect }).unwrap();
        assert_eq!(
            events.first(),
            Some(&Event::EffectActivated {
                player: PlayerId::One,
                effect,
                cost
            })
        );
        assert_eq!(s.budget(), 8 - cost);
        assert_eq!(s.hand_len(PlayerId::One), 4 + count as usize);
    }
}

// ---- no automatic draw ----------------------------------------------------------------

#[test]
fn nothing_is_drawn_automatically_and_the_pile_only_shrinks_through_activations() {
    let (mut state, _) = duel(vec![draw(3, 2)], 8);
    // One activates (cost 3 = budget): hands over to Two.
    apply(&mut state, Action::Activate { effect: 0 }).unwrap();
    assert_eq!(state.active(), PlayerId::Two);
    assert_eq!(state.hand_len(PlayerId::One), 6, "only the activation drew");
    assert_eq!(state.deck_len(PlayerId::One), 24);
    // The handover gave Two no card, and Two's pile is untouched.
    assert_eq!(state.hand_len(PlayerId::Two), 4);
    assert_eq!(state.deck_len(PlayerId::Two), 26);
    // Two activates its own effect and stays active (cost 1 against budget 1 crosses back).
    apply(&mut state, Action::Activate { effect: 0 }).unwrap();
    assert_eq!(state.active(), PlayerId::One);
    assert_eq!(
        state.hand_len(PlayerId::One),
        6,
        "no draw on getting the token back"
    );
    assert_eq!(state.deck_len(PlayerId::One), 24);
    assert_eq!(state.hand_len(PlayerId::Two), 5);
    assert_eq!(state.deck_len(PlayerId::Two), 25);
}

#[test]
fn there_is_no_pass_action() {
    let (state, _) = duel(vec![draw(1, 1)], 8);
    assert!(
        legal_actions(&state)
            .iter()
            .all(|a| matches!(a, Action::Activate { .. } | Action::Concede))
    );
}

// ---- concede, game over ---------------------------------------------------------------

#[test]
fn concede_records_the_winner_and_the_reason_and_ends_the_game() {
    let (mut state, _) = duel(vec![draw(1, 1)], 8);
    let events = apply(&mut state, Action::Concede).unwrap();
    assert_eq!(
        events,
        vec![Event::GameWon {
            winner: PlayerId::Two,
            reason: WinReason::Concede
        }]
    );
    assert_eq!(state.winner(), Some((PlayerId::Two, WinReason::Concede)));
    assert!(legal_actions(&state).is_empty());
    let after = state.clone();
    for action in [Action::Concede, Action::Activate { effect: 0 }] {
        assert_eq!(apply(&mut state, action), Err(RuleError::GameOver));
        assert_eq!(state, after);
    }
}

#[test]
fn rule_errors_have_stable_codes() {
    assert_eq!(RuleError::GameOver.code(), "game_over");
    assert_eq!(RuleError::UnknownEffect.code(), "unknown_effect");
    assert_eq!(
        RuleError::EffectNotApplicable.code(),
        "effect_not_applicable"
    );
}

// ---- legal_actions ---------------------------------------------------------------------

#[test]
fn legal_actions_agree_with_apply() {
    for seed in 0..30 {
        let (mut state, _) = new_game(
            setup(
                hero(1 + (seed % 2) as u32, 3, 8, vec![draw(1, 1), draw(4, 2)]),
                hero(1, 2, 6, vec![draw(2, 1), draw(9, 3)]),
            ),
            seed,
        )
        .unwrap();
        while state.winner().is_none() {
            let legal = legal_actions(&state);
            assert_eq!(legal.last(), Some(&Action::Concede));
            let universe = (0..12)
                .map(|effect| Action::Activate { effect })
                .chain([Action::Concede, Action::Activate { effect: u32::MAX }]);
            for action in universe {
                let mut probe = state.clone();
                assert_eq!(
                    apply(&mut probe, action).is_ok(),
                    legal.contains(&action),
                    "seed {seed}: {action:?}"
                );
            }
            let pick = legal.first().copied().unwrap();
            apply(&mut state, pick).unwrap();
        }
        assert!(legal_actions(&state).is_empty());
    }
}

// ---- journal ---------------------------------------------------------------------------

#[test]
fn an_activation_that_hands_over_logs_in_order() {
    let (_, events) = after_cost(3, 8);
    assert_eq!(
        kinds(&events),
        [
            "EffectActivated",
            "Drew",
            "TokenMoved",
            "ActivePlayerChanged"
        ]
    );
    assert!(matches!(
        events.first(),
        Some(Event::EffectActivated {
            player: PlayerId::One,
            effect: 0,
            cost: 3
        })
    ));
}

#[test]
fn an_activation_that_stays_logs_no_player_change() {
    let (_, events) = after_cost(1, 8);
    assert_eq!(kinds(&events), ["EffectActivated", "Drew", "TokenMoved"]);
}

#[test]
fn a_full_game_is_deterministic() {
    let play = |seed: u64| {
        let (mut state, mut log) = new_game(
            setup(
                hero(2, 3, 8, vec![draw(1, 1), draw(4, 2)]),
                hero(1, 2, 6, vec![draw(2, 1)]),
            ),
            seed,
        )
        .unwrap();
        while state.winner().is_none() {
            let action = legal_actions(&state).into_iter().next().unwrap();
            log.extend(apply(&mut state, action).unwrap());
        }
        (state, log)
    };
    assert_eq!(play(9), play(9));
}

// ---- actions as data -------------------------------------------------------------------

#[test]
fn actions_travel_as_snake_case_json_and_reject_unknown_fields() {
    let json = serde_json::to_string(&Action::Activate { effect: 2 }).unwrap();
    assert_eq!(json, r#"{"activate":{"effect":2}}"#);
    assert_eq!(
        serde_json::to_string(&Action::Concede).unwrap(),
        r#""concede""#
    );
    assert_eq!(
        serde_json::from_str::<Action>(r#"{"activate":{"effect":2}}"#).unwrap(),
        Action::Activate { effect: 2 }
    );
    assert!(serde_json::from_str::<Action>(r#"{"activate":{"effect":2,"x":1}}"#).is_err());
    assert!(serde_json::from_str::<Action>(r#"{"activate":{"effect":-1}}"#).is_err());
}
