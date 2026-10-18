//! Format of the `unit` kind (#206, task 1 of #137): `unit` is the name written, `creature`
//! stays readable (alias), the keyword `immobile` replaces `force` and `attack_cost`, and the
//! frozen data sets are read unchanged.
use duel_cards::{
    Card, CardSpec, CardsError, CreatureStats, MAX_STAT, UnitSpec, parse_cards, validate_card_spec,
};
use serde_json::Value;

fn unit_json(kind: &str, extra: &str) -> String {
    format!(r#"{{"kind":"{kind}","id":"a","name":"A","cost":2,"health":5{extra}}}"#)
}

fn parse_one(json: &str) -> Result<Card, CardsError> {
    parse_cards(&format!("[{json}]")).map(|mut cards| cards.remove(0))
}

fn mobile(kind: &str) -> String {
    unit_json(
        kind,
        r#","force":3,"attack_cost":4,"provocation":true,"riposte":1"#,
    )
}

fn invalid(source: CardsError) -> CardsError {
    CardsError::InvalidCard {
        id: "a".into(),
        source: Box::new(source),
    }
}

#[test]
fn unit_and_creature_are_the_same_card() {
    let unit = parse_one(&mobile("unit")).unwrap();
    let creature = parse_one(&mobile("creature")).unwrap();
    assert_eq!(unit, creature);
    assert_eq!(
        serde_json::from_str::<Card>(&mobile("unit")).unwrap(),
        serde_json::from_str::<Card>(&mobile("creature")).unwrap()
    );
    let written: Value = serde_json::to_value(&creature).unwrap();
    assert_eq!(written["kind"], "unit");
}

#[test]
fn the_frozen_sets_are_read_unchanged() {
    for json in [
        include_str!("../../../data/fixture/1.0.0/cards.json"),
        include_str!("../../../data/starter/1.0.0/cards.json"),
        include_str!("../../../data/starter/1.1.0/cards.json"),
    ] {
        let cards = parse_cards(json).unwrap();
        assert!(!cards.is_empty());
        for card in &cards {
            let spec = card.spec();
            assert!(spec.is_plain_unit(), "{}", card.id());
            assert_eq!(spec.creature(), &card.stats(), "{}", card.id());
        }
    }
}

#[test]
fn a_spec_without_kind_is_a_unit_and_writes_without_kind() {
    let json = r#"{"cost":1,"force":2,"attack_cost":3,"health":4,"provocation":false,"riposte":0}"#;
    let spec: CardSpec = serde_json::from_str(json).unwrap();
    assert!(spec.is_plain_unit());
    assert_eq!(serde_json::to_string(&spec).unwrap(), json);
}

#[test]
fn a_spec_with_kind_unit_or_creature_is_read() {
    for kind in ["unit", "creature"] {
        let json = format!(
            r#"{{"kind":"{kind}","cost":1,"force":2,"attack_cost":3,"health":4,"riposte":2}}"#
        );
        let spec: CardSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec.creature().riposte, 2, "{kind}");
        let written = serde_json::to_string(&spec).unwrap();
        assert!(!written.contains("kind"), "{written}");
    }
    assert!(serde_json::from_str::<CardSpec>(r#"{"kind":"spell","cost":1,"health":1}"#).is_err());
}

#[test]
fn an_immobile_unit_has_no_force_nor_attack_cost() {
    let with_force = unit_json("unit", r#","immobile":true,"force":3"#);
    assert_eq!(
        parse_one(&with_force).unwrap_err(),
        invalid(CardsError::ImmobileWithAttackStat { field: "force" })
    );
    let with_cost = unit_json("unit", r#","immobile":true,"attack_cost":3"#);
    assert_eq!(
        parse_one(&with_cost).unwrap_err(),
        invalid(CardsError::ImmobileWithAttackStat {
            field: "attack_cost"
        })
    );
    let both = unit_json("unit", r#","immobile":true,"force":3,"attack_cost":3"#);
    assert_eq!(
        parse_one(&both).unwrap_err(),
        invalid(CardsError::ImmobileWithAttackStat { field: "force" })
    );
    let card = parse_one(&unit_json("unit", r#","immobile":true"#)).unwrap();
    let stats = card.stats();
    assert_eq!((stats.force, stats.attack_cost), (0, 0));
    assert!(!card.spec().is_plain_unit());
}

#[test]
fn a_unit_that_is_not_immobile_needs_force_and_attack_cost() {
    assert_eq!(
        parse_one(&unit_json("unit", r#","attack_cost":1"#)).unwrap_err(),
        invalid(CardsError::MissingAttackStat { field: "force" })
    );
    assert_eq!(
        parse_one(&unit_json("unit", r#","force":1"#)).unwrap_err(),
        invalid(CardsError::MissingAttackStat {
            field: "attack_cost"
        })
    );
    assert_eq!(
        parse_one(&unit_json("unit", "")).unwrap_err(),
        invalid(CardsError::MissingAttackStat { field: "force" })
    );
    // An explicit `immobile: false` is the same as no `immobile`.
    assert_eq!(
        parse_one(&unit_json("unit", r#","immobile":false"#)).unwrap_err(),
        invalid(CardsError::MissingAttackStat { field: "force" })
    );
    let ok = unit_json("unit", r#","immobile":false,"force":1,"attack_cost":1"#);
    assert!(parse_one(&ok).unwrap().spec().is_plain_unit());
}

#[test]
fn a_mobile_unit_keeps_force_and_attack_cost_at_least_one() {
    assert_eq!(
        parse_one(&unit_json("unit", r#","force":0,"attack_cost":1"#)).unwrap_err(),
        invalid(CardsError::NonPositiveStat { field: "force" })
    );
    assert_eq!(
        parse_one(&unit_json("unit", r#","force":1,"attack_cost":0"#)).unwrap_err(),
        invalid(CardsError::CostBelowOne {
            field: "attack_cost"
        })
    );
}

#[test]
fn an_immobile_unit_keeps_the_other_bounds() {
    let cases = [
        (
            r#"{"kind":"unit","id":"a","name":"A","cost":0,"health":5,"immobile":true}"#,
            CardsError::CostBelowOne { field: "cost" },
        ),
        (
            r#"{"kind":"unit","id":"a","name":"A","cost":1,"health":0,"immobile":true}"#,
            CardsError::NonPositiveStat { field: "health" },
        ),
        (
            r#"{"kind":"unit","id":"a","name":"A","cost":1,"health":1,"immobile":true,"riposte":10001}"#,
            CardsError::StatTooHigh {
                field: "riposte",
                value: MAX_STAT + 1,
                max: MAX_STAT,
            },
        ),
    ];
    for (json, expected) in cases {
        assert_eq!(parse_one(json).unwrap_err(), invalid(expected), "{json}");
    }
}

#[test]
fn immobile_is_omitted_when_false_and_written_when_true() {
    let plain = parse_one(&mobile("unit")).unwrap();
    let written = serde_json::to_value(&plain).unwrap();
    assert!(written.get("immobile").is_none(), "{written}");
    assert!(written.get("tags").is_none(), "{written}");
    let still = parse_one(&unit_json("unit", r#","immobile":true"#)).unwrap();
    let written = serde_json::to_value(&still).unwrap();
    assert_eq!(written["immobile"], true);
    assert!(written.get("force").is_none(), "{written}");
    assert!(written.get("attack_cost").is_none(), "{written}");
}

fn immobile_spec() -> CardSpec {
    CardSpec::Unit(UnitSpec {
        stats: CreatureStats {
            cost: 2,
            force: 0,
            attack_cost: 0,
            health: 5,
            provocation: false,
            riposte: 0,
        },
        immobile: true,
        tags: vec![],
    })
}

#[test]
fn an_immobile_spec_is_written_without_force_and_attack_cost() {
    let json = serde_json::to_string(&immobile_spec()).unwrap();
    assert_eq!(
        json,
        r#"{"cost":2,"health":5,"provocation":false,"riposte":0,"immobile":true}"#
    );
    assert_eq!(
        serde_json::from_str::<CardSpec>(&json).unwrap(),
        immobile_spec()
    );
    let tagged = CardSpec::Unit(UnitSpec {
        tags: vec!["bâtiment".into()],
        ..match immobile_spec() {
            CardSpec::Unit(unit) => unit,
        }
    });
    assert_eq!(
        serde_json::to_string(&tagged).unwrap(),
        r#"{"cost":2,"health":5,"provocation":false,"riposte":0,"immobile":true,"tags":["bâtiment"]}"#
    );
}

#[test]
fn an_immobile_spec_with_force_is_refused_by_serde() {
    for (json, field) in [
        (
            r#"{"cost":2,"force":1,"health":5,"immobile":true}"#,
            "force",
        ),
        (
            r#"{"cost":2,"attack_cost":1,"health":5,"immobile":true}"#,
            "attack_cost",
        ),
    ] {
        let error = serde_json::from_str::<CardSpec>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains(field), "{error}");
        assert!(error.contains("immobile"), "{error}");
    }
}

#[test]
fn a_mobile_spec_without_force_is_refused_by_serde() {
    let error = serde_json::from_str::<CardSpec>(r#"{"cost":1,"attack_cost":1,"health":1}"#)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("missing field `force`"), "{error}");
    let error = serde_json::from_str::<CardSpec>(r#"{"cost":1,"force":1,"health":1}"#)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("missing field `attack_cost`"), "{error}");
}

#[test]
fn validate_card_spec_refuses_an_immobile_spec_with_a_force() {
    // Defence in depth: built by hand, never through serde.
    let CardSpec::Unit(mut unit) = immobile_spec();
    assert_eq!(validate_card_spec(&CardSpec::Unit(unit.clone())), Ok(()));
    unit.stats.force = 3;
    assert_eq!(
        validate_card_spec(&CardSpec::Unit(unit.clone())),
        Err(CardsError::ImmobileWithAttackStat { field: "force" })
    );
    unit.stats.force = 0;
    unit.stats.attack_cost = 2;
    assert_eq!(
        validate_card_spec(&CardSpec::Unit(unit)),
        Err(CardsError::ImmobileWithAttackStat {
            field: "attack_cost"
        })
    );
}

#[test]
fn an_immobile_unit_refuses_a_zero_force_or_attack_cost_too() {
    // Presence is what is refused, never the value: `0` is not a sentinel of the data.
    for (extra, field) in [
        (r#","immobile":true,"force":0"#, "force"),
        (r#","immobile":true,"attack_cost":0"#, "attack_cost"),
        (r#","immobile":true,"force":0,"attack_cost":0"#, "force"),
        (r#","immobile":true,"attack_cost":0,"force":0"#, "force"),
    ] {
        assert_eq!(
            parse_one(&unit_json("unit", extra)).unwrap_err(),
            invalid(CardsError::ImmobileWithAttackStat { field }),
            "{extra}"
        );
    }
}

#[test]
fn an_immobile_spec_with_a_zero_force_or_attack_cost_is_refused_by_serde() {
    for (json, field) in [
        (
            r#"{"cost":2,"force":0,"health":5,"immobile":true}"#,
            "force",
        ),
        (
            r#"{"cost":2,"attack_cost":0,"health":5,"immobile":true}"#,
            "attack_cost",
        ),
        (
            r#"{"cost":2,"force":0,"attack_cost":0,"health":5,"immobile":true}"#,
            "force",
        ),
    ] {
        let error = serde_json::from_str::<CardSpec>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains(&format!("`{field}`")), "{json}: {error}");
        assert!(error.contains("immobile"), "{json}: {error}");
    }
}
