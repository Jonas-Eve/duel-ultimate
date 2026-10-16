//! S3-T1: `GameSetup.data_versions` is copied, opaque, into `Event::GameStarted`; through the
//! public API and the JSON form only. The golden journals of S1 and S2a are in `engine.rs`.

use duel_cards::{CreatureStats, Effect, Hero, HeroEffect};
use duel_engine::{
    Action, CardInstanceId, Event, GameSetup, GameState, PlayerId, PlayerSetup, apply,
    legal_actions, new_game,
};
use serde_json::{Value, json};

fn hero(initiative: u32, start_budget: u32, starting_hand: u32, with_draw: bool) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time: 8,
        starting_hand,
        initiative,
        start_budget,
        health: 10,
        effects: if with_draw {
            vec![HeroEffect {
                name: "Piocher".into(),
                cost: 1,
                effect: Effect::Draw { count: 1 },
            }]
        } else {
            vec![]
        },
    }
}

fn cards() -> Vec<CreatureStats> {
    (1..=30)
        .map(|n| CreatureStats {
            cost: 1 + n % 3,
            force: 1 + n % 2,
            attack_cost: 1 + n % 4,
            health: 1 + n % 4,
            provocation: false,
            riposte: 0,
        })
        .collect()
}

fn setup_with(versions: &[&str]) -> GameSetup {
    GameSetup {
        one: PlayerSetup {
            hero: hero(2, 3, 4, true),
            cards: cards(),
            card_names: Vec::new(),
        },
        two: PlayerSetup {
            hero: hero(1, 1, 4, true),
            cards: cards(),
            card_names: Vec::new(),
        },
        data_versions: versions.iter().map(|v| (*v).to_owned()).collect(),
    }
}

/// The `GameStarted` event of the journal, as JSON text (string comparison, not `Value`).
fn started_json(events: &[Event]) -> String {
    serde_json::to_string(&events[0]).unwrap()
}

fn start(versions: &[&str]) -> Vec<Event> {
    new_game(setup_with(versions), 7).unwrap().1
}

/// The `data_versions` a `GameStarted` event carries, read through its JSON form.
fn versions_of(event: &Event) -> Vec<String> {
    let v = serde_json::to_value(event).unwrap();
    v["game_started"]["data_versions"]
        .as_array()
        .map(|a| a.iter().map(|s| s.as_str().unwrap().to_owned()).collect())
        .unwrap_or_default()
}

#[test]
fn game_started_carries_exactly_the_list_of_the_setup() {
    let lists: [&[&str]; 5] = [
        &["fixture@1.0.0"],
        &["b@2", "a@1", "c@3"],
        &["x", "x", "x"],
        &["a", "", "b"],
        &["é@1.0.0", "数据@1", "🃏", "a\u{0}b"],
    ];
    for list in lists {
        let events = start(list);
        assert!(matches!(events[0], Event::GameStarted { .. }));
        let expected: Vec<String> = list.iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(versions_of(&events[0]), expected, "{list:?}");
    }
}

