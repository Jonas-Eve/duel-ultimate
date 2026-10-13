//! Public-API tests of the engine: setup, the time rule through hero effects, concede,
//! `legal_actions`, the event log. Cases needing private state live next to `GameState`.

use duel_cards::{CardsError, CreatureStats, Effect, Hero, HeroEffect, MAX_STAT};
use duel_engine::{
    Action, CardInstanceId, Event, GameSetup, GameState, PlayerId, PlayerSetup, RuleError,
    SetupError, Target, WinReason, apply, legal_actions, new_game,
};
use serde_json::{Value, json};
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
        .map(|n| stats(n, 1 + n % 3, 1 + n % 2, 1 + n % 4))
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
            Event::Played { .. } => "Played",
            Event::Attacked { .. } => "Attacked",
            Event::Damaged { .. } => "Damaged",
            Event::CreatureDamaged { .. } => "CreatureDamaged",
            Event::Riposted { .. } => "Riposted",
            Event::Died { .. } => "Died",
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
    let SetupError::Invalid(e) = bad(&|s| s.one.cards[5].riposte = MAX_STAT + 1);
    assert_eq!(
        e,
        CardsError::StatTooHigh {
            field: "riposte",
            value: MAX_STAT + 1,
            max: MAX_STAT
        }
    );
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
    let effect_actions: Vec<_> = legal_actions(&state)
        .into_iter()
        .filter(|a| !matches!(a, Action::Play { .. }))
        .collect();
    assert_eq!(
        effect_actions,
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
    assert!(legal_actions(&state).iter().all(|a| matches!(
        a,
        Action::Play { .. } | Action::Activate { .. } | Action::Concede
    )));
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
    assert_eq!(RuleError::CardNotInHand.code(), "card_not_in_hand");
    assert_eq!(
        RuleError::EffectNotApplicable.code(),
        "effect_not_applicable"
    );
    assert_eq!(
        RuleError::AttackerNotOnBoard.code(),
        "attacker_not_on_board"
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
            // Every creature in play (either side) attacking the hero or any of the 64
            // first ids, own creatures, hand cards and unknown ids included.
            let in_play: Vec<CardInstanceId> = [PlayerId::One, PlayerId::Two]
                .into_iter()
                .flat_map(|p| state.board(p))
                .collect();
            let creature_attacks: Vec<Action> = in_play
                .iter()
                .flat_map(|attacker| {
                    (0..64).map(move |n| Action::Attack {
                        attacker: *attacker,
                        target: Target::Creature(CardInstanceId(n)),
                    })
                })
                .collect();
            let universe = (0..12)
                .map(|effect| Action::Activate { effect })
                .chain((0..64).map(|n| Action::Play {
                    card: CardInstanceId(n),
                }))
                .chain((0..64).map(|n| Action::Attack {
                    attacker: CardInstanceId(n),
                    target: Target::Hero,
                }))
                .chain(creature_attacks)
                .chain([Action::Concede, Action::Activate { effect: u32::MAX }]);
            for action in universe {
                let mut probe = state.clone();
                assert_eq!(
                    apply(&mut probe, action).is_ok(),
                    legal.contains(&action),
                    "seed {seed}: {action:?}"
                );
            }
            // Alternate between the first (a Play when there is a card) and the last
            // non-Concede action, to exercise both kinds.
            let playable: Vec<_> = legal.iter().filter(|a| **a != Action::Concede).collect();
            let pick = match (playable.first(), playable.last()) {
                (Some(first), Some(last)) => **if state.budget() % 2 == 0 { first } else { last },
                _ => Action::Concede,
            };
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
        serde_json::to_string(&Action::Play {
            card: CardInstanceId(5)
        })
        .unwrap(),
        r#"{"play":{"card":5}}"#
    );
    assert_eq!(
        serde_json::from_str::<Action>(r#"{"play":{"card":5}}"#).unwrap(),
        Action::Play {
            card: CardInstanceId(5)
        }
    );
    assert!(serde_json::from_str::<Action>(r#"{"play":{"card":5,"x":1}}"#).is_err());
    assert!(serde_json::from_str::<Action>(r#"{"play":{"card":-1}}"#).is_err());
    assert_eq!(
        serde_json::from_str::<Action>(r#"{"activate":{"effect":2}}"#).unwrap(),
        Action::Activate { effect: 2 }
    );
    assert!(serde_json::from_str::<Action>(r#"{"activate":{"effect":2,"x":1}}"#).is_err());
    assert!(serde_json::from_str::<Action>(r#"{"activate":{"effect":-1}}"#).is_err());
}

// ---- Play ----------------------------------------------------------------------------------

/// A deck where every card costs `cost` to summon.
fn flat_cards(cost: u32) -> Vec<CreatureStats> {
    (0..30).map(|_| stats(cost, 2, 1, 3)).collect()
}

/// One (budget 3, max_time 8) against a Two of `two_max`; every card costs `cost`.
fn flat_duel(cost: u32, two_max: u32) -> GameState {
    let mut s = setup(
        hero(2, 3, 8, vec![draw(1, 1)]),
        hero(1, 1, two_max, vec![draw(1, 1)]),
    );
    s.one.cards = flat_cards(cost);
    s.two.cards = flat_cards(cost);
    new_game(s, 7).unwrap().0
}

fn first_in_hand(state: &GameState, player: PlayerId) -> CardInstanceId {
    state.hand(player)[0]
}

fn play_first(state: &mut GameState) -> Vec<Event> {
    let card = first_in_hand(state, state.active());
    apply(state, Action::Play { card }).unwrap()
}

#[test]
fn playing_moves_the_card_from_the_hand_to_the_board() {
    let mut state = flat_duel(1, 8);
    let card = first_in_hand(&state, PlayerId::One);
    assert!(state.board(PlayerId::One).is_empty());
    apply(&mut state, Action::Play { card }).unwrap();
    assert_eq!(state.hand_len(PlayerId::One), 3);
    assert!(!state.hand(PlayerId::One).contains(&card));
    assert_eq!(state.board(PlayerId::One), vec![card]);
    assert!(state.board(PlayerId::Two).is_empty());
    assert_eq!(
        state.hand_len(PlayerId::Two),
        4,
        "the opponent is untouched"
    );
}

#[test]
fn the_board_keeps_the_order_of_summoning() {
    let mut state = flat_duel(1, 8);
    let hand = state.hand(PlayerId::One);
    let order = [hand[2], hand[0], hand[3]];
    for card in order {
        apply(&mut state, Action::Play { card }).unwrap();
    }
    assert_eq!(state.board(PlayerId::One), order);
    assert_eq!(state.hand(PlayerId::One), vec![hand[1]]);
}

#[test]
fn play_with_cost_below_budget_keeps_the_token() {
    let mut state = flat_duel(2, 8);
    let events = play_first(&mut state);
    assert_eq!((state.active(), state.budget()), (PlayerId::One, 1));
    assert!(!kinds(&events).contains(&"ActivePlayerChanged"));
}

#[test]
fn play_with_cost_equal_to_budget_crosses_and_the_opponent_gets_one() {
    let mut state = flat_duel(3, 8);
    let events = play_first(&mut state);
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 1));
    assert!(events.contains(&Event::ActivePlayerChanged {
        player: PlayerId::Two
    }));
}

#[test]
fn play_with_cost_above_budget_gives_the_excess_plus_one() {
    let mut state = flat_duel(5, 8);
    play_first(&mut state);
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 3));
}

#[test]
fn play_is_capped_by_the_receivers_max_time_and_may_exceed_the_own_one() {
    let mut state = flat_duel(9, 5);
    play_first(&mut state);
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 5));
    // A cost above One's own max_time (8) is legal.
    let mut state = flat_duel(30, 10);
    play_first(&mut state);
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 10));
}

#[test]
fn a_play_logs_played_then_token_then_player_change() {
    let mut state = flat_duel(3, 8);
    let card = first_in_hand(&state, PlayerId::One);
    let events = apply(&mut state, Action::Play { card }).unwrap();
    assert_eq!(
        events,
        vec![
            Event::Played {
                player: PlayerId::One,
                card,
                cost: 3
            },
            Event::TokenMoved {
                player: PlayerId::Two,
                budget: 1
            },
            Event::ActivePlayerChanged {
                player: PlayerId::Two
            },
        ]
    );
    let mut state = flat_duel(1, 8);
    assert_eq!(kinds(&play_first(&mut state)), ["Played", "TokenMoved"]);
}

#[test]
fn an_illegal_play_is_refused_and_changes_nothing() {
    let mut state = flat_duel(1, 8);
    let on_board = first_in_hand(&state, PlayerId::One);
    apply(&mut state, Action::Play { card: on_board }).unwrap();
    // Ids 0..30 are One's cards, 30..60 Two's: find one still in One's pile.
    let in_pile = (0..30)
        .map(CardInstanceId)
        .find(|c| !state.hand(PlayerId::One).contains(c) && !state.board(PlayerId::One).contains(c))
        .unwrap();
    let cases = [
        (CardInstanceId(9999), "unknown id"),
        (on_board, "already on the board"),
        (first_in_hand(&state, PlayerId::Two), "the opponent's card"),
        (in_pile, "a card still in the pile"),
    ];
    for (card, why) in cases {
        let before = state.clone();
        assert_eq!(
            apply(&mut state, Action::Play { card }),
            Err(RuleError::CardNotInHand),
            "{why}"
        );
        assert_eq!(state, before, "{why}");
    }
}

#[test]
fn play_is_refused_once_the_game_is_over() {
    let mut state = flat_duel(1, 8);
    let card = first_in_hand(&state, PlayerId::One);
    apply(&mut state, Action::Concede).unwrap();
    let after = state.clone();
    assert_eq!(
        apply(&mut state, Action::Play { card }),
        Err(RuleError::GameOver)
    );
    assert_eq!(state, after);
}

#[test]
fn legal_actions_list_the_hand_first_then_effects_then_concede() {
    let state = flat_duel(1, 8);
    let mut expected: Vec<Action> = state
        .hand(PlayerId::One)
        .into_iter()
        .map(|card| Action::Play { card })
        .collect();
    expected.push(Action::Activate { effect: 0 });
    expected.push(Action::Concede);
    assert_eq!(legal_actions(&state), expected);
    assert_eq!(expected.len(), 6);
}

#[test]
fn only_the_active_players_hand_is_playable() {
    let state = flat_duel(1, 8);
    let theirs = state.hand(PlayerId::Two);
    assert!(legal_actions(&state).iter().all(|a| match a {
        Action::Play { card } => !theirs.contains(card),
        _ => true,
    }));
}

#[test]
fn a_hero_without_effects_can_still_play_its_hand() {
    let mut s = setup(hero(2, 3, 8, vec![]), hero(1, 1, 8, vec![draw(1, 1)]));
    s.one.cards = flat_cards(1);
    let (mut state, _) = new_game(s, 3).unwrap();
    assert_eq!(state.winner(), None);
    for _ in 0..2 {
        play_first(&mut state);
    }
    assert_eq!(state.winner(), None);
}

#[test]
fn drawing_everything_then_activating_again_is_refused_as_not_applicable() {
    // With `Play`, an empty pile no longer ends the game, so this error is reachable.
    let (mut state, _) = new_game(
        setup(
            hero(2, 8, 8, vec![draw(1, 30)]),
            hero(1, 1, 8, vec![draw(1, 1)]),
        ),
        1,
    )
    .unwrap();
    apply(&mut state, Action::Activate { effect: 0 }).unwrap();
    assert_eq!(state.deck_len(PlayerId::One), 0);
    assert_eq!(state.winner(), None, "the hand keeps One alive");
    assert_eq!(state.active(), PlayerId::One);
    let before = state.clone();
    assert_eq!(
        apply(&mut state, Action::Activate { effect: 0 }),
        Err(RuleError::EffectNotApplicable)
    );
    assert_eq!(state, before);
    assert!(!legal_actions(&state).contains(&Action::Activate { effect: 0 }));
}

// ---- Attack ----------------------------------------------------------------------------------

/// A deck where every card has these stats (summoning costs 1).
fn attackers(force: u32, attack_cost: u32) -> Vec<CreatureStats> {
    (0..30).map(|_| stats(1, force, attack_cost, 3)).collect()
}

/// One is active with `budget` left (max_time 8) after summoning a creature of `force` and
/// `attack_cost`; Two has `two_max` and `two_health`. Returns the state and the creature.
fn armed(
    force: u32,
    attack_cost: u32,
    budget: u32,
    two_max: u32,
    two_health: u32,
) -> (GameState, CardInstanceId) {
    let mut s = setup(
        hero(2, budget + 1, 8, vec![draw(1, 1)]),
        hero(1, 1, two_max, vec![draw(1, 1)]),
    );
    s.one.cards = attackers(force, attack_cost);
    s.two.cards = attackers(force, attack_cost);
    s.two.hero.health = two_health;
    let (mut state, _) = new_game(s, 7).unwrap();
    play_first(&mut state);
    assert_eq!((state.active(), state.budget()), (PlayerId::One, budget));
    let creature = state.board(PlayerId::One)[0];
    (state, creature)
}

fn hit(creature: CardInstanceId) -> Action {
    Action::Attack {
        attacker: creature,
        target: Target::Hero,
    }
}

#[test]
fn an_attack_deals_the_force_of_the_creature_to_the_opposing_hero() {
    let (mut state, creature) = armed(3, 1, 5, 8, 10);
    let events = apply(&mut state, hit(creature)).unwrap();
    assert_eq!(state.hero_health(PlayerId::Two), 7);
    assert_eq!(
        state.hero_health(PlayerId::One),
        10,
        "the attacker's hero is untouched"
    );
    assert!(events.contains(&Event::Damaged {
        player: PlayerId::Two,
        amount: 3
    }));
    assert_eq!(
        state.board(PlayerId::One),
        vec![creature],
        "the creature stays in play"
    );
}

#[test]
fn attacking_costs_the_attack_cost_through_the_time_rule() {
    // c < b: stays.
    let (mut state, c) = armed(1, 2, 5, 8, 10);
    apply(&mut state, hit(c)).unwrap();
    assert_eq!((state.active(), state.budget()), (PlayerId::One, 3));
    // c = b: crosses, the opponent gets 1.
    let (mut state, c) = armed(1, 2, 2, 8, 10);
    apply(&mut state, hit(c)).unwrap();
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 1));
    // c > b: excess plus one.
    let (mut state, c) = armed(1, 4, 2, 8, 10);
    apply(&mut state, hit(c)).unwrap();
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 3));
    // The receiver's max_time caps it, and the cost may exceed the attacker's own max_time.
    let (mut state, c) = armed(1, 30, 2, 5, 10);
    apply(&mut state, hit(c)).unwrap();
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 5));
}

