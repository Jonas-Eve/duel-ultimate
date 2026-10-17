//! Card names carried by a game (S2-T1 of #52): optional, parallel to `cards`, bounded, and
//! with no effect on the rules, the generator or the events. How a name follows its card
//! through the zones is tested inside the crate (`setup.rs`), where the zones are visible.

use duel_cards::{CardsError, MAX_NAME_LEN, parse_cards, parse_decks};
use duel_engine::{
    Action, GameSetup, PlayerSetup, RuleSet, SetupError, apply, legal_actions, new_game,
    player_setup,
};

const STARTER_CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const STARTER_DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");

fn side_of(index: usize) -> PlayerSetup {
    let cards = parse_cards(STARTER_CARDS).unwrap();
    let decks = parse_decks(STARTER_DECKS).unwrap();
    player_setup(&decks[index], &cards).unwrap()
}

fn side() -> PlayerSetup {
    side_of(0)
}

fn named_setup() -> GameSetup {
    GameSetup {
        one: side_of(0),
        two: side_of(1),
        data_versions: vec![],
        rules: RuleSet::default(),
    }
}

#[test]
fn a_side_without_names_serializes_as_before() {
    let mut one = side();
    one.card_names.clear();
    one.cards.clear();
    let json = serde_json::to_string(&one).unwrap();
    assert!(!json.contains("card_names"), "{json}");
    assert!(json.ends_with(r#""cards":[]}"#), "{json}");
    assert!(json.starts_with(r#"{"hero":{"name":"#), "{json}");
}

#[test]
fn a_side_with_names_serializes_them_after_the_cards() {
    let json = serde_json::to_string(&side()).unwrap();
    assert!(json.contains(r#""card_names":["#), "{json}");
    assert!(
        json.find(r#""cards":"#) < json.find(r#""card_names":"#),
        "{json}"
    );
}

#[test]
fn a_setup_json_without_card_names_is_accepted_and_has_none() {
    let mut value = serde_json::to_value(named_setup()).unwrap();
    for seat in ["one", "two"] {
        value[seat].as_object_mut().unwrap().remove("card_names");
    }
    let setup: GameSetup = serde_json::from_value(value).unwrap();
    assert!(setup.one.card_names.is_empty() && setup.two.card_names.is_empty());
    assert!(new_game(setup, 1).is_ok());
}

#[test]
fn a_setup_json_round_trips_its_names() {
    let setup = named_setup();
    let back: GameSetup = serde_json::from_str(&serde_json::to_string(&setup).unwrap()).unwrap();
    assert_eq!(back, setup);
    assert_eq!(back.one.card_names.len(), 30);
}

fn refused(setup: GameSetup) -> CardsError {
    match new_game(setup, 1) {
        Err(SetupError::Invalid(error)) => error,
        Err(other) => panic!("expected a card error, got {other}"),
        Ok(_) => panic!("the setup should have been refused"),
    }
}

#[test]
fn names_of_another_length_are_refused_with_invalid_setup() {
    for len in [1, 29, 31] {
        let mut setup = named_setup();
        setup.two.card_names = (0..len).map(|i| format!("n{i}")).collect();
        assert_eq!(
            refused(setup),
            CardsError::CardNamesMismatch {
                names: len,
                cards: 30
            }
        );
    }
    let mut setup = named_setup();
    setup.one.card_names.truncate(29);
    let error = new_game(setup, 1).unwrap_err();
    assert_eq!(error.code(), "invalid_setup");
}

#[test]
fn a_name_of_64_characters_is_accepted_and_one_of_65_is_refused() {
    let mut setup = named_setup();
    setup.one.card_names[7] = "x".repeat(MAX_NAME_LEN);
    setup.two.card_names[29] = "é".repeat(MAX_NAME_LEN);
    assert!(new_game(setup, 1).is_ok());
    for seat_one in [true, false] {
        let mut setup = named_setup();
        let side = if seat_one {
            &mut setup.one
        } else {
            &mut setup.two
        };
        side.card_names[0] = "x".repeat(MAX_NAME_LEN + 1);
        assert_eq!(
            refused(setup),
            CardsError::NameTooLong {
                len: MAX_NAME_LEN + 1,
                max: MAX_NAME_LEN
            }
        );
    }
}

#[test]
fn the_deck_size_error_comes_before_the_names_error() {
    let mut setup = named_setup();
    setup.one.cards.pop(); // 29 cards, 30 names
    assert_eq!(
        refused(setup),
        CardsError::WrongDeckSize {
            found: 29,
            expected: 30
        }
    );
}

/// Plays a whole game with a seed-driven pick among the legal actions and returns every
/// serialized event of the game (the setup's included) and every list of legal actions.
fn journal(setup: GameSetup, seed: u64) -> (Vec<String>, Vec<String>) {
    let (mut state, events) = new_game(setup, seed).unwrap();
    let mut log: Vec<String> = events
        .iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect();
    let mut legal = Vec::new();
    let mut pick = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    for _ in 0..5_000 {
        let actions = legal_actions(&state);
        if actions.is_empty() {
            return (log, legal);
        }
        legal.push(serde_json::to_string(&actions).unwrap());
        pick = pick
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let playable: Vec<&Action> = actions.iter().filter(|a| **a != Action::Concede).collect();
        let action = playable
            .get(usize::try_from(pick >> 33).unwrap() % playable.len().max(1))
            .map_or(Action::Concede, |a| **a);
        let events = apply(&mut state, action).unwrap();
        log.extend(events.iter().map(|e| serde_json::to_string(e).unwrap()));
    }
    panic!("seed {seed}: the game did not end");
}

#[test]
fn names_change_no_event_and_no_legal_action_over_100_whole_games() {
    let mut longest = 0;
    for seed in 0..100 {
        let with = named_setup();
        let mut without = with.clone();
        without.one.card_names.clear();
        without.two.card_names.clear();
        assert_ne!(with, without);
        let (events_with, legal_with) = journal(with, seed);
        let (events_without, legal_without) = journal(without, seed);
        assert_eq!(events_with, events_without, "seed {seed}");
        assert_eq!(legal_with, legal_without, "seed {seed}");
        longest = longest.max(events_with.len());
    }
    assert!(longest > 20, "the games are real games ({longest} events)");
}
