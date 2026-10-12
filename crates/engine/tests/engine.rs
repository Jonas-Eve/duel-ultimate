//! Public-API tests of the engine: setup, the time rule through hero effects, concede,
//! `legal_actions`, the event log. Cases needing private state live next to `GameState`.

use duel_cards::{CardsError, CreatureStats, Effect, Hero, HeroEffect, MAX_STAT};
use duel_engine::{
    Action, CardInstanceId, Event, GameSetup, GameState, PlayerId, PlayerSetup, RuleError,
    SetupError, Target, WinReason, apply, legal_actions, new_game,
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
            Event::Played { .. } => "Played",
            Event::Attacked { .. } => "Attacked",
            Event::Damaged { .. } => "Damaged",
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
            let universe = (0..12)
                .map(|effect| Action::Activate { effect })
                .chain((0..64).map(|n| Action::Play {
                    card: CardInstanceId(n),
                }))
                .chain((0..64).map(|n| Action::Attack {
                    attacker: CardInstanceId(n),
                    target: Target::Hero,
                }))
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
    (0..30)
        .map(|_| CreatureStats {
            cost,
            force: 2,
            attack_cost: 1,
            health: 3,
        })
        .collect()
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
    (0..30)
        .map(|_| CreatureStats {
            cost: 1,
            force,
            attack_cost,
            health: 3,
        })
        .collect()
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
    let expected: Vec<Action> = state
        .hand(PlayerId::Two)
        .into_iter()
        .map(|card| Action::Play { card })
        .chain([Action::Activate { effect: 0 }])
        .chain(two_board.iter().map(|c| hit(*c)))
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

/// Plays one game by always taking the last action other than `Concede` (an attack as soon as
/// a creature is in play).
fn play_to_the_end(seed: u64) -> (GameState, Vec<Event>) {
    let (mut state, mut log) = new_game(
        setup(
            hero(2, 3, 8, vec![draw(1, 1), draw(4, 2)]),
            hero(1, 2, 6, vec![draw(2, 1)]),
        ),
        seed,
    )
    .unwrap();
    while state.winner().is_none() {
        let action = legal_actions(&state)
            .into_iter()
            .rfind(|a| *a != Action::Concede)
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

/// Like `play_to_the_end`, but the policy takes the last action other than `Concede` whose
/// target is not a creature: a game in which only heroes are ever attacked.
fn hero_only_game(seed: u64) -> (GameState, Vec<Event>) {
    let (mut state, mut log) = new_game(
        setup(
            hero(2, 3, 8, vec![draw(1, 1), draw(4, 2)]),
            hero(1, 2, 6, vec![draw(2, 1)]),
        ),
        seed,
    )
    .unwrap();
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
            let (state, log) = hero_only_game(seed);
            let (winner, reason) = state.winner().unwrap();
            let json = serde_json::to_string(&log).unwrap();
            (winner, reason, log.len(), fnv1a(json.as_bytes()))
        })
        .collect();
    assert_eq!(observed, S1_GOLDEN);
}