#[test]
fn a_creature_can_attack_right_after_being_summoned_and_as_often_as_the_budget_allows() {
    // `armed` already attacks without any delay after the summoning; here budget 5, cost 1.
    let (mut state, c) = armed(2, 1, 5, 8, 100);
    for n in 1..=4 {
        let events = apply(&mut state, hit(c)).unwrap();
        assert_eq!(state.hero_health(PlayerId::Two), 100 - 2 * n);
        assert_eq!(state.active(), PlayerId::One, "attack {n} keeps the token");
        assert!(!kinds(&events).contains(&"ActivePlayerChanged"));
    }
    assert_eq!(state.budget(), 1);
    let events = apply(&mut state, hit(c)).unwrap();
    assert_eq!(state.hero_health(PlayerId::Two), 90);
    assert_eq!(
        state.active(),
        PlayerId::Two,
        "the fifth attack spends the last unit"
    );
    assert!(kinds(&events).contains(&"ActivePlayerChanged"));
}

#[test]
fn a_non_lethal_attack_logs_in_order() {
    let (mut state, c) = armed(3, 2, 5, 8, 10);
    let events = apply(&mut state, hit(c)).unwrap();
    assert_eq!(
        events,
        vec![
            Event::Attacked {
                player: PlayerId::One,
                attacker: c,
                cost: 2
            },
            Event::Damaged {
                player: PlayerId::Two,
                amount: 3
            },
            Event::TokenMoved {
                player: PlayerId::One,
                budget: 3
            },
        ]
    );
    let (mut state, c) = armed(3, 2, 2, 8, 10);
    let events = apply(&mut state, hit(c)).unwrap();
    assert_eq!(
        kinds(&events),
        ["Attacked", "Damaged", "TokenMoved", "ActivePlayerChanged"]
    );
    assert_eq!(
        events.last(),
        Some(&Event::ActivePlayerChanged {
            player: PlayerId::Two
        })
    );
}