#[test]
fn an_empty_list_leaves_the_json_of_game_started_untouched() {
    for seed in 0..10 {
        let (_, events) = new_game(setup_with(&[]), seed).unwrap();
        let Event::GameStarted { first, budget, .. } = events[0] else {
            panic!("first event is GameStarted");
        };
        let first = match first {
            PlayerId::One => "One",
            PlayerId::Two => "Two",
        };
        assert_eq!(
            started_json(&events),
            format!(r#"{{"game_started":{{"first":"{first}","budget":{budget}}}}}"#)
        );
    }
}

#[test]
fn a_non_empty_list_adds_the_key_last() {
    let events = start(&["fixture@1.0.0", "other@2"]);
    let json = started_json(&events);
    assert!(json.starts_with(r#"{"game_started":{"first":""#), "{json}");
    assert!(
        json.ends_with(r#","data_versions":["fixture@1.0.0","other@2"]}}"#),
        "{json}"
    );
    let first = json.find(r#""first""#).unwrap();
    let budget = json.find(r#""budget""#).unwrap();
    let versions = json.find(r#""data_versions""#).unwrap();
    assert!(first < budget && budget < versions, "{json}");
}

#[test]
fn an_old_journal_without_the_key_reads_back_with_an_empty_list_and_reserializes_identically() {
    let old = r#"{"game_started":{"first":"One","budget":3}}"#;
    let event: Event = serde_json::from_str(old).unwrap();
    assert_eq!(versions_of(&event), Vec::<String>::new());
    assert_eq!(serde_json::to_string(&event).unwrap(), old);
    // A journal that does carry the key round-trips too.
    let new = r#"{"game_started":{"first":"Two","budget":1,"data_versions":["a","b"]}}"#;
    let event: Event = serde_json::from_str(new).unwrap();
    assert_eq!(versions_of(&event), ["a", "b"]);
    assert_eq!(serde_json::to_string(&event).unwrap(), new);
}

#[test]
fn the_state_still_exposes_the_versions() {
    let (state, _) = new_game(setup_with(&["fixture@1.0.0", "x"]), 3).unwrap();
    assert_eq!(state.data_versions(), ["fixture@1.0.0", "x"]);
    let (state, _) = new_game(setup_with(&[]), 3).unwrap();
    assert!(state.data_versions().is_empty());
}

/// Everything observable of a state except the versions.
fn snapshot(state: &GameState) -> Value {
    let side = |p: PlayerId| {
        let ids = |v: Vec<CardInstanceId>| v.iter().map(|c| c.0).collect::<Vec<_>>();
        json!({
            "hand": ids(state.hand(p)),
            "board": ids(state.board(p)),
            "discard": ids(state.discard(p)),
            "deck": state.deck_len(p),
            "health": state.hero_health(p),
            "creatures": state.board(p).iter().map(|c| state.creature_health(*c)).collect::<Vec<_>>(),
        })
    };
    json!({
        "one": side(PlayerId::One),
        "two": side(PlayerId::Two),
        "active": state.active(),
        "budget": state.budget(),
        "winner": state.winner().map(|(p, r)| json!([p, r])),
    })
}

#[test]
fn the_versions_influence_neither_the_rng_nor_the_rules() {
    for seed in 0..20 {
        let (mut with, events_with) = new_game(setup_with(&["a@1", "b@2", "c@3"]), seed).unwrap();
        let (mut without, events_without) = new_game(setup_with(&[]), seed).unwrap();
        assert_eq!(snapshot(&with), snapshot(&without), "seed {seed}");
        assert_eq!(events_with[1..], events_without[1..], "seed {seed}");
        // A scripted game: always the last non-concede action, compared at every step.
        while with.winner().is_none() {
            let actions = legal_actions(&with);
            assert_eq!(actions, legal_actions(&without), "seed {seed}");
            let action = actions
                .into_iter()
                .rfind(|a| *a != Action::Concede)
                .unwrap_or(Action::Concede);
            let a = apply(&mut with, action).unwrap();
            let b = apply(&mut without, action).unwrap();
            assert_eq!(a, b, "seed {seed}");
            assert_eq!(snapshot(&with), snapshot(&without), "seed {seed}");
        }
        assert_eq!(without.winner(), with.winner());
    }
}

#[test]
fn exhaustion_at_setup_follows_game_started_which_keeps_its_versions() {
    let mut setup = setup_with(&["fixture@1.0.0"]);
    setup.one.hero = hero(2, 3, 0, false);
    let (state, events) = new_game(setup, 1).unwrap();
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(versions_of(&events[0]), ["fixture@1.0.0"]);
    assert!(
        matches!(events[1], Event::GameWon { .. }),
        "{:?}",
        events[1]
    );
    assert!(state.winner().is_some());
}
