//! `provocation` (boolean, default false) and `riposte` (u32 >= 0, default 0, <= `MAX_STAT`) are
//! plain data on creature cards and `CreatureStats`; the engine does not read them yet (S2b-T2
//! reads `provocation`, S2b-T3 reads `riposte`).
use duel_cards::{
    Card, CardsError, CreatureStats, DECK_SIZE, Deck, Effect, Hero, HeroEffect, MAX_STAT,
    parse_cards, parse_decks, resolve_deck, validate_creature,
};

const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

const OLD_STATS: &str = r#"{"cost":2,"force":3,"attack_cost":4,"health":5}"#;
const OLD_CARD: &str =
    r#"[{"kind":"creature","id":"a","name":"A","cost":2,"force":3,"attack_cost":4,"health":5}]"#;

fn stats(provocation: bool, riposte: u32) -> CreatureStats {
    CreatureStats {
        cost: 1,
        force: 1,
        attack_cost: 1,
        health: 1,
        provocation,
        riposte,
    }
}

fn card(id: &str, provocation: bool, riposte: u32) -> Card {
    Card::Creature {
        id: id.into(),
        name: id.into(),
        cost: 1,
        force: 1,
        attack_cost: 1,
        health: 1,
        provocation,
        riposte,
    }
}

fn card_json(extra: &str) -> String {
    format!(
        r#"[{{"kind":"creature","id":"a","name":"A","cost":2,"force":3,"attack_cost":4,"health":5,{extra}}}]"#
    )
}

