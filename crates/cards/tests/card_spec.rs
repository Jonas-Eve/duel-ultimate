//! `CardSpec` is the card type the engine setup consumes (story #128). With a single variant it
//! is read and written exactly like `CreatureStats`: same JSON, same errors, same validation.
use duel_cards::{
    CardSpec, CardsError, CreatureStats, MAX_STAT, validate_card_spec, validate_creature,
};

fn stats() -> CreatureStats {
    CreatureStats {
        cost: 2,
        force: 3,
        attack_cost: 4,
        health: 5,
        provocation: true,
        riposte: 1,
    }
}

#[test]
fn card_spec_json_is_exactly_the_creature_stats_json() {
    let spec = CardSpec::from(stats());
    let json = serde_json::to_string(&spec).unwrap();
    assert_eq!(json, serde_json::to_string(&stats()).unwrap());
    assert_eq!(
        json,
        r#"{"cost":2,"force":3,"attack_cost":4,"health":5,"provocation":true,"riposte":1}"#
    );
    // Six keys even for a plain creature: `provocation` and `riposte` are always written.
    let plain = CardSpec::from(CreatureStats {
        provocation: false,
        riposte: 0,
        ..stats()
    });
    assert_eq!(
        serde_json::to_string(&plain).unwrap(),
        r#"{"cost":2,"force":3,"attack_cost":4,"health":5,"provocation":false,"riposte":0}"#
    );
    assert_eq!(serde_json::from_str::<CardSpec>(&json).unwrap(), spec);
}

#[test]
fn card_spec_reads_four_or_six_keys() {
    let four = r#"{"cost":1,"force":2,"attack_cost":3,"health":4}"#;
    let spec: CardSpec = serde_json::from_str(four).unwrap();
    assert_eq!(
        spec.creature(),
        &CreatureStats {
            cost: 1,
            force: 2,
            attack_cost: 3,
            health: 4,
            provocation: false,
            riposte: 0,
        }
    );
    let six = r#"{"cost":1,"force":2,"attack_cost":3,"health":4,"provocation":true,"riposte":2}"#;
    let spec: CardSpec = serde_json::from_str(six).unwrap();
    assert!(spec.creature().provocation);
    assert_eq!(spec.creature().riposte, 2);
}

#[test]
fn card_spec_errors_are_the_creature_stats_errors() {
    // Same cause, same position as `CreatureStats` (#206 re-pins this test: the text of an
    // `unknown field` error, which lists the expected keys, is pinned in full below).
    let bad = [
        r#"{"cost":"1","force":1,"attack_cost":1,"health":1}"#,
        r#"{"cost":1,"force":1,"attack_cost":1}"#,
        r#"{"cost":1,"force":1,"attack_cost":1,"health":1,"riposte":-1}"#,
    ];
    for json in bad {
        let expected = serde_json::from_str::<CreatureStats>(json).unwrap_err();
        let got = serde_json::from_str::<CardSpec>(json).unwrap_err();
        assert_eq!(got.to_string(), expected.to_string(), "{json}");
    }
    // `null` has the same cause (the struct name is that of the wire form, `CardSpec`).
    let got = serde_json::from_str::<CardSpec>("null").unwrap_err().to_string();
    assert!(got.starts_with("invalid type: null, expected"), "{got}");
    assert!(serde_json::from_str::<CardSpec>("[1,2]").is_err());
}

#[test]
fn card_spec_pins_the_text_of_its_unknown_field_error() {
    let json = r#"{"cost":1,"force":1,"attack_cost":1,"health":1,"bonus":9}"#;
    assert_eq!(
        serde_json::from_str::<CardSpec>(json).unwrap_err().to_string(),
        "unknown field `bonus`, expected one of `kind`, `cost`, `force`, `attack_cost`, `health`, \
         `provocation`, `riposte`, `immobile`, `tags` at line 1 column 54"
    );
}

#[test]
fn card_spec_reports_the_first_missing_field_in_a_pinned_order() {
    // Required fields of the flat form first (`cost`, `health`), then `force`, then `attack_cost`.
    for (json, expected) in [
        (r#"{}"#, "missing field `cost` at line 1 column 2"),
        (r#"{"cost":1}"#, "missing field `health` at line 1 column 10"),
        (
            r#"{"cost":1,"health":1}"#,
            "missing field `force` at line 1 column 21",
        ),
        (
            r#"{"cost":1,"force":1,"health":1}"#,
            "missing field `attack_cost` at line 1 column 31",
        ),
    ] {
        assert_eq!(
            serde_json::from_str::<CardSpec>(json).unwrap_err().to_string(),
            expected,
            "{json}"
        );
    }
}

#[test]
fn validate_card_spec_gives_the_same_errors_as_validate_creature() {
    let mut cases = vec![stats()];
    for field in 0..6 {
        for value in [0, MAX_STAT, MAX_STAT + 1] {
            let mut s = stats();
            match field {
                0 => s.cost = value,
                1 => s.force = value,
                2 => s.attack_cost = value,
                3 => s.health = value,
                4 => s.riposte = value,
                _ => s.provocation = value == 0,
            }
            cases.push(s);
        }
    }
    for s in cases {
        let expected: Result<(), CardsError> = validate_creature(&s);
        assert_eq!(
            validate_card_spec(&CardSpec::from(s.clone())),
            expected,
            "{s:?}"
        );
    }
    let mut over = stats();
    over.riposte = MAX_STAT + 1;
    assert!(matches!(
        validate_card_spec(&over.into()),
        Err(CardsError::StatTooHigh {
            field: "riposte",
            ..
        })
    ));
}
