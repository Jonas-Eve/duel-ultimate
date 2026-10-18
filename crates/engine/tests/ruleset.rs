//! #146: `RuleSet`, the per-game options carried by `GameSetup.rules` and recorded in
//! `Event::GameStarted`. Through the public API and the JSON form only; JSON comparisons are
//! on text, not on `Value`. "Different from the default" is always written relative to
//! `RuleSet::default()`, so the switch of the default (#127) does not rewrite these tests.

use duel_cards::{CreatureStats, Effect, Hero, HeroEffect};
use duel_engine::{
    Action, Event, GameSetup, GameState, MAX_BOARD_LIMIT, PlayerId, PlayerSetup, RuleSet,
    RuleSetError, SetupError, apply, legal_actions, new_game,
};

fn hero(initiative: u32, start_budget: u32, starting_hand: u32) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time: 8,
        starting_hand,
        initiative,
        start_budget,
        health: 10,
        effects: vec![HeroEffect {
            name: "Piocher".into(),
            cost: 1,
            effect: Effect::Draw { count: 1 },
        }],
    }
}

fn cards() -> Vec<CreatureStats> {
    (1..=30)
        .map(|n| CreatureStats {
            cost: 1 + n % 3,
            force: 1 + n % 2,
            attack_cost: 1 + n % 4,
            health: 1 + n % 4,
            provocation: n % 7 == 0,
            riposte: n % 3,
        })
        .collect()
}

fn setup_with(versions: &[&str], rules: RuleSet) -> GameSetup {
    GameSetup {
        one: PlayerSetup {
            hero: hero(2, 3, 4),
            cards: cards(),
            card_names: Vec::new(),
        },
        two: PlayerSetup {
            hero: hero(1, 1, 4),
            cards: cards(),
            card_names: Vec::new(),
        },
        data_versions: versions.iter().map(|v| (*v).to_owned()).collect(),
        rules,
    }
}

/// A rule set that differs from the engine default in both options.
fn flipped() -> RuleSet {
    let d = RuleSet::default();
    RuleSet {
        hero_riposte: !d.hero_riposte,
        board_limit: if d.board_limit == Some(5) {
            None
        } else {
            Some(5)
        },
    }
}

/// The text a rule set must serialize to: complete, in declaration order.
fn rules_text(rules: RuleSet) -> String {
    let limit = rules
        .board_limit
        .map_or("null".to_owned(), |n| n.to_string());
    format!(
        r#"{{"hero_riposte":{},"board_limit":{limit}}}"#,
        rules.hero_riposte
    )
}

fn started_json(events: &[Event]) -> String {
    serde_json::to_string(&events[0]).unwrap()
}

// ---- The type ---------------------------------------------------------------------------------

#[test]
fn the_default_is_the_standard_rules() {
    assert_eq!(RuleSet::default(), RuleSet::standard());
    assert_ne!(RuleSet::legacy(), RuleSet::default());
    assert!(RuleSet::default().is_default());
    assert!(RuleSet::standard().is_default());
    assert!(!RuleSet::legacy().is_default());
    assert!(!flipped().is_default());
}

#[test]
fn standard_is_hero_riposte_on_and_board_limit_five() {
    assert_eq!(
        RuleSet::standard(),
        RuleSet {
            hero_riposte: true,
            board_limit: Some(5)
        }
    );
}

#[test]
fn the_board_limit_bound_is_the_deck_size() {
    assert_eq!(MAX_BOARD_LIMIT, 30);
}

#[test]
fn a_rule_set_serializes_complete_in_declaration_order() {
    let d = RuleSet::default();
    for rules in [
        d,
        flipped(),
        RuleSet {
            hero_riposte: true,
            board_limit: Some(5),
        },
        RuleSet {
            hero_riposte: false,
            board_limit: Some(30),
        },
        RuleSet {
            hero_riposte: true,
            board_limit: None,
        },
    ] {
        assert_eq!(
            serde_json::to_string(&rules).unwrap(),
            rules_text(rules),
            "{rules:?}"
        );
    }
    assert_eq!(
        serde_json::to_string(&RuleSet::legacy()).unwrap(),
        r#"{"hero_riposte":false,"board_limit":null}"#
    );
}