fn stats_json(extra: &str) -> String {
    format!(r#"{{"cost":2,"force":3,"attack_cost":4,"health":5,{extra}}}"#)
}

fn deck_of(ids: &[&str]) -> Deck {
    Deck {
        id: "d".into(),
        name: "D".into(),
        hero: Hero {
            name: "H".into(),
            max_time: 8,
            starting_hand: 4,
            initiative: 1,
            start_budget: 3,
            health: 10,
            effects: vec![HeroEffect {
                name: "Piocher".into(),
                cost: 2,
                effect: Effect::Draw { count: 1 },
            }],
        },
        cards: ids.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn creature_stats_round_trip_with_both_fields() {
    let s = CreatureStats {
        cost: 2,
        force: 3,
        attack_cost: 4,
        health: 5,
        provocation: true,
        riposte: 7,
    };
    let json = serde_json::to_string(&s).unwrap();
    assert_eq!(serde_json::from_str::<CreatureStats>(&json).unwrap(), s);
}

#[test]
fn creature_stats_always_serialize_both_fields_canonically() {
    let json = serde_json::to_string(&stats(false, 0)).unwrap();
    assert_eq!(
        json,
        r#"{"cost":1,"force":1,"attack_cost":1,"health":1,"provocation":false,"riposte":0}"#
    );
}

#[test]
fn a_four_key_stats_json_defaults_provocation_to_false() {
    let s: CreatureStats = serde_json::from_str(OLD_STATS).unwrap();
    assert!(!s.provocation);
    assert_eq!(s.riposte, 0);
}

#[test]
fn a_stats_json_with_only_one_of_the_new_keys_defaults_the_other() {
    let s: CreatureStats = serde_json::from_str(&stats_json(r#""provocation":true"#)).unwrap();
    assert!(s.provocation);
    assert_eq!(s.riposte, 0);
    let s: CreatureStats = serde_json::from_str(&stats_json(r#""riposte":3"#)).unwrap();
    assert!(!s.provocation);
    assert_eq!(s.riposte, 3);
}

#[test]
fn a_four_key_card_json_defaults_both_fields_in_parse_cards() {
    let cards = parse_cards(OLD_CARD).unwrap();
    assert_eq!(
        cards[0].stats(),
        CreatureStats {
            cost: 2,
            force: 3,
            attack_cost: 4,
            health: 5,
            provocation: false,
            riposte: 0,
        }
    );
    let cards = parse_cards(&card_json(r#""provocation":true"#)).unwrap();
    assert_eq!(cards[0].stats().riposte, 0);
    assert!(cards[0].stats().provocation);
    let cards = parse_cards(&card_json(r#""riposte":6"#)).unwrap();
    assert!(!cards[0].stats().provocation);
    assert_eq!(cards[0].stats().riposte, 6);
}

#[test]
fn a_card_round_trips_through_json_with_both_fields() {
    let cards = vec![card("a", true, 2), card("b", false, 0)];
    let json = serde_json::to_string(&cards).unwrap();
    assert_eq!(parse_cards(&json).unwrap(), cards);
}

#[test]
fn riposte_zero_and_max_stat_are_accepted_above_is_refused() {
    assert_eq!(validate_creature(&stats(false, 0)), Ok(()));
    assert_eq!(validate_creature(&stats(false, MAX_STAT)), Ok(()));
    assert_eq!(
        validate_creature(&stats(false, MAX_STAT + 1)),
        Err(CardsError::StatTooHigh {
            field: "riposte",
            value: MAX_STAT + 1,
            max: MAX_STAT
        })
    );
}

#[test]
fn parse_cards_refuses_a_riposte_above_max_stat() {
    let json = card_json(&format!(r#""riposte":{}"#, MAX_STAT + 1));
    assert_eq!(
        parse_cards(&json),
        Err(CardsError::InvalidCard {
            id: "a".into(),
            source: Box::new(CardsError::StatTooHigh {
                field: "riposte",
                value: MAX_STAT + 1,
                max: MAX_STAT
            })
        })
    );
    assert!(parse_cards(&card_json(&format!(r#""riposte":{MAX_STAT}"#))).is_ok());
    assert!(parse_cards(&card_json(r#""riposte":0"#)).is_ok());
}

#[test]
fn hostile_forms_are_refused_by_serde_one_by_one() {
    let hostile = [
        r#""riposte":-1"#,
        r#""riposte":1.5"#,
        r#""riposte":"3""#,
        r#""riposte":null"#,
        r#""riposte":4294967296"#,
        r#""provocation":"true""#,
        r#""provocation":1"#,
        r#""provocation":null"#,
        r#""bonus":1"#,
    ];
    for extra in hostile {
        assert!(
            serde_json::from_str::<CreatureStats>(&stats_json(extra)).is_err(),
            "stats accepted {extra}"
        );
        assert!(
            matches!(parse_cards(&card_json(extra)), Err(CardsError::Json(_))),
            "card accepted {extra}"
        );
    }
}

#[test]
fn an_unknown_key_is_refused_on_cards_and_on_stats() {
    let err = serde_json::from_str::<CreatureStats>(&stats_json(r#""rage":1"#)).unwrap_err();
    assert!(err.to_string().contains("rage"), "{err}");
    let err = parse_cards(&card_json(r#""rage":1"#)).unwrap_err();
    assert!(
        matches!(&err, CardsError::Json(m) if m.contains("rage")),
        "{err}"
    );
}

#[test]
fn resolve_deck_passes_both_fields_through_in_deck_order() {
    let cards = vec![card("a", true, 2), card("b", false, 0), card("c", false, 9)];
    let mut ids = vec!["b"; DECK_SIZE];
    ids[0] = "a";
    ids[1] = "c";
    let resolved = resolve_deck(&deck_of(&ids), &cards).unwrap();
    assert_eq!(resolved[0], stats(true, 2));
    assert_eq!(resolved[1], stats(false, 9));
    assert_eq!(resolved[2], stats(false, 0));
}

#[test]
fn the_fixture_has_no_provocation_and_no_riposte_anywhere() {
    let cards = parse_cards(FIXTURE_CARDS).unwrap();
    assert!(!cards.is_empty());
    for c in &cards {
        let s = c.stats();
        assert!(!s.provocation, "{}", c.id());
        assert_eq!(s.riposte, 0, "{}", c.id());
    }
    for deck in parse_decks(FIXTURE_DECKS).unwrap() {
        for s in resolve_deck(&deck, &cards).unwrap() {
            assert!(!s.provocation);
            assert_eq!(s.riposte, 0);
        }
    }
}
