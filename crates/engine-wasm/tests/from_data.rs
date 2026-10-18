//! `Duel::from_data` (what `Game.from_data` runs): a game built from `cards.json` and
//! `decks.json` strings, decks chosen by identifier (S2-T1 of #52). Equivalent to
//! `Duel::from_json` on the setup built by hand, with stable error codes and a 1 MiB ceiling
//! on each of the five strings.

use duel_cards::{parse_cards, parse_decks, resolve_deck};
use duel_engine::{GameSetup, PlayerSetup, RuleSet};
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
    Duel::from_data(cards, decks, one, two, versions, 1, None)
}

/// The setup built by hand from `duel-cards` (no help from the code under test), as JSON.
fn by_hand(cards: &str, decks: &str, one: &str, two: &str, versions: &[&str]) -> String {
    let cards = parse_cards(cards).unwrap();
    let decks = parse_decks(decks).unwrap();
    let side = |id: &str| {
        let deck = decks.iter().find(|d| d.id == id).unwrap();
        PlayerSetup {
            hero: deck.hero.clone(),
            cards: resolve_deck(deck, &cards)
                .unwrap()
                .into_iter()
                .map(Into::into)
                .collect(),
            card_names: Vec::new(),
        }
    };
    serde_json::to_string(&GameSetup {
        one: side(one),
        two: side(two),
        data_versions: versions.iter().map(|v| (*v).to_owned()).collect(),
        rules: RuleSet::default(),
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
                let data = Duel::from_data(cards, decks, one, two, &versions, seed, None).unwrap();
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
            let a = Duel::from_data(FIXTURE_CARDS, FIXTURE_DECKS, one, two, VERSIONS, seed, None)
                .unwrap();
            let b =
                Duel::from_data(FIXTURE_CARDS, &reordered, one, two, VERSIONS, seed, None).unwrap();
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
            u32::MAX,
            None
        )
        .is_ok()
    );
}

// ---- #147: the optional sixth input, `rules_json` ------------------------------------------

/// `from_data` on the fixture with the given rules text (`None` = absent) and versions.
fn with_rules(rules: Option<&str>, versions: &str) -> Result<Duel, String> {
    Duel::from_data(
        FIXTURE_CARDS,
        FIXTURE_DECKS,
        "aggro",
        "control",
        versions,
        1,
        rules,
    )
}

/// A rule set that differs from the engine default in both options, written relatively to it
/// (the default changes with #127, these tests must not).
fn non_default() -> RuleSet {
    let default = RuleSet::default();
    RuleSet {
        hero_riposte: !default.hero_riposte,
        board_limit: if default.board_limit == Some(5) {
            Some(6)
        } else {
            Some(5)
        },
    }
}

fn rules_text(rules: &RuleSet) -> String {
    serde_json::to_string(rules).unwrap()
}

fn start_events(game: &Duel) -> String {
    game.start_events_json().unwrap()
}

#[test]
fn without_rules_is_the_default_game() {
    let absent = start_events(&with_rules(None, VERSIONS).unwrap());
    let default = rules_text(&RuleSet::default());
    let explicit = start_events(&with_rules(Some(&default), VERSIONS).unwrap());
    let empty_object = start_events(&with_rules(Some("{}"), VERSIONS).unwrap());
    assert_eq!(absent, explicit);
    assert_eq!(absent, empty_object);
    assert!(!absent.contains("\"rules\""), "{absent}");
}

#[test]
fn non_default_rules_are_recorded_last() {
    let rules = non_default();
    let text = rules_text(&rules);
    let suffix = format!(",\"rules\":{text}}}}}]");
    for versions in [VERSIONS, "[]"] {
        let events = start_events(&with_rules(Some(&text), versions).unwrap());
        assert!(events.ends_with(&suffix), "{events}");
        if versions == VERSIONS {
            let versions_at = events.find("\"data_versions\"").unwrap();
            let rules_at = events.find("\"rules\"").unwrap();
            assert!(versions_at < rules_at, "{events}");
        }
    }
}