#[test]
fn a_rule_set_reads_complete_or_partial_objects() {
    let d = RuleSet::default();
    let read = |text: &str| serde_json::from_str::<RuleSet>(text).unwrap();
    assert_eq!(read("{}"), d);
    // An explicit null is how "no limit" is written, whatever the default is.
    assert_eq!(
        read(r#"{"board_limit":null}"#),
        RuleSet {
            board_limit: None,
            ..d
        }
    );
    // An absent field takes the default of the engine at that time.
    assert_eq!(
        read(r#"{"hero_riposte":true}"#),
        RuleSet {
            hero_riposte: true,
            ..d
        }
    );
    assert_eq!(
        read(r#"{"board_limit":7}"#),
        RuleSet {
            board_limit: Some(7),
            ..d
        }
    );
    assert_eq!(
        read(r#"{"hero_riposte":true,"board_limit":5}"#),
        RuleSet {
            hero_riposte: true,
            board_limit: Some(5)
        }
    );
}

#[test]
fn a_rule_set_refuses_unknown_fields_and_wrong_types() {
    for bad in [
        r#"{"heroRiposte":true}"#,
        r#"{"hero_riposte":true,"extra":1}"#,
        r#"{"hero_riposte":"yes"}"#,
        r#"{"hero_riposte":1}"#,
        r#"{"hero_riposte":null}"#,
        r#"{"board_limit":-1}"#,
        r#"{"board_limit":2.5}"#,
        r#"{"board_limit":"5"}"#,
        r#"{"board_limit":4294967296}"#,
        r#"null"#,
    ] {
        assert!(serde_json::from_str::<RuleSet>(bad).is_err(), "{bad}");
    }
}

#[test]
fn validate_bounds_the_board_limit_to_one_through_the_deck_size() {
    let with = |board_limit| RuleSet {
        board_limit,
        ..RuleSet::default()
    };
    for ok in [None, Some(1), Some(5), Some(30)] {
        assert_eq!(with(ok).validate(), Ok(()), "{ok:?}");
    }
    for value in [0, 31, 1000, u32::MAX] {
        assert_eq!(
            with(Some(value)).validate(),
            Err(RuleSetError::BoardLimitOutOfRange { value, max: 30 }),
            "{value}"
        );
    }
    let message = with(Some(0)).validate().unwrap_err().to_string();
    assert!(message.contains("board_limit"), "{message}");
}

// ---- GameSetup --------------------------------------------------------------------------------

#[test]
fn a_setup_without_rules_reads_as_default_and_reserializes_without_the_key() {
    let default_setup = setup_with(&["fixture@1.0.0"], RuleSet::default());
    let text = serde_json::to_string(&default_setup).unwrap();
    assert!(!text.contains("rules"), "{text}");
    let back: GameSetup = serde_json::from_str(&text).unwrap();
    assert_eq!(back.rules, RuleSet::default());
    assert_eq!(back, default_setup);
    assert_eq!(serde_json::to_string(&back).unwrap(), text);
}

#[test]
fn a_setup_with_default_rules_serializes_byte_for_byte_as_before() {
    // Before #146 a setup was exactly `one`, `two`, `data_versions`, in this order.
    #[derive(serde::Serialize)]
    struct BeforeRules<'a> {
        one: &'a PlayerSetup,
        two: &'a PlayerSetup,
        data_versions: &'a [String],
    }
    let setup = setup_with(&["a@1"], RuleSet::default());
    let before = BeforeRules {
        one: &setup.one,
        two: &setup.two,
        data_versions: &setup.data_versions,
    };
    assert_eq!(
        serde_json::to_string(&setup).unwrap(),
        serde_json::to_string(&before).unwrap()
    );
}

#[test]
fn a_setup_with_other_rules_writes_the_complete_object_last_and_round_trips() {
    let setup = setup_with(&["a@1"], flipped());
    let text = serde_json::to_string(&setup).unwrap();
    assert!(
        text.ends_with(&format!(r#","rules":{}}}"#, rules_text(flipped()))),
        "{text}"
    );
    let back: GameSetup = serde_json::from_str(&text).unwrap();
    assert_eq!(back, setup);
    // A partial object in a setup is completed with the default.
    let mut value = serde_json::to_value(&setup).unwrap();
    value["rules"] = serde_json::json!({"hero_riposte": !RuleSet::default().hero_riposte});
    let partial: GameSetup = serde_json::from_value(value).unwrap();
    assert_eq!(
        partial.rules,
        RuleSet {
            hero_riposte: !RuleSet::default().hero_riposte,
            ..RuleSet::default()
        }
    );
}

#[test]
fn a_setup_still_refuses_unknown_fields() {
    let mut value = serde_json::to_value(setup_with(&[], RuleSet::default())).unwrap();
    value["rule"] = serde_json::json!({});
    assert!(serde_json::from_value::<GameSetup>(value).is_err());
}

// ---- new_game ---------------------------------------------------------------------------------

#[test]
fn new_game_refuses_invalid_rules_without_panicking() {
    for value in [0, 31, u32::MAX] {
        let rules = RuleSet {
            board_limit: Some(value),
            ..RuleSet::default()
        };
        let error = new_game(setup_with(&[], rules), 1).unwrap_err();
        assert_eq!(
            error,
            SetupError::InvalidRules(RuleSetError::BoardLimitOutOfRange { value, max: 30 })
        );
        assert_eq!(error.code(), "invalid_setup");
        assert!(error.to_string().contains("board_limit"), "{error}");
    }
}

#[test]
fn an_invalid_deck_wins_over_invalid_rules_with_the_message_it_had() {
    let mut bad_deck = setup_with(&[], RuleSet::default());
    bad_deck.one.cards.pop();
    let before = new_game(bad_deck.clone(), 1).unwrap_err();
    assert!(matches!(before, SetupError::Invalid(_)), "{before:?}");

    bad_deck.rules = RuleSet {
        board_limit: Some(0),
        ..RuleSet::default()
    };
    let both = new_game(bad_deck, 1).unwrap_err();
    assert_eq!(both, before);
    assert_eq!(both.to_string(), before.to_string());
}

#[test]
fn valid_non_default_rules_start_a_game() {
    for rules in [
        flipped(),
        RuleSet {
            board_limit: Some(1),
            ..RuleSet::default()
        },
        RuleSet {
            board_limit: Some(30),
            ..RuleSet::default()
        },
    ] {
        assert!(new_game(setup_with(&[], rules), 1).is_ok(), "{rules:?}");
    }
}

// ---- Event::GameStarted -----------------------------------------------------------------------

fn start(versions: &[&str], rules: RuleSet) -> (GameState, Vec<Event>) {
    new_game(setup_with(versions, rules), 7).unwrap()
}

#[test]
fn default_rules_add_no_key_to_game_started() {
    for versions in [&[][..], &["fixture@1.0.0"][..]] {
        let (_, events) = start(versions, RuleSet::default());
        assert!(!started_json(&events).contains("rules"));
    }
    // `data_versions.rs` pins the exact strings; here the shape with explicit defaults.
    let (_, events) = start(&["a@1"], RuleSet::default());
    assert!(started_json(&events).ends_with(r#","data_versions":["a@1"]}}"#));
}

#[test]
fn other_rules_add_the_complete_object_after_data_versions() {
    let (_, events) = start(&["fixture@1.0.0", "other@2"], flipped());
    let json = started_json(&events);
    assert!(
        json.ends_with(&format!(
            r#","data_versions":["fixture@1.0.0","other@2"],"rules":{}}}}}"#,
            rules_text(flipped())
        )),
        "{json}"
    );
    let versions = json.find(r#""data_versions""#).unwrap();
    let rules = json.find(r#""rules""#).unwrap();
    assert!(versions < rules, "{json}");
}

#[test]
fn the_key_is_there_with_empty_versions_too() {
    let (_, events) = start(&[], flipped());
    let Event::GameStarted { first, budget, .. } = events[0] else {
        panic!("first event is GameStarted");
    };
    let first = match first {
        PlayerId::One => "One",
        PlayerId::Two => "Two",
    };
    assert_eq!(
        started_json(&events),
        format!(
            r#"{{"game_started":{{"first":"{first}","budget":{budget},"rules":{}}}}}"#,
            rules_text(flipped())
        )
    );
}

#[test]
fn a_legacy_game_records_its_rules_and_a_standard_game_does_not() {
    let legacy_text = r#""rules":{"hero_riposte":false,"board_limit":null}"#;
    for versions in [&[][..], &["fixture@1.0.0"][..]] {
        let (_, events) = start(versions, RuleSet::legacy());
        let json = started_json(&events);
        assert!(json.ends_with(&format!("{legacy_text}}}}}")), "{json}");
        if !versions.is_empty() {
            let at = |needle: &str| json.find(needle).unwrap();
            assert!(at(r#""data_versions""#) < at(r#""rules""#), "{json}");
        }
        let (_, events) = start(versions, RuleSet::standard());
        assert!(!started_json(&events).contains("rules"));
    }
}

#[test]
fn explicit_default_rules_write_no_key() {
    let (_, explicit) = start(&["a@1"], RuleSet::default());
    let (_, implicit) = new_game(
        GameSetup {
            rules: Default::default(),
            ..setup_with(&["a@1"], flipped())
        },
        7,
    )
    .unwrap();
    assert_eq!(started_json(&explicit), started_json(&implicit));
}

#[test]
fn the_event_carries_the_whole_object_even_when_one_field_differs() {
    let d = RuleSet::default();
    for rules in [
        RuleSet {
            hero_riposte: !d.hero_riposte,
            ..d
        },
        RuleSet {
            board_limit: if d.board_limit == Some(9) {
                None
            } else {
                Some(9)
            },
            ..d
        },
    ] {
        let (_, events) = start(&[], rules);
        let json = started_json(&events);
        assert!(
            json.contains(&format!(r#""rules":{}"#, rules_text(rules))),
            "{json}"
        );
    }
}

#[test]
fn a_journal_with_or_without_the_key_reads_back_and_reserializes_identically() {
    let flipped_text = rules_text(flipped());
    for old in [
        r#"{"game_started":{"first":"One","budget":3}}"#.to_owned(),
        r#"{"game_started":{"first":"One","budget":3,"data_versions":["a"]}}"#.to_owned(),
        format!(r#"{{"game_started":{{"first":"Two","budget":1,"rules":{flipped_text}}}}}"#),
        format!(
            r#"{{"game_started":{{"first":"Two","budget":1,"data_versions":["a","b"],"rules":{flipped_text}}}}}"#
        ),
    ] {
        let event: Event = serde_json::from_str(&old).unwrap();
        assert_eq!(serde_json::to_string(&event).unwrap(), old);
    }
    // A key that spells out the default is read, and written back without the key.
    let spelled = format!(
        r#"{{"game_started":{{"first":"One","budget":3,"rules":{}}}}}"#,
        rules_text(RuleSet::default())
    );
    let event: Event = serde_json::from_str(&spelled).unwrap();
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        r#"{"game_started":{"first":"One","budget":3}}"#
    );
}

#[test]
fn the_state_exposes_the_rules_of_the_game() {
    let (state, _) = start(&[], flipped());
    assert_eq!(state.rules(), flipped());
    let (state, _) = start(&[], RuleSet::default());
    assert_eq!(state.rules(), RuleSet::default());
}

// ---- Randomness ------------------------------------------------------------------------------

/// What the generator decided at setup and during the first draws, by seat.
fn random_outcome(rules: RuleSet, seed: u64) -> (PlayerId, u32, Vec<Vec<u32>>, Vec<Event>) {
    let (mut state, events) = new_game(setup_with(&[], rules), seed).unwrap();
    let Event::GameStarted { first, budget, .. } = events[0] else {
        panic!("first event is GameStarted");
    };
    let hands = [PlayerId::One, PlayerId::Two]
        .map(|p| state.hand(p).iter().map(|c| c.0).collect())
        .to_vec();
    // The order of the piles, seen through the cards drawn by the hero effect.
    let mut drawn = Vec::new();
    for _ in 0..6 {
        let Some(action) = legal_actions(&state)
            .into_iter()
            .find(|a| matches!(a, Action::Activate { .. }))
        else {
            break;
        };
        drawn.extend(apply(&mut state, action).unwrap());
    }
    (first, budget, hands, drawn)
}

#[test]
fn no_randomness_is_consumed_by_the_rules() {
    for seed in 0..20 {
        let reference = random_outcome(RuleSet::default(), seed);
        for rules in [
            flipped(),
            RuleSet {
                hero_riposte: true,
                board_limit: Some(5),
            },
            RuleSet {
                board_limit: Some(1),
                ..RuleSet::default()
            },
        ] {
            assert_eq!(
                random_outcome(rules, seed),
                reference,
                "seed {seed} {rules:?}"
            );
        }
    }
}
