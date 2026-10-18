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
        spec.creature().unwrap(),
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
    assert!(spec.creature().unwrap().provocation);
    assert_eq!(spec.creature().unwrap().riposte, 2);
}

#[test]
fn card_spec_errors_are_the_creature_stats_errors() {
    // Same cause, same position as `CreatureStats` (#206 re-pins this test: the text of an
    // `unknown field` error, which lists the expected keys, is pinned in full below).
    let bad = [
        r#"{"cost":"1","force":1,"attack_cost":1,"health":1}"#,
        r#"{"cost":1,"force":1,"attack_cost":1,"health":1,"riposte":-1}"#,
    ];
    for json in bad {
        let expected = serde_json::from_str::<CreatureStats>(json).unwrap_err();
        let got = serde_json::from_str::<CardSpec>(json).unwrap_err();
        assert_eq!(got.to_string(), expected.to_string(), "{json}");
    }
    // A missing `health` is the same cause (without the position, see the pinned order below).
    let json = r#"{"cost":1,"force":1,"attack_cost":1}"#;
    let expected = serde_json::from_str::<CreatureStats>(json)
        .unwrap_err()
        .to_string();
    let got = serde_json::from_str::<CardSpec>(json)
        .unwrap_err()
        .to_string();
    assert!(expected.starts_with(&got), "{expected} / {got}");
    // `null` has the same cause (the struct name is that of the wire form, `CardSpec`).
    let got = serde_json::from_str::<CardSpec>("null")
        .unwrap_err()
        .to_string();
    assert!(got.starts_with("invalid type: null, expected"), "{got}");
    assert!(serde_json::from_str::<CardSpec>("[1,2]").is_err());
}

#[test]
fn card_spec_pins_the_text_of_its_unknown_field_error() {
    let json = r#"{"cost":1,"force":1,"attack_cost":1,"health":1,"bonus":9}"#;
    assert_eq!(
        serde_json::from_str::<CardSpec>(json)
            .unwrap_err()
            .to_string(),
        "unknown field `bonus`, expected one of `kind`, `cost`, `force`, `attack_cost`, `health`, \
         `provocation`, `riposte`, `immobile`, `tags`, `effects` at line 1 column 54"
    );
}

#[test]
fn card_spec_reports_the_first_missing_field_in_a_pinned_order() {
    // The flat form reads every field as optional (#207: `cost` and `health` too, a spell has no
    // `health`), so the check comes after the struct is read: no position in the message. Order:
    // `cost`, `health`, then `force`, then `attack_cost`.
    for (json, expected) in [
        (r#"{}"#, "missing field `cost`"),
        (r#"{"cost":1}"#, "missing field `health`"),
        (r#"{"cost":1,"health":1}"#, "missing field `force`"),
        (
            r#"{"cost":1,"force":1,"health":1}"#,
            "missing field `attack_cost`",
        ),
    ] {
        assert_eq!(
            serde_json::from_str::<CardSpec>(json)
                .unwrap_err()
                .to_string(),
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

#[test]
fn a_plain_unit_spec_is_still_exactly_the_creature_stats_json() {
    // The wire form of a unit without effects is unchanged byte for byte (#207): no `kind`, no
    // `effects`, whatever the new variants.
    let spec = CardSpec::from(stats());
    assert_eq!(
        serde_json::to_string(&spec).unwrap(),
        serde_json::to_string(&stats()).unwrap()
    );
    assert_eq!(spec.creature(), Some(&stats()));
    assert!(spec.is_plain_unit());
}

#[test]
fn is_plain_unit_is_false_for_a_spell_and_for_a_unit_with_effects() {
    use duel_cards::{CardEffect, SpellSpec, UnitSpec};
    let draw: CardEffect = serde_json::from_str(
        r#"{"trigger":"on_play","ops":[{"op":"draw","target":{"select":"hero","side":"own"},"count":1}]}"#,
    )
    .unwrap();
    let spell = CardSpec::Spell(SpellSpec {
        cost: 2,
        tags: vec![],
        effects: vec![draw.clone()],
    });
    assert!(!spell.is_plain_unit());
    assert!(spell.creature().is_none());
    let unit = CardSpec::Unit(UnitSpec {
        stats: stats(),
        immobile: false,
        tags: vec![],
        effects: vec![draw],
    });
    assert!(!unit.is_plain_unit());
    assert_eq!(unit.creature(), Some(&stats()));
}

#[test]
fn a_null_value_is_refused_except_for_the_attack_stats_and_the_kind() {
    // `null` was never a value of `cost`, `health`, `provocation`, `riposte` (wasm test
    // `a_setup_with_provocation_and_riposte_starts_and_hostile_forms_are_refused`), and still is
    // not now that every field is read as an option.
    for field in ["cost", "health", "provocation", "riposte", "immobile"] {
        let mut json: serde_json::Value =
            serde_json::from_str(r#"{"cost":1,"force":1,"attack_cost":1,"health":1}"#).unwrap();
        json[field] = serde_json::Value::Null;
        let error = serde_json::from_value::<CardSpec>(json).unwrap_err();
        assert!(
            error.to_string().contains("invalid type: null"),
            "{field}: {error}"
        );
        let spell = serde_json::json!({"kind":"spell","cost":1,field:null,"effects":[]});
        assert!(
            serde_json::from_value::<CardSpec>(spell).is_err(),
            "{field}"
        );
    }
    // As since #206, `force`, `attack_cost` and `kind` read `null` as absent.
    let json = r#"{"kind":null,"cost":1,"force":null,"attack_cost":null,"health":1}"#;
    assert!(
        serde_json::from_str::<CardSpec>(json)
            .unwrap_err()
            .to_string()
            .contains("missing field `force`")
    );
}
