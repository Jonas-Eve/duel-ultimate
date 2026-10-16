//! `Duel::from_data` (what `Game.from_data` runs): a game built from `cards.json` and
//! `decks.json` strings, decks chosen by identifier (S2-T1 of #52). Equivalent to
//! `Duel::from_json` on the setup built by hand, with stable error codes and a 1 MiB ceiling
//! on each of the five strings.

use duel_cards::{parse_cards, parse_decks, resolve_deck};
use duel_engine::{GameSetup, PlayerSetup};
use duel_engine_wasm::{Duel, MAX_INPUT_BYTES};
use serde_json::Value;

const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");
const STARTER_CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const STARTER_DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");
const VERSIONS: &str = r#"["fixture@1.0.0"]"#;

fn code(err: &str) -> String {
    let v: Value = serde_json::from_str(err).expect("errors are JSON");
    assert!(
        v["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{err}"
    );
    v["code"].as_str().expect("a code").to_owned()
}

fn from_data(
    cards: &str,
    decks: &str,
    one: &str,
    two: &str,
    versions: &str,
) -> Result<Duel, String> {
    Duel::from_data(cards, decks, one, two, versions, 1)
}

/// The setup built by hand from `duel-cards` (no help from the code under test), as JSON.
fn by_hand(cards: &str, decks: &str, one: &str, two: &str, versions: &[&str]) -> String {
    let cards = parse_cards(cards).unwrap();
    let decks = parse_decks(decks).unwrap();
    let side = |id: &str| {
        let deck = decks.iter().find(|d| d.id == id).unwrap();
        PlayerSetup {
            hero: deck.hero.clone(),
            cards: resolve_deck(deck, &cards).unwrap(),
            card_names: Vec::new(),
        }
    };
    serde_json::to_string(&GameSetup {
        one: side(one),
        two: side(two),
        data_versions: versions.iter().map(|v| (*v).to_owned()).collect(),
    })
    .unwrap()
}

#[test]
fn from_data_equals_from_json_on_the_setup_built_by_hand() {
    for (cards, decks, a, b, version) in [
        (
            FIXTURE_CARDS,
            FIXTURE_DECKS,
            "aggro",
            "control",
            "fixture@1.0.0",
        ),
        (
            STARTER_CARDS,
            STARTER_DECKS,
            "rush",
            "bulwark",
            "starter@1.0.0",
        ),
    ] {
        let versions = serde_json::to_string(&[version]).unwrap();
        for (one, two) in [(a, b), (b, a), (a, a), (b, b)] {
            for seed in [0, 1, 7, 12345, u32::MAX] {
                let data = Duel::from_data(cards, decks, one, two, &versions, seed).unwrap();
                let json =
                    Duel::from_json(&by_hand(cards, decks, one, two, &[version]), seed).unwrap();
                let context = format!("{one} vs {two}, seed {seed}");
                assert_eq!(
                    data.start_events_json().unwrap(),
                    json.start_events_json().unwrap(),
                    "{context}"
                );
                assert_eq!(
                    data.legal_actions_json().unwrap(),
                    json.legal_actions_json().unwrap(),
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn the_decks_are_chosen_by_identifier_not_by_position_in_the_file() {
    let mut reordered: Value = serde_json::from_str(FIXTURE_DECKS).unwrap();
    reordered.as_array_mut().unwrap().reverse();
    let reordered = reordered.to_string();
    for seed in [1, 2, 3] {
        for (one, two) in [("aggro", "control"), ("control", "aggro")] {
            let a =
                Duel::from_data(FIXTURE_CARDS, FIXTURE_DECKS, one, two, VERSIONS, seed).unwrap();
            let b = Duel::from_data(FIXTURE_CARDS, &reordered, one, two, VERSIONS, seed).unwrap();
            assert_eq!(
                a.start_events_json().unwrap(),
                b.start_events_json().unwrap()
            );
            assert_eq!(
                a.legal_actions_json().unwrap(),
                b.legal_actions_json().unwrap()
            );
        }
    }
    // The two seats really differ (aggro and control have different heroes), so a swapped seat
    // would be seen: the opening hands, hence the legal actions, are not the same.
    let one_first = from_data(FIXTURE_CARDS, FIXTURE_DECKS, "aggro", "control", VERSIONS).unwrap();
    let two_first = from_data(FIXTURE_CARDS, FIXTURE_DECKS, "control", "aggro", VERSIONS).unwrap();
    assert_ne!(
        one_first.start_events_json().unwrap(),
        two_first.start_events_json().unwrap()
    );
}

#[test]
fn the_versions_reach_game_started_and_an_empty_list_leaves_the_key_out() {
    let game = from_data(
        FIXTURE_CARDS,
        FIXTURE_DECKS,
        "aggro",
        "control",
        r#"["fixture@1.0.0","extra@2"]"#,
    )
    .unwrap();
    let events: Value = serde_json::from_str(&game.start_events_json().unwrap()).unwrap();
    assert_eq!(
        events[0]["game_started"]["data_versions"],
        serde_json::json!(["fixture@1.0.0", "extra@2"])
    );
    let empty = from_data(FIXTURE_CARDS, FIXTURE_DECKS, "aggro", "control", "[]").unwrap();
    let json = empty.start_events_json().unwrap();
    assert!(!json.contains("data_versions"), "{json}");
}

#[test]
fn an_unknown_deck_has_its_own_code() {
    for (one, two) in [
        ("nope", "control"),
        ("aggro", "nope"),
        ("AGGRO", "control"),
        ("", ""),
    ] {
        let err = from_data(FIXTURE_CARDS, FIXTURE_DECKS, one, two, VERSIONS)
            .err()
            .unwrap();
        assert_eq!(code(&err), "unknown_deck", "{one:?} {two:?}");
    }
}

#[test]
fn malformed_data_is_invalid_json_and_invalid_data_is_invalid_setup() {
    let ok = |cards: &str, decks: &str, versions: &str| {
        from_data(cards, decks, "aggro", "control", versions)
    };
    for (cards, decks, versions) in [
        ("{", FIXTURE_DECKS, VERSIONS),
        ("", FIXTURE_DECKS, VERSIONS),
        (FIXTURE_CARDS, "[", VERSIONS),
        (FIXTURE_CARDS, FIXTURE_DECKS, "["),
        (FIXTURE_CARDS, FIXTURE_DECKS, ""),
        (FIXTURE_CARDS, FIXTURE_DECKS, r#"{"a":1}"#),
        (FIXTURE_CARDS, FIXTURE_DECKS, "[1]"),
        (FIXTURE_CARDS, FIXTURE_DECKS, r#""fixture""#),
    ] {
        let err = ok(cards, decks, versions).err().unwrap();
        assert_eq!(
            code(&err),
            "invalid_json",
            "{cards:.20} {decks:.20} {versions}"
        );
    }
    let unknown_card = FIXTURE_DECKS.replacen("\"goblin\"", "\"ghost\"", 1);
    assert_ne!(unknown_card, FIXTURE_DECKS);
    let err = ok(FIXTURE_CARDS, &unknown_card, VERSIONS).err().unwrap();
    assert_eq!(code(&err), "invalid_setup");
    assert!(err.contains("ghost"), "{err}");
    let short_deck = FIXTURE_DECKS.replacen("\"goblin\",", "", 1);
    assert_ne!(short_deck, FIXTURE_DECKS);
    assert_eq!(
        code(&ok(FIXTURE_CARDS, &short_deck, VERSIONS).err().unwrap()),
        "invalid_setup"
    );
}

/// Calls `from_data` with the five strings, one of them replaced by `value`.
fn with_string(index: usize, value: &str) -> Result<Duel, String> {
    let mut args = [FIXTURE_CARDS, FIXTURE_DECKS, "aggro", "control", VERSIONS];
    args[index] = value;
    from_data(args[0], args[1], args[2], args[3], args[4])
}

#[test]
fn each_of_the_five_strings_is_bounded_before_any_parsing() {
    let huge = " ".repeat(MAX_INPUT_BYTES + 1);
    for index in 0..5 {
        let err = with_string(index, &huge).err().unwrap();
        assert_eq!(code(&err), "input_too_large", "string #{index}");
    }
    // Non-whitespace deck identifiers too (not only JSON).
    let long_id = "a".repeat(MAX_INPUT_BYTES + 1);
    assert_eq!(
        code(&with_string(2, &long_id).err().unwrap()),
        "input_too_large"
    );
    assert_eq!(
        code(&with_string(3, &long_id).err().unwrap()),
        "input_too_large"
    );
}

#[test]
fn the_bound_is_checked_on_all_five_before_the_first_is_parsed() {
    // A broken `cards_json` would be `invalid_json`; the oversized last string wins.
    let huge = "a".repeat(MAX_INPUT_BYTES + 1);
    let err = from_data("{", FIXTURE_DECKS, "aggro", &huge, VERSIONS)
        .err()
        .unwrap();
    assert_eq!(code(&err), "input_too_large");
    let err = from_data("{", "[", "nope", "nope", &huge).err().unwrap();
    assert_eq!(code(&err), "input_too_large");
}

#[test]
fn exactly_at_the_limit_it_is_the_analysis_that_answers() {
    let spaces = " ".repeat(MAX_INPUT_BYTES);
    for index in [0, 1, 4] {
        let err = with_string(index, &spaces).err().unwrap();
        assert_eq!(code(&err), "invalid_json", "string #{index}");
    }
    let id = "a".repeat(MAX_INPUT_BYTES);
    for index in [2, 3] {
        let err = with_string(index, &id).err().unwrap();
        assert_eq!(code(&err), "unknown_deck", "string #{index}");
    }
}

#[test]
fn the_largest_seed_is_accepted() {
    assert!(
        Duel::from_data(
            FIXTURE_CARDS,
            FIXTURE_DECKS,
            "aggro",
            "control",
            VERSIONS,
            u32::MAX
        )
        .is_ok()
    );
}