#[test]
fn a_lethal_attack_ends_the_game_without_handing_over() {
    // Overkill (force 5 against 3 health) and an exact kill, with a cost that would cross.
    for (force, health) in [(5, 3), (4, 4)] {
        let (mut state, c) = armed(force, 6, 2, 8, health);
        let events = apply(&mut state, hit(c)).unwrap();
        assert_eq!(
            events,
            vec![
                Event::Attacked {
                    player: PlayerId::One,
                    attacker: c,
                    cost: 6
                },
                Event::Damaged {
                    player: PlayerId::Two,
                    amount: force
                },
                Event::GameWon {
                    winner: PlayerId::One,
                    reason: WinReason::HeroDown
                },
            ]
        );
        assert_eq!(state.hero_health(PlayerId::Two), 0, "no underflow");
        assert_eq!(state.winner(), Some((PlayerId::One, WinReason::HeroDown)));
        assert_eq!(state.active(), PlayerId::One, "no handover");
        assert_eq!(
            state.budget(),
            2,
            "the budget is untouched: 1 <= budget <= max_time still holds"
        );
        assert!(legal_actions(&state).is_empty());
        let after = state.clone();
        for action in [hit(c), Action::Concede, Action::Activate { effect: 0 }] {
            assert_eq!(apply(&mut state, action), Err(RuleError::GameOver));
            assert_eq!(state, after);
        }
    }
}

/// One and Two each have a creature on the board and One holds the token again.
fn both_armed() -> (GameState, CardInstanceId, CardInstanceId) {
    let mut s = setup(
        hero(2, 1, 8, vec![draw(1, 1)]),
        hero(1, 1, 8, vec![draw(1, 1)]),
    );
    s.one.cards = attackers(1, 1);
    s.two.cards = attackers(1, 1);
    let (mut state, _) = new_game(s, 7).unwrap();
    play_first(&mut state); // One summons and hands over (cost 1 = budget 1)
    play_first(&mut state); // Two does the same
    assert_eq!((state.active(), state.budget()), (PlayerId::One, 1));
    let (mine, theirs) = (state.board(PlayerId::One)[0], state.board(PlayerId::Two)[0]);
    (state, mine, theirs)
}

#[test]
fn an_attacker_that_is_not_on_the_active_board_is_refused_and_changes_nothing() {
    let (mut state, mine, theirs) = both_armed();
    let in_hand = state.hand(PlayerId::One)[0];
    let their_hand = state.hand(PlayerId::Two)[0];
    let cases = [
        (theirs, "the opponent's creature"),
        (in_hand, "a card still in hand"),
        (their_hand, "a card in the opponent's hand"),
        (CardInstanceId(9999), "an unknown id"),
    ];
    for (attacker, why) in cases {
        let before = state.clone();
        assert_eq!(
            apply(&mut state, hit(attacker)),
            Err(RuleError::AttackerNotOnBoard),
            "{why}"
        );
        assert_eq!(state, before, "{why}");
    }
    assert!(apply(&mut state, hit(mine)).is_ok());
}