#[test]
fn bad_rules_are_refused_with_the_existing_codes() {
    for (rules, expected) in [
        (r#"{"nope":1}"#, "invalid_json"),
        (r#"{"hero_riposte":"yes"}"#, "invalid_json"),
        (r#"{"board_limit":"5"}"#, "invalid_json"),
        (r#"{"board_limit":-1}"#, "invalid_json"),
        ("", "invalid_json"),
        ("null", "invalid_json"),
        (r#"{"hero_riposte":true,"#, "invalid_json"),
        (r#"{"board_limit":0}"#, "invalid_setup"),
        (r#"{"board_limit":31}"#, "invalid_setup"),
    ] {
        let err = with_rules(Some(rules), VERSIONS).err().unwrap();
        assert_eq!(code(&err), expected, "{rules:?}");
    }
    // The bounds themselves are valid.
    for rules in [
        r#"{"board_limit":1}"#,
        r#"{"board_limit":30}"#,
        r#"{"board_limit":null}"#,
    ] {
        assert!(with_rules(Some(rules), VERSIONS).is_ok(), "{rules}");
    }
}

#[test]
fn the_rules_are_bounded_before_any_parsing() {
    let huge = " ".repeat(MAX_INPUT_BYTES + 1);
    let err = with_rules(Some(&huge), VERSIONS).err().unwrap();
    assert_eq!(code(&err), "input_too_large");
    // Even when every other string is unreadable: the size comes first.
    let err = Duel::from_data("{", "[", "nope", "nope", "[", 1, Some(&huge))
        .err()
        .unwrap();
    assert_eq!(code(&err), "input_too_large");
    // Exactly at the limit, it is the analysis that answers.
    let spaces = " ".repeat(MAX_INPUT_BYTES);
    let err = with_rules(Some(&spaces), VERSIONS).err().unwrap();
    assert_eq!(code(&err), "invalid_json");
}

#[test]
fn the_rules_are_read_after_the_versions_and_before_the_data() {
    let from_data_with = |deck: &str, versions: &str, rules: &str| {
        Duel::from_data(
            FIXTURE_CARDS,
            FIXTURE_DECKS,
            deck,
            "control",
            versions,
            1,
            Some(rules),
        )
        .err()
        .map(|e| code(&e))
    };
    // Unreadable rules win over an unknown deck.
    assert_eq!(
        from_data_with("nope", VERSIONS, r#"{"nope":1}"#).as_deref(),
        Some("invalid_json")
    );
    // So do out-of-range rules (`new_game` revalidates, after the decks are resolved: a valid
    // deck here, the unknown one would answer first).
    assert_eq!(
        from_data_with("aggro", VERSIONS, r#"{"board_limit":0}"#).as_deref(),
        Some("invalid_setup")
    );
    // Valid rules and an unknown deck: the deck answers.
    assert_eq!(
        from_data_with("nope", VERSIONS, "{}").as_deref(),
        Some("unknown_deck")
    );
    // Versions are analysed before the rules.
    assert_eq!(
        from_data_with("aggro", "[", r#"{"nope":1}"#).as_deref(),
        Some("invalid_json")
    );
}

#[test]
fn from_json_with_a_rules_key_equals_from_data_with_the_same_rules() {
    let rules = non_default();
    let mut setup: Value = serde_json::from_str(&by_hand(
        FIXTURE_CARDS,
        FIXTURE_DECKS,
        "aggro",
        "control",
        &["fixture@1.0.0"],
    ))
    .unwrap();
    setup["rules"] = serde_json::to_value(rules).unwrap();
    let json = Duel::from_json(&setup.to_string(), 1).unwrap();
    let data = with_rules(Some(&rules_text(&rules)), VERSIONS).unwrap();
    assert_eq!(start_events(&json), start_events(&data));
    assert!(start_events(&json).contains("\"rules\""));
}

/// #170: with `hero_riposte` a non-lethal hit on the hero destroys the attacker: the
/// `hero_riposted` event comes through `apply`, and the attacker is in the discard of `view`.
#[test]
fn the_hero_riposte_reaches_apply_and_the_view() {
    let mut game = with_rules(
        Some(r#"{"hero_riposte":true,"board_limit":null}"#),
        VERSIONS,
    )
    .unwrap();
    let mut slain = None;
    for _ in 0..200 {
        let list: Vec<Value> = serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
        let hit = list
            .iter()
            .find(|a| a["attack"]["target"] == "hero")
            .cloned();
        let Some(action) = hit.or_else(|| list.iter().find(|a| **a != "concede").cloned()) else {
            break; // the game is over
        };
        let seat = {
            let view: Value = serde_json::from_str(&game.view_json("One").unwrap()).unwrap();
            view["active"].as_str().unwrap().to_owned()
        };
        let events: Vec<Value> =
            serde_json::from_str(&game.apply_json(&action.to_string()).unwrap()).unwrap();
        if let Some(event) = events.iter().find_map(|e| e.get("hero_riposted")) {
            assert_eq!(event["player"], seat.as_str());
            assert!(
                events
                    .iter()
                    .any(|e| e["died"]["creature"] == event["creature"]),
                "{events:?}"
            );
            slain = Some((seat, event["creature"].clone()));
            break;
        }
    }
    let (seat, creature) = slain.expect("a hero hit was riposted");
    let view: Value = serde_json::from_str(&game.view_json(&seat).unwrap()).unwrap();
    let discard = view["you"]["discard"].as_array().unwrap();
    assert!(discard.iter().any(|c| c["id"] == creature), "{view}");
    assert!(
        !view["you"]["board"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == creature)
    );
}

// ---- #171: `board_limit` acts --------------------------------------------------------------

/// With `{"board_limit":1}` the second summoning of a seat is refused with `board_full` and
/// the game is left exactly as it was (legal actions, view of both seats).
#[test]
fn a_board_limit_in_rules_json_refuses_a_second_summoning_with_board_full() {
    let mut game = with_rules(Some(r#"{"hero_riposte":false,"board_limit":1}"#), VERSIONS).unwrap();
    let plays = |game: &Duel| -> Vec<String> {
        let list: Vec<Value> = serde_json::from_str(&game.legal_actions_json().unwrap()).unwrap();
        list.iter()
            .filter(|a| a.get("play").is_some())
            .map(Value::to_string)
            .collect()
    };
    // The active seat summons until the token changes sides or its board is full.
    let first = plays(&game).into_iter().next().expect("a summoning");
    game.apply_json(&first).unwrap();
    // Whoever is active now, a seat with a creature on its board is refused another one: play
    // on until some seat is active with a board of one and cards in hand.
    for _ in 0..4 {
        let view: Value = serde_json::from_str(&game.view_json("One").unwrap()).unwrap();
        let active = view["active"].as_str().unwrap().to_owned();
        let seat = serde_json::from_str::<Value>(&game.view_json(&active).unwrap()).unwrap();
        if seat["you"]["board"].as_array().unwrap().len() == 1 {
            let card = seat["you"]["hand"][0]["id"].as_u64().unwrap();
            let legal_before = game.legal_actions_json().unwrap();
            let views_before = (
                game.view_json("One").unwrap(),
                game.view_json("Two").unwrap(),
            );
            let error = game
                .apply_json(&format!(r#"{{"play":{{"card":{card}}}}}"#))
                .unwrap_err();
            assert_eq!(code(&error), "board_full", "{error}");
            assert_eq!(game.legal_actions_json().unwrap(), legal_before);
            assert_eq!(
                (
                    game.view_json("One").unwrap(),
                    game.view_json("Two").unwrap()
                ),
                views_before
            );
            return;
        }
        let next = plays(&game).into_iter().next().expect("a summoning");
        game.apply_json(&next).unwrap();
    }
    panic!("no seat reached a full board");
}