/// The attacks the active player is owed: per attacker (board order), the opposing hero
/// first, then every creature of the opposing board (board order).
fn expected_attacks(state: &GameState) -> Vec<Action> {
    let targets: Vec<Target> = std::iter::once(Target::Hero)
        .chain(
            state
                .board(state.active().opponent())
                .into_iter()
                .map(Target::Creature),
        )
        .collect();
    state
        .board(state.active())
        .into_iter()
        .flat_map(|attacker| {
            targets
                .iter()
                .map(move |target| Action::Attack {
                    attacker,
                    target: *target,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn legal_actions_list_hand_effects_attacks_then_concede() {
    let (mut state, mine, theirs) = both_armed();
    let second = state.hand(PlayerId::One)[0];
    apply(&mut state, Action::Play { card: second }).unwrap();
    assert_eq!(
        state.active(),
        PlayerId::Two,
        "cost 1 against budget 1 crosses"
    );
    let two_board = state.board(PlayerId::Two);
    // Two's only creature attacks One's hero, then each of One's two creatures (3 targets).
    let expected: Vec<Action> = state
        .hand(PlayerId::Two)
        .into_iter()
        .map(|card| Action::Play { card })
        .chain([Action::Activate { effect: 0 }])
        .chain([hit(theirs), strike(theirs, mine), strike(theirs, second)])
        .chain([Action::Concede])
        .collect();
    assert_eq!(legal_actions(&state), expected);
    assert_eq!(
        two_board,
        vec![theirs],
        "only the active player's creatures attack"
    );
    assert!(!legal_actions(&state).contains(&hit(mine)));
    assert!(!legal_actions(&state).contains(&hit(second)));
}

#[test]
fn attacks_are_listed_per_attacker_hero_first_then_the_opposing_board_in_order() {
    // Two creatures on each side.
    let state = skirmish(stats(1, 1, 1, 3), stats(1, 1, 1, 3), 2, 8);
    let (ones, twos) = (state.board(PlayerId::One), state.board(PlayerId::Two));
    assert_eq!((ones.len(), twos.len()), (2, 2));
    let hand: Vec<Action> = state
        .hand(PlayerId::One)
        .into_iter()
        .map(|card| Action::Play { card })
        .collect();
    let expected: Vec<Action> = hand
        .into_iter()
        .chain([Action::Activate { effect: 0 }])
        .chain([
            hit(ones[0]),
            strike(ones[0], twos[0]),
            strike(ones[0], twos[1]),
            hit(ones[1]),
            strike(ones[1], twos[0]),
            strike(ones[1], twos[1]),
        ])
        .chain([Action::Concede])
        .collect();
    assert_eq!(legal_actions(&state), expected);
    assert_eq!(expected_attacks(&state).len(), 6, "attackers x (1 + board)");
}

#[test]
fn a_creature_never_targets_its_own_side() {
    let state = skirmish(stats(1, 1, 1, 3), stats(1, 1, 1, 3), 2, 8);
    let ones = state.board(PlayerId::One);
    let legal = legal_actions(&state);
    for attacker in &ones {
        for own in &ones {
            assert!(
                !legal.contains(&strike(*attacker, *own)),
                "{attacker:?} -> {own:?}"
            );
        }
    }
    // Nor cards that are not in play.
    for card in state.hand(PlayerId::Two) {
        assert!(!legal.contains(&strike(ones[0], card)));
    }
}

#[test]
fn a_dead_creature_is_neither_an_attacker_nor_a_target_in_legal_actions() {
    // One kills Two's creature (force 3 against 3 health) and keeps the token (cost 1 of 3).
    let mut state = skirmish(stats(1, 3, 1, 3), stats(3, 1, 1, 3), 1, 8);
    let (a, x) = duelists(&state);
    apply(&mut state, strike(a, x)).unwrap();
    assert_eq!(state.active(), PlayerId::One);
    assert_eq!(state.discard(PlayerId::Two), vec![x]);
    let legal = legal_actions(&state);
    assert_eq!(
        legal
            .iter()
            .filter(|a| matches!(a, Action::Attack { .. }))
            .count(),
        1,
        "only the hero is left to hit"
    );
    assert!(legal.contains(&hit(a)));
    assert!(!legal.contains(&strike(a, x)), "dead: not a target");
    assert_eq!(
        legal
            .iter()
            .filter(|a| matches!(a, Action::Attack { .. }))
            .copied()
            .collect::<Vec<_>>(),
        expected_attacks(&state)
    );

    // Now the other way round: Two kills One's creature and the dead one is not an attacker.
    let mut state = skirmish(stats(1, 1, 1, 1), stats(1, 5, 1, 3), 1, 8);
    let (a, x) = duelists(&state);
    play_first(&mut state); // One summons a second creature, the token crosses to Two
    assert_eq!(state.active(), PlayerId::Two);
    apply(&mut state, strike(x, a)).unwrap();
    assert_eq!(state.discard(PlayerId::One), vec![a]);
    let legal = legal_actions(&state);
    assert!(
        !legal.iter().any(|act| matches!(
            act,
            Action::Attack { attacker, target } if *attacker == a || *target == Target::Creature(a)
        )),
        "{legal:?}"
    );
    assert_eq!(
        legal
            .iter()
            .filter(|a| matches!(a, Action::Attack { .. }))
            .copied()
            .collect::<Vec<_>>(),
        expected_attacks(&state)
    );
}

#[test]
fn a_player_without_creatures_is_offered_no_attack() {
    let state = flat_duel(1, 8);
    assert!(
        !legal_actions(&state)
            .iter()
            .any(|a| matches!(a, Action::Attack { .. }))
    );
}

#[test]
fn attacks_travel_as_snake_case_json() {
    let action = hit(CardInstanceId(5));
    let json = serde_json::to_string(&action).unwrap();
    assert_eq!(json, r#"{"attack":{"attacker":5,"target":"hero"}}"#);
    assert_eq!(serde_json::from_str::<Action>(&json).unwrap(), action);
    assert!(
        serde_json::from_str::<Action>(r#"{"attack":{"attacker":5,"target":"moon"}}"#).is_err()
    );
    assert!(
        serde_json::from_str::<Action>(r#"{"attack":{"attacker":5,"target":"hero","x":1}}"#)
            .is_err()
    );
}

/// Plays one game by always taking the last action other than `Concede` whose target is not
/// a creature (an attack on the hero as soon as a creature is in play): a game in which only
/// heroes are ever attacked, like every game of S1.
fn play_to_the_end(seed: u64) -> (GameState, Vec<Event>) {
    play_to_the_end_from(s1_setup(), seed)
}

/// The setup of the S1 and S2a games (its `data_versions` is `["fixture@1.0.0"]`).
fn s1_setup() -> GameSetup {
    setup(
        hero(2, 3, 8, vec![draw(1, 1), draw(4, 2)]),
        hero(1, 2, 6, vec![draw(2, 1)]),
    )
}

/// The same setup with no recorded data version: `GameStarted` then serializes exactly as it
/// did before `data_versions` existed, which is what the golden journals were recorded with.
fn without_versions(mut setup: GameSetup) -> GameSetup {
    setup.data_versions = vec![];
    setup
}

fn play_to_the_end_from(setup: GameSetup, seed: u64) -> (GameState, Vec<Event>) {
    let (mut state, mut log) = new_game(setup, seed).unwrap();
    while state.winner().is_none() {
        let action = legal_actions(&state)
            .into_iter()
            .rfind(|a| {
                *a != Action::Concede
                    && !matches!(a, Action::Attack { target, .. } if *target != Target::Hero)
            })
            .unwrap_or(Action::Concede);
        log.extend(apply(&mut state, action).unwrap());
    }
    (state, log)
}

#[test]
fn a_full_game_to_a_victory_by_attack_is_deterministic() {
    let mut victories = 0;
    for seed in 0..20 {
        let (state, log) = play_to_the_end(seed);
        let (again, log_again) = play_to_the_end(seed);
        assert_eq!(state, again);
        assert_eq!(log, log_again);
        if let Some((winner, WinReason::HeroDown)) = state.winner() {
            victories += 1;
            assert_eq!(state.hero_health(winner.opponent()), 0);
            assert_eq!(
                log.last(),
                Some(&Event::GameWon {
                    winner,
                    reason: WinReason::HeroDown
                })
            );
        }
    }
    assert!(victories > 0, "attacking games end by a hero going down");
}

// ---- S1 golden fingerprints -------------------------------------------------------------------

/// FNV-1a (64 bits) of a byte string: a stable, dependency-free fingerprint.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// `(winner, reason, number of events, FNV-1a of the events as JSON)` per seed 0..20,
/// recorded on the engine as merged by S1, before creatures became attackable. Attacking a
/// hero must keep producing exactly these games.
const S1_GOLDEN: [(PlayerId, WinReason, usize, u64); 20] = [
    (PlayerId::One, WinReason::HeroDown, 178, 0x3b151a1a9605c44e),
    (PlayerId::One, WinReason::HeroDown, 177, 0x8b94d569b261925e),
    (PlayerId::One, WinReason::HeroDown, 198, 0x5f474243cdd2f630),
    (PlayerId::One, WinReason::HeroDown, 180, 0x199476a8e485a0eb),
    (PlayerId::Two, WinReason::HeroDown, 206, 0x16af5747217869e1),
    (PlayerId::One, WinReason::HeroDown, 203, 0xd511655f75711327),
    (PlayerId::One, WinReason::HeroDown, 198, 0xc15cd7315b72db1c),
    (PlayerId::Two, WinReason::HeroDown, 201, 0x67813e90fd9a9896),
    (PlayerId::Two, WinReason::HeroDown, 184, 0xc17a7c24d3d6d626),
    (PlayerId::One, WinReason::HeroDown, 183, 0xbc8b79f422fe4dc5),
    (PlayerId::One, WinReason::HeroDown, 203, 0x3a663674ede8f6b5),
    (PlayerId::One, WinReason::HeroDown, 198, 0x469a02939eafb809),
    (PlayerId::One, WinReason::HeroDown, 175, 0xeb1c485fe42abedd),
    (PlayerId::One, WinReason::HeroDown, 183, 0xfe0c281c83eeda31),
    (PlayerId::One, WinReason::HeroDown, 203, 0xa837e5ede5fb2465),
    (PlayerId::One, WinReason::HeroDown, 183, 0xd534faec380ec8de),
    (PlayerId::One, WinReason::HeroDown, 203, 0xab981dd4311f1ec0),
    (PlayerId::One, WinReason::HeroDown, 198, 0x182570caee0e4d49),
    (PlayerId::One, WinReason::HeroDown, 175, 0x80ca15486dedadc1),
    (PlayerId::One, WinReason::HeroDown, 228, 0xed66defe86333441),
];

#[test]
fn s1_hero_only_games_keep_their_exact_outcome_and_journal() {
    let observed: Vec<_> = (0..20)
        .map(|seed| {
            let (state, log) = play_to_the_end_from(without_versions(s1_setup()), seed);
            let (winner, reason) = state.winner().unwrap();
            let json = serde_json::to_string(&log).unwrap();
            (winner, reason, log.len(), fnv1a(json.as_bytes()))
        })
        .collect();
    assert_eq!(observed, S1_GOLDEN);
}

// ---- S2a golden fingerprints ------------------------------------------------------------------

/// Plays one game with the S1 setup, picking at each step a pseudo-random action among
/// `legal_actions` other than `Concede` (creature targets included), from a generator owned
/// by the test (a 64-bit LCG), so that the policy does not depend on the engine's own seed.
/// Returns the final state and the journal.
fn play_scripted_s2a(seed: u64) -> (GameState, Vec<Event>) {
    play_scripted_s2a_from(s1_setup(), seed)
}

fn play_scripted_s2a_from(setup: GameSetup, seed: u64) -> (GameState, Vec<Event>) {
    let (mut state, mut log) = new_game(setup, seed).unwrap();
    let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0xdead_beef;
    while state.winner().is_none() {
        let actions: Vec<_> = legal_actions(&state)
            .into_iter()
            .filter(|a| *a != Action::Concede)
            .collect();
        if actions.is_empty() {
            log.extend(apply(&mut state, Action::Concede).unwrap());
            continue;
        }
        rng = rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let pick = usize::try_from((rng >> 33) % actions.len() as u64).unwrap();
        log.extend(apply(&mut state, actions[pick]).unwrap());
    }
    (state, log)
}

/// `(winner, reason, number of events, FNV-1a of the events as JSON)` per seed 0..20,
/// recorded on the engine as merged by S2a, before `provocation` and `riposte` existed.
/// A deck without either must keep producing exactly these games (S2b-T2 and T3 replay it).
const S2A_GOLDEN: [(PlayerId, WinReason, usize, u64); 20] = [
    (PlayerId::Two, WinReason::HeroDown, 86, 0x80c1381779ea79a7),
    (PlayerId::Two, WinReason::HeroDown, 98, 0x908d721b91e5d893),
    (PlayerId::Two, WinReason::HeroDown, 77, 0x54de9d8b184fabea),
    (PlayerId::Two, WinReason::HeroDown, 134, 0x23bd68595d38a5cc),
    (PlayerId::One, WinReason::HeroDown, 169, 0x7e66ce1c77ff57b1),
    (PlayerId::Two, WinReason::HeroDown, 278, 0x98bbc6c56698d62d),
    (PlayerId::Two, WinReason::HeroDown, 147, 0x465ec0422879479c),
    (PlayerId::One, WinReason::HeroDown, 185, 0x1804dec2bf7ece73),
    (PlayerId::One, WinReason::HeroDown, 287, 0x79ae1cfe65efdb37),
    (PlayerId::Two, WinReason::HeroDown, 97, 0x66768c08e59e417e),
    (PlayerId::One, WinReason::HeroDown, 71, 0x8e0890bcf9c88878),
    (PlayerId::Two, WinReason::HeroDown, 126, 0x815c922de826a087),
    (PlayerId::Two, WinReason::HeroDown, 157, 0x843a3979c5094ce7),
    (PlayerId::One, WinReason::HeroDown, 215, 0xecb4ef78cee5f4fe),
    (PlayerId::One, WinReason::HeroDown, 76, 0xa6c716fc0f1dbad9),
    (PlayerId::Two, WinReason::HeroDown, 133, 0x0452da6eb21a6562),
    (PlayerId::One, WinReason::HeroDown, 146, 0xf25f28d39f5724e5),
    (PlayerId::Two, WinReason::HeroDown, 231, 0x6cf53186683b1074),
    (PlayerId::Two, WinReason::HeroDown, 89, 0x1ed1ba46e872c1ac),
    (PlayerId::Two, WinReason::HeroDown, 152, 0x92909ca938ebc409),
];

#[test]
fn s2a_scripted_games_keep_their_exact_outcome_and_journal() {
    let mut deaths = 0;
    let mut creature_attacks = 0;
    let observed: Vec<_> = (0..20)
        .map(|seed| {
            let (state, log) = play_scripted_s2a_from(without_versions(s1_setup()), seed);
            deaths += log
                .iter()
                .filter(|e| matches!(e, Event::Died { .. }))
                .count();
            creature_attacks += log
                .iter()
                .filter(|e| matches!(e, Event::CreatureDamaged { .. }))
                .count();
            let (winner, reason) = state.winner().unwrap();
            let json = serde_json::to_string(&log).unwrap();
            (winner, reason, log.len(), fnv1a(json.as_bytes()))
        })
        .collect();
    assert!(deaths > 0, "the golden games must include a death");
    assert!(
        creature_attacks > 0,
        "the golden games must include an attack on a creature"
    );
    assert_eq!(observed, S2A_GOLDEN);
}

// ---- Data versions do not change the journals ---------------------------------------------------

/// Removes the `data_versions` key from the first event (`GameStarted`) of a journal as JSON
/// and returns it, with the rest of the journal.
fn split_versions(log: &[Event]) -> (Value, Value, Vec<Value>) {
    let mut events: Vec<Value> = log
        .iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect();
    let mut first = events.remove(0);
    let versions = first["game_started"]
        .as_object_mut()
        .unwrap()
        .remove("data_versions")
        .unwrap_or(Value::Null);
    (first, versions, events)
}

#[test]
fn non_empty_versions_only_add_the_key_to_the_s1_and_s2a_journals() {
    type Play = fn(GameSetup, u64) -> (GameState, Vec<Event>);
    let games: [(&str, Play); 2] = [
        ("S1", play_to_the_end_from),
        ("S2a", play_scripted_s2a_from),
    ];
    for (name, play) in games {
        for seed in 0..20 {
            let (plain, plain_log) = play(without_versions(s1_setup()), seed);
            let versioned = s1_setup();
            assert_eq!(versioned.data_versions, ["fixture@1.0.0"]);
            let (state, log) = play(versioned, seed);
            assert_eq!(state.winner(), plain.winner(), "{name} seed {seed}");
            assert_eq!(log.len(), plain_log.len(), "{name} seed {seed}");
            let (head, versions, rest) = split_versions(&log);
            let (plain_head, plain_versions, plain_rest) = split_versions(&plain_log);
            assert_eq!(head, plain_head, "{name} seed {seed}");
            assert_eq!(plain_versions, Value::Null, "{name} seed {seed}");
            assert_eq!(versions, json!(["fixture@1.0.0"]), "{name} seed {seed}");
            assert_eq!(rest, plain_rest, "{name} seed {seed}");
        }
    }
}

// ---- Attacking creatures -------------------------------------------------------------------

fn stats(cost: u32, force: u32, attack_cost: u32, health: u32) -> CreatureStats {
    stats_with(cost, force, attack_cost, health, false, 0)
}

/// Stats with `provocation` and `riposte`, reusable by the S2b tests (T2, T3).
#[allow(dead_code)] // first used by S2b-T2 / S2b-T3
fn stats_with(
    cost: u32,
    force: u32,
    attack_cost: u32,
    health: u32,
    provocation: bool,
    riposte: u32,
) -> CreatureStats {
    CreatureStats {
        cost,
        force,
        attack_cost,
        health,
        provocation,
        riposte,
    }
}

fn strike(attacker: CardInstanceId, victim: CardInstanceId) -> Action {
    Action::Attack {
        attacker,
        target: Target::Creature(victim),
    }
}

/// Both sides summon `rounds` creatures, alternating, One first; every card of One is `one`
/// and every card of Two is `two` (cards are plain copies, so any can be played). One's
/// cost must be 1 (with its start budget of 1, each summoning hands the token over). One
/// ends active, holding `min(two.cost, 8)` time (1 beyond the first round: `two.cost` must
/// then be 1), with `rounds` creatures facing `rounds` creatures.
fn skirmish(one: CreatureStats, two: CreatureStats, rounds: usize, two_max: u32) -> GameState {
    assert_eq!(one.cost, 1);
    let two_cost = two.cost;
    assert!(rounds == 1 || two_cost == 1);
    let mut s = setup(
        hero(2, 1, 8, vec![draw(1, 1)]),
        hero(1, 1, two_max, vec![draw(1, 1)]),
    );
    s.one.cards = vec![one; 30];
    s.two.cards = vec![two; 30];
    let (mut state, _) = new_game(s, 7).unwrap();
    for _ in 0..rounds {
        play_first(&mut state);
        play_first(&mut state);
    }
    assert_eq!(state.active(), PlayerId::One);
    assert_eq!(state.budget(), two_cost.min(8));
    assert_eq!(state.board(PlayerId::One).len(), rounds);
    assert_eq!(state.board(PlayerId::Two).len(), rounds);
    state
}

/// One's first creature and Two's first creature of a one-round skirmish.
fn duelists(state: &GameState) -> (CardInstanceId, CardInstanceId) {
    (state.board(PlayerId::One)[0], state.board(PlayerId::Two)[0])
}

#[test]
fn an_attack_on_a_creature_deals_the_force_and_a_survivor_stays_a_target() {
    // Force 2 against 5 health, 6 time units at 1 per attack.
    let mut state = skirmish(stats(1, 2, 1, 5), stats(6, 1, 1, 5), 1, 8);
    let (a, x) = duelists(&state);
    assert_eq!(state.creature_health(x), Some(5));
    apply(&mut state, strike(a, x)).unwrap();
    assert_eq!(state.creature_health(x), Some(3));
    assert_eq!(state.creature_health(a), Some(5), "no riposte");
    assert_eq!(state.board(PlayerId::Two), vec![x], "the survivor stays");
    assert!(state.discard(PlayerId::Two).is_empty());
    assert_eq!(state.hero_health(PlayerId::Two), 10, "heroes are untouched");
    assert_eq!(state.hero_health(PlayerId::One), 10);
    assert_eq!(state.winner(), None);
    apply(&mut state, strike(a, x)).unwrap();
    assert_eq!(
        state.creature_health(x),
        Some(1),
        "still targetable, and damage accumulates"
    );
}

#[test]
fn a_creature_attacked_to_death_goes_to_its_owners_discard_and_cannot_act_or_be_hit() {
    // One's attack costs its whole budget: the token crosses and Two is active.
    let mut state = skirmish(stats(1, 3, 1, 3), stats(1, 1, 1, 3), 1, 8);
    let (a, x) = duelists(&state);
    let hand_two = state.hand(PlayerId::Two);
    apply(&mut state, strike(a, x)).unwrap();
    assert!(state.board(PlayerId::Two).is_empty());
    assert_eq!(state.discard(PlayerId::Two), vec![x]);
    assert!(state.discard(PlayerId::One).is_empty());
    assert_eq!(state.creature_health(x), None, "no longer in play");
    assert_eq!(state.hand(PlayerId::Two), hand_two, "nothing else moved");
    assert_eq!(state.board(PlayerId::One), vec![a]);
    assert_eq!(state.active(), PlayerId::Two);
    // The dead creature cannot attack...
    let before = state.clone();
    assert_eq!(
        apply(&mut state, strike(x, a)),
        Err(RuleError::AttackerNotOnBoard)
    );
    assert_eq!(
        apply(&mut state, hit(x)),
        Err(RuleError::AttackerNotOnBoard)
    );
    assert_eq!(state, before);
    // ... nor be attacked (Two summons another card, then it is One's turn again).
    play_first(&mut state);
    assert_eq!(state.active(), PlayerId::One);
    let before = state.clone();
    assert_eq!(
        apply(&mut state, strike(a, x)),
        Err(RuleError::TargetNotOnBoard)
    );
    assert_eq!(state, before);
}

#[test]
fn successive_kills_reach_the_discard_in_the_order_of_death_not_of_id() {
    let mut state = skirmish(stats(1, 3, 1, 3), stats(1, 1, 1, 3), 2, 8);
    let ones = state.board(PlayerId::One);
    let twos = state.board(PlayerId::Two);
    // Kill the creature of the higher id first.
    let (first, second) = (twos[0].max(twos[1]), twos[0].min(twos[1]));
    apply(&mut state, strike(ones[0], first)).unwrap();
    assert_eq!(state.active(), PlayerId::Two);
    play_first(&mut state);
    assert_eq!(state.active(), PlayerId::One);
    let events = apply(&mut state, strike(ones[1], second)).unwrap();
    assert!(events.contains(&Event::Died {
        player: PlayerId::Two,
        creature: second
    }));
    assert_eq!(state.discard(PlayerId::Two), vec![first, second]);
    assert!(first > second, "the order of death is not the order of ids");
    assert!(
        !state.board(PlayerId::Two).contains(&first)
            && !state.board(PlayerId::Two).contains(&second)
    );
}

#[test]
fn excess_damage_is_lost_and_never_reaches_the_hero() {
    // Force 5 against 1 health.
    let mut state = skirmish(stats(1, 5, 1, 3), stats(1, 1, 1, 1), 1, 8);
    let (a, x) = duelists(&state);
    let events = apply(&mut state, strike(a, x)).unwrap();
    assert_eq!(state.hero_health(PlayerId::Two), 10);
    assert_eq!(state.winner(), None);
    assert_eq!(state.discard(PlayerId::Two), vec![x]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Damaged { .. } | Event::GameWon { .. })),
        "{events:?}"
    );
    assert!(events.contains(&Event::CreatureDamaged {
        player: PlayerId::Two,
        creature: x,
        amount: 5
    }));
}

#[test]
fn a_creature_attack_never_ends_the_game_by_hero_down() {
    // Even against a hero at 1 health: only a hero-targeted attack can bring it down.
    let mut s = setup(
        hero(2, 1, 8, vec![draw(1, 1)]),
        hero(1, 1, 8, vec![draw(1, 1)]),
    );
    s.one.cards = vec![stats(1, 9, 1, 3); 30];
    s.two.cards = vec![stats(1, 1, 1, 1); 30];
    s.two.hero.health = 1;
    let (mut state, _) = new_game(s, 7).unwrap();
    play_first(&mut state);
    play_first(&mut state);
    let (a, x) = duelists(&state);
    apply(&mut state, strike(a, x)).unwrap();
    assert_eq!(state.hero_health(PlayerId::Two), 1);
    assert_eq!(state.winner(), None);
}

#[test]
fn a_non_lethal_creature_attack_logs_in_order_with_and_without_handover() {
    // 4 time units, force 2 against 5 health: attack cost 2 stays, attack cost 4 crosses.
    let log = |cost: u32| {
        let mut state = skirmish(stats(1, 2, cost, 3), stats(4, 1, 1, 5), 1, 8);
        let (a, x) = duelists(&state);
        (apply(&mut state, strike(a, x)).unwrap(), a, x)
    };
    let (events, a, x) = log(2);
    assert_eq!(
        events,
        vec![
            Event::Attacked {
                player: PlayerId::One,
                attacker: a,
                cost: 2
            },
            Event::CreatureDamaged {
                player: PlayerId::Two,
                creature: x,
                amount: 2
            },
            Event::TokenMoved {
                player: PlayerId::One,
                budget: 2
            },
        ]
    );
    let (events, a, x) = log(4);
    assert_eq!(
        events,
        vec![
            Event::Attacked {
                player: PlayerId::One,
                attacker: a,
                cost: 4
            },
            Event::CreatureDamaged {
                player: PlayerId::Two,
                creature: x,
                amount: 2
            },
            Event::TokenMoved {
                player: PlayerId::Two,
                budget: 1
            },
            Event::ActivePlayerChanged {
                player: PlayerId::Two
            },
        ]
    );
}

#[test]
fn a_lethal_creature_attack_logs_the_death_before_the_token_with_and_without_handover() {
    let log = |cost: u32| {
        let mut state = skirmish(stats(1, 5, cost, 3), stats(4, 1, 1, 3), 1, 8);
        let (a, x) = duelists(&state);
        (apply(&mut state, strike(a, x)).unwrap(), a, x)
    };
    let (events, a, x) = log(2);
    assert_eq!(
        events,
        vec![
            Event::Attacked {
                player: PlayerId::One,
                attacker: a,
                cost: 2
            },
            Event::CreatureDamaged {
                player: PlayerId::Two,
                creature: x,
                amount: 5
            },
            Event::Died {
                player: PlayerId::Two,
                creature: x
            },
            Event::TokenMoved {
                player: PlayerId::One,
                budget: 2
            },
        ]
    );
    let (events, a, x) = log(4);
    assert_eq!(
        kinds(&events),
        [
            "Attacked",
            "CreatureDamaged",
            "Died",
            "TokenMoved",
            "ActivePlayerChanged"
        ]
    );
    assert_eq!(
        events[2],
        Event::Died {
            player: PlayerId::Two,
            creature: x
        }
    );
    assert_eq!(
        events[0],
        Event::Attacked {
            player: PlayerId::One,
            attacker: a,
            cost: 4
        }
    );
}

#[test]
fn a_creature_attack_follows_the_same_time_rule_as_a_hero_attack() {
    // One holds 5 time units. For each cost: (active player, budget) after the attack.
    let cases = [
        (2, PlayerId::One, 3),
        (5, PlayerId::Two, 1),
        (7, PlayerId::Two, 3),
    ];
    for (cost, active, budget) in cases {
        for on_hero in [true, false] {
            let mut state = skirmish(stats(1, 1, cost, 3), stats(5, 1, 1, 100), 1, 8);
            let (a, x) = duelists(&state);
            let target = if on_hero {
                Target::Hero
            } else {
                Target::Creature(x)
            };
            apply(
                &mut state,
                Action::Attack {
                    attacker: a,
                    target,
                },
            )
            .unwrap();
            assert_eq!(
                (state.active(), state.budget()),
                (active, budget),
                "cost {cost}, on the hero: {on_hero}"
            );
        }
    }
    // The receiver's max_time caps the budget, and a cost above the attacker's own max_time
    // is legal.
    let mut state = skirmish(stats(1, 1, 30, 3), stats(5, 1, 1, 100), 1, 5);
    let (a, x) = duelists(&state);
    apply(&mut state, strike(a, x)).unwrap();
    assert_eq!((state.active(), state.budget()), (PlayerId::Two, 5));
}

#[test]
fn a_target_that_is_not_an_opposing_creature_in_play_is_refused_atomically() {
    // 4 time units at 1 per attack: One kills one creature and stays active.
    let mut state = skirmish(stats(1, 3, 1, 3), stats(4, 1, 1, 3), 1, 8);
    let (a, x) = duelists(&state);
    apply(&mut state, strike(a, x)).unwrap();
    assert_eq!(state.active(), PlayerId::One);
    assert_eq!(state.discard(PlayerId::Two), vec![x]);
    let in_pile = (30..60)
        .map(CardInstanceId)
        .find(|c| {
            !state.hand(PlayerId::Two).contains(c)
                && !state.board(PlayerId::Two).contains(c)
                && !state.discard(PlayerId::Two).contains(c)
        })
        .unwrap();
    let cases = [
        (a, "one's own creature"),
        (state.hand(PlayerId::One)[0], "a card in one's own hand"),
        (
            state.hand(PlayerId::Two)[0],
            "a card in the opponent's hand",
        ),
        (in_pile, "a card in the opponent's pile"),
        (x, "a card in the opponent's discard"),
        (CardInstanceId(9999), "an unknown id"),
    ];
    for (target, why) in cases {
        let before = state.clone();
        assert_eq!(
            apply(&mut state, strike(a, target)),
            Err(RuleError::TargetNotOnBoard),
            "{why}"
        );
        assert_eq!(state, before, "{why}");
    }
}

#[test]
fn the_attacker_is_checked_before_the_target_and_game_over_before_both() {
    let mut state = skirmish(stats(1, 3, 1, 3), stats(4, 1, 1, 3), 1, 8);
    let (a, x) = duelists(&state);
    let absent = CardInstanceId(9999);
    let their_hand = state.hand(PlayerId::Two)[0];
    for (attacker, target, why) in [
        (absent, absent, "both unknown"),
        (x, absent, "the opponent's creature as attacker"),
        (their_hand, a, "a card of the opponent's hand as attacker"),
    ] {
        let before = state.clone();
        assert_eq!(
            apply(&mut state, strike(attacker, target)),
            Err(RuleError::AttackerNotOnBoard),
            "{why}"
        );
        assert_eq!(state, before, "{why}");
    }
    let before = state.clone();
    assert_eq!(
        apply(&mut state, strike(a, absent)),
        Err(RuleError::TargetNotOnBoard),
        "a valid attacker reaches the target check"
    );
    assert_eq!(state, before);
    apply(&mut state, Action::Concede).unwrap();
    let after = state.clone();
    assert_eq!(
        apply(&mut state, strike(absent, absent)),
        Err(RuleError::GameOver)
    );
    assert_eq!(state, after);
}

#[test]
fn creature_targets_travel_as_json_and_hostile_forms_are_rejected() {
    let action = strike(CardInstanceId(3), CardInstanceId(7));
    let json = serde_json::to_string(&action).unwrap();
    assert_eq!(json, r#"{"attack":{"attacker":3,"target":{"creature":7}}}"#);
    assert_eq!(serde_json::from_str::<Action>(&json).unwrap(), action);
    assert_eq!(
        serde_json::from_str::<Action>(r#"{"attack":{"attacker":3,"target":"hero"}}"#).unwrap(),
        hit(CardInstanceId(3))
    );
    for hostile in [
        r#"{"creature":-1}"#,
        "4294967296",
        r#""7""#,
        "1.5",
        "null",
        r#"{"creature":7,"x":1}"#,
        r#"{"Creature":7}"#,
        r#"{"creature":"7"}"#,
        r#"{"creature":null}"#,
        r#"{"creature":7.5}"#,
        r#"{"creature":4294967296}"#,
        r#"{"creature":{"id":7}}"#,
        "{}",
    ] {
        let action = format!(r#"{{"attack":{{"attacker":3,"target":{hostile}}}}}"#);
        assert!(
            serde_json::from_str::<Action>(&action).is_err(),
            "{hostile} must be rejected"
        );
    }
}

#[test]
fn the_new_events_and_error_travel_with_stable_shapes() {
    assert_eq!(
        serde_json::to_string(&Event::CreatureDamaged {
            player: PlayerId::Two,
            creature: CardInstanceId(7),
            amount: 3
        })
        .unwrap(),
        r#"{"creature_damaged":{"player":"Two","creature":7,"amount":3}}"#
    );
    assert_eq!(
        serde_json::to_string(&Event::Died {
            player: PlayerId::Two,
            creature: CardInstanceId(7)
        })
        .unwrap(),
        r#"{"died":{"player":"Two","creature":7}}"#
    );
    assert_eq!(RuleError::TargetNotOnBoard.code(), "target_not_on_board");
    assert!(!RuleError::TargetNotOnBoard.to_string().is_empty());
}

// ---- Targets in legal_actions: exhaustion and termination ------------------------------------

/// Both heroes have no effect and a single card in hand (the rest stays in the pile, out of
/// reach), every card being `creature`; One (initiative 2, 1 time unit) starts.
fn lone_card_duel(creature: CreatureStats) -> GameState {
    let lone = |initiative| {
        let mut h = hero(initiative, 1, 8, vec![]);
        h.starting_hand = 1;
        h
    };
    let mut s = setup(lone(2), lone(1));
    s.one.cards = vec![creature.clone(); 30];
    s.two.cards = vec![creature; 30];
    new_game(s, 7).unwrap().0
}

#[test]
fn a_player_with_a_living_creature_is_never_exhausted_even_without_hand_or_effect() {
    let mut state = lone_card_duel(stats(1, 1, 1, 3));
    // Each side summons its only card, then has nothing but its creature to act with.
    play_first(&mut state);
    play_first(&mut state);
    let mut hits = 0;
    while state.winner().is_none() {
        assert!(state.hand(state.active()).is_empty());
        let actions = legal_actions(&state);
        assert!(
            actions.iter().any(|a| matches!(a, Action::Attack { .. })),
            "a creature in play is always an action"
        );
        // The hero is the first target of the first attacker: the creatures all survive.
        apply(&mut state, actions[0]).unwrap();
        hits += 1;
        assert!(hits < 100, "the game does not end");
    }
    assert!(hits > 1, "several attacks were needed ({hits})");
    assert_eq!(state.winner().map(|(_, r)| r), Some(WinReason::HeroDown));
}

#[test]
fn a_player_is_exhausted_after_the_death_of_its_last_creature() {
    // Force 5 against 1 health: any creature dies to any hit. One's attack (1 of 1) crosses
    // the budget, so the token reaches Two, who then has no card, no effect and no creature.
    let mut state = lone_card_duel(stats(1, 5, 1, 1));
    play_first(&mut state);
    play_first(&mut state);
    assert_eq!(state.active(), PlayerId::One);
    let (a, x) = duelists(&state);
    let events = apply(&mut state, strike(a, x)).unwrap();
    assert_eq!(state.discard(PlayerId::Two), vec![x]);
    assert_eq!(state.winner(), Some((PlayerId::One, WinReason::Exhaustion)));
    assert_eq!(
        kinds(&events),
        [
            "Attacked",
            "CreatureDamaged",
            "Died",
            "TokenMoved",
            "ActivePlayerChanged",
            "GameWon"
        ]
    );
    assert!(legal_actions(&state).is_empty());
    assert_eq!(
        apply(&mut state, Action::Concede),
        Err(RuleError::GameOver),
        "a finished game accepts nothing"
    );
}

/// Plays a game choosing uniformly among `legal_actions` other than `Concede` with a policy
/// generator of its own, and returns (strikes on creatures, deaths, final state).
fn random_game(seed: u64) -> (usize, usize, GameState) {
    use rand::{RngExt, SeedableRng};
    let (mut state, _) = new_game(
        setup(
            hero(1 + (seed % 2) as u32, 3, 8, vec![draw(1, 1), draw(4, 3)]),
            hero(1, 2, 6, vec![draw(2, 2)]),
        ),
        seed,
    )
    .unwrap();
    let mut policy = rand_chacha::ChaCha8Rng::seed_from_u64(seed ^ 0xC0FFEE);
    let (mut strikes, mut deaths, mut steps) = (0, 0, 0);
    while state.winner().is_none() {
        let playable: Vec<Action> = legal_actions(&state)
            .into_iter()
            .filter(|a| *a != Action::Concede)
            .collect();
        let action = if playable.is_empty() {
            Action::Concede
        } else {
            playable[policy.random_range(0..playable.len())]
        };
        if matches!(
            action,
            Action::Attack {
                target: Target::Creature(_),
                ..
            }
        ) {
            strikes += 1;
        }
        let events = apply(&mut state, action).unwrap();
        deaths += events
            .iter()
            .filter(|e| matches!(e, Event::Died { .. }))
            .count();
        steps += 1;
        assert!(steps < 2000, "seed {seed}: the game does not end");
    }
    (strikes, deaths, state)
}

#[test]
fn random_games_with_creature_targets_end_without_a_concession_and_are_reproducible() {
    let (mut strikes, mut deaths) = (0, 0);
    for seed in 0..200 {
        let (s, d, state) = random_game(seed);
        let (again_s, again_d, again) = random_game(seed);
        assert_eq!((s, d, &state), (again_s, again_d, &again), "seed {seed}");
        let (_, reason) = state.winner().unwrap();
        assert_ne!(reason, WinReason::Concede, "seed {seed}");
        strikes += s;
        deaths += d;
    }
    assert!(strikes > 200, "creatures are really attacked: {strikes}");
    assert!(deaths > 200, "creatures really die: {deaths}");
}
