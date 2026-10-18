//! Effects of units and spells (#207, task 2 of #137): closed sets, bounds, derived Phi class,
//! polarity, expected picks and wire form. Every refusal is pinned with its boundary values
//! (0 and the maximum included).
use duel_cards::{
    AttackMode, Card, CardEffect, CardSpec, CardsError, Duration, Keyword, MAX_CARD_EFFECTS,
    MAX_EFFECT_PICKS, MAX_OPS_PER_EFFECT, MAX_STAT, Op, OpKind, PhiClass, PickKind, PickRole,
    PickSlot, Polarity, Selector, SelectorKind, Side, SpellSpec, Stat, Trigger, parse_card_set,
    validate_card_spec,
};
use serde_json::Value;

// ---- builders -------------------------------------------------------------------------------

const ATTACK: &str =
    r#"{"op":"attack","target":{"select":"hero_or_unit","side":"opposing"},"amount":3}"#;
const DEAL: &str =
    r#"{"op":"deal_damage","target":{"select":"hero_or_unit","side":"opposing"},"amount":2}"#;
const DEAL_UNIT: &str =
    r#"{"op":"deal_damage","target":{"select":"unit","side":"opposing"},"amount":2}"#;
const DRAW: &str = r#"{"op":"draw","target":{"select":"hero","side":"own"},"count":1}"#;
const MODIFY: &str = r#"{"op":"modify_stat","target":{"select":"self"},"stat":"force","delta":1}"#;
const GRANT: &str = r#"{"op":"grant_keyword","target":{"select":"self"},"keyword":"ranged"}"#;

fn on_play(ops: &[&str]) -> String {
    format!(r#"{{"trigger":"on_play","ops":[{}]}}"#, ops.join(","))
}
fn activated(ops: &[&str]) -> String {
    format!(
        r#"{{"trigger":"activated","cost":2,"ops":[{}]}}"#,
        ops.join(",")
    )
}
fn continuous(ops: &[&str]) -> String {
    format!(r#"{{"trigger":"continuous","ops":[{}]}}"#, ops.join(","))
}

fn unit_json(effects: &[String]) -> String {
    format!(
        r#"{{"kind":"unit","id":"a","name":"A","cost":2,"force":3,"attack_cost":2,"health":5,"effects":[{}]}}"#,
        effects.join(",")
    )
}
fn immobile_json(effects: &[String]) -> String {
    format!(
        r#"{{"kind":"unit","id":"a","name":"A","cost":2,"health":5,"immobile":true,"effects":[{}]}}"#,
        effects.join(",")
    )
}
fn spell_json(cost: u32, effects: &[String]) -> String {
    format!(
        r#"{{"kind":"spell","id":"a","name":"A","cost":{cost},"effects":[{}]}}"#,
        effects.join(",")
    )
}

fn parse(card: &str) -> Result<Vec<Card>, CardsError> {
    parse_card_set(&format!("[{card}]"), &["bâtiment".to_owned()])
}

/// The error of a card, unwrapped from `InvalidCard { id: "a", .. }`.
fn refused(card: &str) -> CardsError {
    match parse(card) {
        Err(CardsError::InvalidCard { id, source }) => {
            assert_eq!(id, "a");
            *source
        }
        other => panic!("expected an invalid card, got {other:?} for {card}"),
    }
}

fn unit_refused(effects: &[String]) -> CardsError {
    refused(&unit_json(effects))
}

fn accepted(card: &str) {
    assert!(parse(card).is_ok(), "{card}: {:?}", parse(card));
}

fn effect(json: &str) -> CardEffect {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}
fn op(json: &str) -> Op {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

// ---- counts and costs -----------------------------------------------------------------------

#[test]
fn the_bounds_are_four_three_and_three() {
    assert_eq!(MAX_CARD_EFFECTS, 4);
    assert_eq!(MAX_OPS_PER_EFFECT, 3);
    assert_eq!(MAX_EFFECT_PICKS, 3);
}

#[test]
fn too_many_card_effects() {
    let one = on_play(&[DRAW]);
    for n in [0, 1, MAX_CARD_EFFECTS] {
        accepted(&unit_json(&vec![one.clone(); n]));
    }
    assert_eq!(
        unit_refused(&vec![one.clone(); MAX_CARD_EFFECTS + 1]),
        CardsError::TooManyCardEffects {
            found: 5,
            max: MAX_CARD_EFFECTS
        }
    );
    // The same bound for a spell.
    accepted(&spell_json(2, &vec![one.clone(); MAX_CARD_EFFECTS]));
    assert_eq!(
        refused(&spell_json(2, &vec![one; MAX_CARD_EFFECTS + 1])),
        CardsError::TooManyCardEffects {
            found: 5,
            max: MAX_CARD_EFFECTS
        }
    );
}

#[test]
fn effect_without_ops() {
    assert_eq!(
        unit_refused(&[on_play(&[])]),
        CardsError::EffectWithoutOps { effect: 0 }
    );
    // The index is the one of the effect in the card.
    assert_eq!(
        unit_refused(&[on_play(&[DRAW]), on_play(&[])]),
        CardsError::EffectWithoutOps { effect: 1 }
    );
}

#[test]
fn too_many_ops() {
    accepted(&unit_json(&[on_play(&[DRAW])]));
    accepted(&unit_json(&[on_play(&[DRAW, DRAW, DRAW])]));
    assert_eq!(
        unit_refused(&[on_play(&[DRAW]), on_play(&[DRAW; 4])]),
        CardsError::TooManyOps {
            effect: 1,
            found: 4,
            max: MAX_OPS_PER_EFFECT
        }
    );
}

#[test]
fn cost_on_a_non_activated_effect() {
    for (trigger, ops) in [("on_play", DRAW), ("continuous", MODIFY)] {
        // Presence is refused, whatever the value (0 included).
        for cost in [0, 1, MAX_STAT] {
            let json = format!(r#"{{"trigger":"{trigger}","cost":{cost},"ops":[{ops}]}}"#);
            assert_eq!(
                unit_refused(&[json]),
                CardsError::EffectCostForbidden { effect: 0 },
                "{trigger} {cost}"
            );
        }
    }
}

#[test]
fn activated_effect_without_cost() {
    let json = format!(r#"{{"trigger":"activated","ops":[{DEAL}]}}"#);
    assert_eq!(
        unit_refused(&[json]),
        CardsError::EffectCostRequired { effect: 0 }
    );
}

#[test]
fn a_continuous_effect_has_no_cost() {
    accepted(&unit_json(&[continuous(&[MODIFY])]));
    let json = format!(r#"{{"trigger":"continuous","cost":2,"ops":[{MODIFY}]}}"#);
    assert_eq!(
        unit_refused(&[json]),
        CardsError::EffectCostForbidden { effect: 0 }
    );
}

#[test]
fn effect_cost_zero_and_above_the_maximum() {
    let with = |cost: u32| format!(r#"{{"trigger":"activated","cost":{cost},"ops":[{DEAL}]}}"#);
    assert_eq!(
        unit_refused(&[with(0)]),
        CardsError::EffectCostBelowOne { effect: 0 }
    );
    accepted(&unit_json(&[with(1)]));
    accepted(&unit_json(&[with(MAX_STAT)]));
    assert_eq!(
        unit_refused(&[on_play(&[DRAW]), with(MAX_STAT + 1)]),
        CardsError::EffectStatTooHigh {
            effect: 1,
            field: "cost",
            value: MAX_STAT + 1,
            max: MAX_STAT
        }
    );
}

// ---- trigger, selector, side per operation (the table) --------------------------------------

const OPS: [(&str, &str); 5] = [
    ("attack", "attack"),
    ("deal_damage", "deal_damage"),
    ("draw", "draw"),
    ("modify_stat", "modify_stat"),
    ("grant_keyword", "grant_keyword"),
];

fn selector_json(select: &str) -> String {
    if select == "self" {
        r#"{"select":"self"}"#.to_owned()
    } else {
        format!(r#"{{"select":"{select}","side":"opposing"}}"#)
    }
}

fn op_with(name: &str, select: &str) -> String {
    let target = selector_json(select);
    match name {
        "attack" => format!(r#"{{"op":"attack","target":{target},"amount":3}}"#),
        "deal_damage" => format!(r#"{{"op":"deal_damage","target":{target},"amount":2}}"#),
        "draw" => format!(r#"{{"op":"draw","target":{target},"count":1}}"#),
        "modify_stat" => {
            format!(r#"{{"op":"modify_stat","target":{target},"stat":"force","delta":1}}"#)
        }
        _ => format!(r#"{{"op":"grant_keyword","target":{target},"keyword":"ranged"}}"#),
    }
}

fn effect_with(trigger: &str, op: &str) -> String {
    match trigger {
        "activated" => format!(r#"{{"trigger":"activated","cost":2,"ops":[{op}]}}"#),
        t => format!(r#"{{"trigger":"{t}","ops":[{op}]}}"#),
    }
}

/// The table of the catalog (op -> triggers, selectors), written out independently of the code.
fn allowed_triggers(name: &str) -> &'static [&'static str] {
    match name {
        "attack" | "deal_damage" | "draw" => &["on_play", "activated"],
        _ => &["continuous"],
    }
}
fn allowed_selectors(name: &str) -> &'static [&'static str] {
    match name {
        "attack" => &["hero_or_unit"],
        "deal_damage" => &["hero_or_unit", "unit", "all_units"],
        "draw" => &["hero"],
        _ => &["self", "unit", "all_units"],
    }
}

#[test]
fn trigger_not_allowed_for_the_operation() {
    for (name, _) in OPS {
        let select = allowed_selectors(name)[0];
        for trigger in ["on_play", "activated", "continuous"] {
            let json = unit_json(&[effect_with(trigger, &op_with(name, select))]);
            if allowed_triggers(name).contains(&trigger) {
                accepted(&json);
            } else {
                assert_eq!(
                    refused(&json),
                    CardsError::TriggerNotAllowed {
                        effect: 0,
                        op: name_of(name),
                        trigger: trigger_name(trigger)
                    },
                    "{name} {trigger}"
                );
            }
        }
    }
}

fn name_of(name: &str) -> &'static str {
    OPS.iter()
        .find(|(n, _)| *n == name)
        .map(|(_, s)| *s)
        .unwrap()
}
fn trigger_name(name: &str) -> &'static str {
    ["on_play", "activated", "continuous"]
        .into_iter()
        .find(|t| *t == name)
        .unwrap()
}
fn select_name(name: &str) -> &'static str {
    ["self", "hero", "unit", "hero_or_unit", "all_units"]
        .into_iter()
        .find(|s| *s == name)
        .unwrap()
}

#[test]
fn selector_not_allowed_for_the_operation() {
    // One line per case of the table: 5 operations x 5 selectors.
    for (name, _) in OPS {
        let trigger = allowed_triggers(name)[0];
        for select in ["self", "hero", "unit", "hero_or_unit", "all_units"] {
            let json = unit_json(&[effect_with(trigger, &op_with(name, select))]);
            if allowed_selectors(name).contains(&select) {
                accepted(&json);
            } else {
                assert_eq!(
                    refused(&json),
                    CardsError::SelectorNotAllowed {
                        effect: 0,
                        op: name_of(name),
                        select: select_name(select)
                    },
                    "{name} {select}"
                );
            }
        }
    }
}

#[test]
fn attack_side_must_be_opposing() {
    let attack = |side: &str| {
        format!(
            r#"{{"op":"attack","target":{{"select":"hero_or_unit","side":"{side}"}},"amount":3}}"#
        )
    };
    accepted(&unit_json(&[activated(&[&attack("opposing")])]));
    for side in ["own", "both"] {
        assert_eq!(
            unit_refused(&[activated(&[&attack(side)])]),
            CardsError::SideNotAllowed {
                effect: 0,
                op: "attack"
            },
            "{side}"
        );
    }
    // The restriction is the one of `attack` only.
    for side in ["own", "opposing", "both"] {
        let deal = format!(
            r#"{{"op":"deal_damage","target":{{"select":"unit","side":"{side}"}},"amount":2}}"#
        );
        accepted(&unit_json(&[activated(&[&deal])]));
    }
}

// ---- values ----------------------------------------------------------------------------------

fn value_op(name: &str, field: &str, value: u32) -> String {
    match name {
        "attack" => format!(
            r#"{{"op":"attack","target":{{"select":"hero_or_unit","side":"opposing"}},"{field}":{value}}}"#
        ),
        "deal_damage" => format!(
            r#"{{"op":"deal_damage","target":{{"select":"hero_or_unit","side":"opposing"}},"{field}":{value}}}"#
        ),
        _ => format!(
            r#"{{"op":"draw","target":{{"select":"hero","side":"own"}},"{field}":{value}}}"#
        ),
    }
}

#[test]
fn amount_and_count_at_least_one() {
    for (name, field) in [
        ("attack", "amount"),
        ("deal_damage", "amount"),
        ("draw", "count"),
    ] {
        let card = |value: u32| unit_json(&[on_play(&[&value_op(name, field, value)])]);
        assert_eq!(
            refused(&card(0)),
            CardsError::OpValueBelowOne {
                effect: 0,
                op: name_of(name),
                field
            },
            "{name}"
        );
        accepted(&card(1));
        accepted(&card(MAX_STAT));
        assert_eq!(
            refused(&card(MAX_STAT + 1)),
            CardsError::EffectStatTooHigh {
                effect: 0,
                field,
                value: MAX_STAT + 1,
                max: MAX_STAT
            },
            "{name}"
        );
    }
}

#[test]
fn delta_zero_and_out_of_bounds() {
    let card = |delta: i64| {
        let op = format!(
            r#"{{"op":"modify_stat","target":{{"select":"self"}},"stat":"force","delta":{delta}}}"#
        );
        unit_json(&[continuous(&[&op])])
    };
    assert_eq!(
        refused(&card(0)),
        CardsError::DeltaZero {
            effect: 0,
            op: "modify_stat"
        }
    );
    let max = i64::from(MAX_STAT);
    for delta in [1, -1, max, -max] {
        accepted(&card(delta));
    }
    for (delta, value) in [
        (max + 1, MAX_STAT + 1),
        (-max - 1, MAX_STAT + 1),
        (i64::from(i32::MAX), i32::MAX.unsigned_abs()),
        (i64::from(i32::MIN), i32::MIN.unsigned_abs()),
    ] {
        assert_eq!(
            refused(&card(delta)),
            CardsError::EffectStatTooHigh {
                effect: 0,
                field: "delta",
                value,
                max: MAX_STAT
            },
            "{delta}"
        );
    }
}

fn attack_with(extra: &str) -> String {
    format!(
        r#"{{"op":"attack","target":{{"select":"hero_or_unit","side":"opposing"}},"amount":3{extra}}}"#
    )
}

#[test]
fn attackers_out_of_range() {
    for attackers in [1, 2] {
        let mode = if attackers == 2 {
            r#","mode":"simultaneous""#
        } else {
            ""
        };
        accepted(&unit_json(&[activated(&[&attack_with(&format!(
            r#","attackers":{attackers}{mode}"#
        ))])]));
    }
    for found in [0, 3, 8, u32::MAX] {
        assert_eq!(
            unit_refused(&[activated(&[&attack_with(&format!(
                r#","attackers":{found},"mode":"chained""#
            ))])]),
            CardsError::AttackersOutOfRange { effect: 0, found },
            "{found}"
        );
    }
}

#[test]
fn mode_required_exactly_when_two_attackers() {
    let eff = |extra: &str| unit_json(&[activated(&[&attack_with(extra)])]);
    for mode in ["simultaneous", "chained"] {
        accepted(&eff(&format!(r#","attackers":2,"mode":"{mode}""#)));
    }
    assert_eq!(
        refused(&eff(r#","attackers":2"#)),
        CardsError::AttackModeMissing { effect: 0 }
    );
    // `attackers` omitted means 1: a mode is refused.
    for extra in [
        r#","mode":"chained""#,
        r#","attackers":1,"mode":"simultaneous""#,
    ] {
        assert_eq!(
            refused(&eff(extra)),
            CardsError::AttackModeUnexpected { effect: 0 },
            "{extra}"
        );
    }
    accepted(&eff(r#","attackers":1"#));
    // The range is checked before the mode.
    assert_eq!(
        refused(&eff(r#","attackers":0,"mode":"chained""#)),
        CardsError::AttackersOutOfRange {
            effect: 0,
            found: 0
        }
    );
}

#[test]
fn too_many_expected_picks() {
    // 3 picks (two attackers and a target) is the maximum.
    let two = attack_with(r#","attackers":2,"mode":"chained""#);
    accepted(&unit_json(&[activated(&[&two])]));
    accepted(&spell_json(2, &[on_play(&[&two])]));
    accepted(&unit_json(&[on_play(&[DEAL_UNIT, DEAL_UNIT, DEAL_UNIT])]));
    // 4 picks: two attackers, a target, and another target.
    assert_eq!(
        unit_refused(&[activated(&[&two, DEAL_UNIT])]),
        CardsError::TooManyExpectedPicks {
            effect: 0,
            found: 4,
            max: MAX_EFFECT_PICKS
        }
    );
    // A spell attack with one attacker still asks for the attacker: 2 + 1 + 1 targets.
    let one = attack_with("");
    accepted(&spell_json(2, &[on_play(&[&one, DEAL_UNIT])]));
    assert_eq!(
        refused(&spell_json(2, &[on_play(&[&one, DEAL_UNIT, DEAL_UNIT])])),
        CardsError::TooManyExpectedPicks {
            effect: 0,
            found: 4,
            max: MAX_EFFECT_PICKS
        }
    );
}

// ---- immobile units, spells ------------------------------------------------------------------

#[test]
fn immobile_unit_cannot_declare_an_attack_effect() {
    for effect in [activated(&[ATTACK]), on_play(&[ATTACK])] {
        assert_eq!(
            refused(&immobile_json(std::slice::from_ref(&effect))),
            CardsError::ImmobileWithAttackEffect { effect: 0 }
        );
        // Among other operations and at another index.
        assert_eq!(
            refused(&immobile_json(&[on_play(&[DRAW]), effect])),
            CardsError::ImmobileWithAttackEffect { effect: 1 }
        );
    }
    // The same effect is fine on a unit that attacks.
    accepted(&unit_json(&[activated(&[ATTACK])]));
}

#[test]
fn immobile_unit_can_declare_other_effects() {
    accepted(&immobile_json(&[
        on_play(&[DRAW]),
        activated(&[DEAL]),
        continuous(&[MODIFY, GRANT]),
    ]));
}

#[test]
fn a_spell_has_at_least_one_effect() {
    assert_eq!(
        refused(&spell_json(2, &[])),
        CardsError::SpellWithoutEffects
    );
    // `effects` absent reads as empty, then fails the same way.
    assert_eq!(
        refused(r#"{"kind":"spell","id":"a","name":"A","cost":2}"#),
        CardsError::SpellWithoutEffects
    );
    accepted(&spell_json(2, &[on_play(&[DEAL])]));
}

#[test]
fn a_spell_effect_must_be_on_play() {
    for (effect, expected) in [(activated(&[DEAL]), 1), (continuous(&[MODIFY]), 1)] {
        assert_eq!(
            refused(&spell_json(2, &[on_play(&[DEAL]), effect])),
            CardsError::SpellEffectNotOnPlay { effect: expected }
        );
    }
    accepted(&spell_json(2, &[on_play(&[DEAL]), on_play(&[DRAW])]));
}

#[test]
fn a_spell_cost_is_bounded_like_a_unit_cost() {
    let effects = [on_play(&[DEAL])];
    accepted(&spell_json(1, &effects));
    accepted(&spell_json(MAX_STAT, &effects));
    assert_eq!(
        refused(&spell_json(0, &effects)),
        CardsError::CostBelowOne { field: "cost" }
    );
    assert_eq!(
        refused(&spell_json(MAX_STAT + 1, &effects)),
        CardsError::StatTooHigh {
            field: "cost",
            value: MAX_STAT + 1,
            max: MAX_STAT
        }
    );
}

#[test]
fn a_spell_checks_its_cost_then_its_triggers_then_its_bounds_then_its_tags() {
    // Cost before anything else.
    assert_eq!(
        refused(&spell_json(0, &[])),
        CardsError::CostBelowOne { field: "cost" }
    );
    // No effect before the triggers; the triggers before the bounds of the effects.
    let five = vec![on_play(&[DRAW]); 4];
    let mut mixed = five.clone();
    mixed.push(activated(&[DEAL]));
    assert_eq!(
        refused(&spell_json(2, &mixed)),
        CardsError::SpellEffectNotOnPlay { effect: 4 }
    );
    // Tags last.
    let tagged = format!(
        r#"{{"kind":"spell","id":"a","name":"A","cost":2,"tags":["inconnue"],"effects":[{}]}}"#,
        on_play(&[DRAW])
    );
    assert_eq!(refused(&tagged), CardsError::UnknownTag("inconnue".into()));
    let bad = format!(
        r#"{{"kind":"spell","id":"a","name":"A","cost":2,"tags":["bâtiment"],"effects":[{}]}}"#,
        on_play(&[])
    );
    assert_eq!(refused(&bad), CardsError::EffectWithoutOps { effect: 0 });
    let many = format!(
        r#"{{"kind":"spell","id":"a","name":"A","cost":2,"tags":["a","b","c","d","e"],"effects":[{}]}}"#,
        on_play(&[DRAW])
    );
    assert!(matches!(refused(&many), CardsError::TooManyTags { .. }));
}

#[test]
fn the_checks_of_an_effect_run_in_the_documented_order() {
    // Count of ops, then cost, then trigger, selector, side, values, attack parameters, picks.
    let bad_cost = r#"{"trigger":"activated","cost":0,"ops":[{"op":"draw","target":{"select":"hero","side":"own"},"count":0}]}"#;
    assert_eq!(
        unit_refused(&[bad_cost.to_owned()]),
        CardsError::EffectCostBelowOne { effect: 0 }
    );
    let bad_trigger = r#"{"trigger":"continuous","ops":[{"op":"draw","target":{"select":"unit","side":"own"},"count":0}]}"#;
    assert!(matches!(
        unit_refused(&[bad_trigger.to_owned()]),
        CardsError::TriggerNotAllowed { .. }
    ));
    let bad_selector = r#"{"trigger":"on_play","ops":[{"op":"draw","target":{"select":"unit","side":"own"},"count":0}]}"#;
    assert!(matches!(
        unit_refused(&[bad_selector.to_owned()]),
        CardsError::SelectorNotAllowed { .. }
    ));
    let bad_side = r#"{"trigger":"on_play","ops":[{"op":"attack","target":{"select":"hero_or_unit","side":"own"},"amount":0}]}"#;
    assert!(matches!(
        unit_refused(&[bad_side.to_owned()]),
        CardsError::SideNotAllowed { .. }
    ));
    let bad_value = r#"{"trigger":"on_play","ops":[{"op":"attack","target":{"select":"hero_or_unit","side":"opposing"},"amount":0,"attackers":3}]}"#;
    assert!(matches!(
        unit_refused(&[bad_value.to_owned()]),
        CardsError::OpValueBelowOne { .. }
    ));
    // Picks before the immobile check.
    let two = attack_with(r#","attackers":2,"mode":"chained""#);
    assert_eq!(
        refused(&immobile_json(&[activated(&[&two, DEAL_UNIT])])),
        CardsError::TooManyExpectedPicks {
            effect: 0,
            found: 4,
            max: MAX_EFFECT_PICKS
        }
    );
}

// ---- outside the closed sets -----------------------------------------------------------------

#[test]
fn everything_outside_the_closed_sets_is_refused() {
    let heal = r#"{"op":"heal","target":{"select":"hero","side":"own"},"amount":2}"#;
    let shield = r#"{"op":"shield","target":{"select":"unit","side":"own"},"amount":2}"#;
    let cases = [
        unit_json(&[on_play(&[heal])]),
        unit_json(&[on_play(&[shield])]),
        unit_json(&[format!(r#"{{"trigger":"on_event","ops":[{DRAW}]}}"#)]),
        unit_json(&[format!(
            r#"{{"trigger":"on_play","when":"x","ops":[{DRAW}]}}"#
        )]),
        r#"{"kind":"building","id":"a","name":"A","cost":2,"health":5}"#.to_owned(),
        r#"{"kind":"equipment","id":"a","name":"A","cost":2,"effects":[]}"#.to_owned(),
        unit_json(&[continuous(&[
            r#"{"op":"grant_keyword","target":{"select":"self"},"keyword":"flying"}"#,
        ])]),
        unit_json(&[continuous(&[
            r#"{"op":"modify_stat","target":{"select":"self"},"stat":"health","delta":1}"#,
        ])]),
        unit_json(&[on_play(&[
            r#"{"op":"draw","target":{"select":"hero","side":"own"},"count":1,"extra":1}"#,
        ])]),
        // `self` carries no side.
        unit_json(&[continuous(&[
            r#"{"op":"modify_stat","target":{"select":"self","side":"own"},"stat":"force","delta":1}"#,
        ])]),
        // A selector that does not exist, a side that does not exist, a duration written.
        unit_json(&[on_play(&[
            r#"{"op":"draw","target":{"select":"player","side":"own"},"count":1}"#,
        ])]),
        unit_json(&[on_play(&[
            r#"{"op":"draw","target":{"select":"hero","side":"enemy"},"count":1}"#,
        ])]),
        format!(
            r#"{{"kind":"unit","id":"a","name":"A","cost":2,"force":3,"attack_cost":2,"health":5,"effects":[{{"trigger":"on_play","duration":"instant","ops":[{DRAW}]}}]}}"#
        ),
    ];
    for json in cases {
        assert!(
            matches!(parse(&json), Err(CardsError::Json(_))),
            "{json}: {:?}",
            parse(&json)
        );
    }
}

#[test]
fn an_unknown_op_field_is_refused() {
    // D1: the cost belongs to the effect, never to the operation.
    let with_cost = attack_with(r#","cost":2"#);
    assert!(matches!(
        parse(&unit_json(&[activated(&[&with_cost])])),
        Err(CardsError::Json(_))
    ));
    let unknown = attack_with(r#","bonus":1"#);
    assert!(matches!(
        parse(&unit_json(&[activated(&[&unknown])])),
        Err(CardsError::Json(_))
    ));
    // A field of another operation.
    let wrong = r#"{"op":"draw","target":{"select":"hero","side":"own"},"count":1,"amount":2}"#;
    assert!(matches!(
        parse(&unit_json(&[on_play(&[wrong])])),
        Err(CardsError::Json(_))
    ));
}

// ---- Phi class, polarity, duration, picks ----------------------------------------------------

#[test]
fn every_operation_has_a_phi_class_and_the_table_gives_corollaries() {
    for (json, class) in [
        (ATTACK, PhiClass::D),
        (DEAL, PhiClass::D),
        (DRAW, PhiClass::D),
        (MODIFY, PhiClass::S),
        (GRANT, PhiClass::S),
    ] {
        let op = op(json);
        assert_eq!(op.phi_class(), class, "{json}");
        assert_eq!(op.kind().phi_class(), class, "{json}");
    }
    // Corollaries: `continuous` carries S operations only, `on_play` and `activated` D only.
    for kind in OpKind::ALL {
        for trigger in kind.triggers() {
            let expected = if *trigger == Trigger::Continuous {
                PhiClass::S
            } else {
                PhiClass::D
            };
            assert_eq!(kind.phi_class(), expected, "{kind:?} {trigger:?}");
        }
    }
    // No operation of wave 1 is of class N.
    assert!(OpKind::ALL.iter().all(|k| k.phi_class() != PhiClass::N));
}

#[test]
fn the_operation_tables_are_the_ones_of_the_catalog() {
    use SelectorKind::{AllUnits, Hero, HeroOrUnit, Source, Unit};
    use Trigger::{Activated, Continuous, OnPlay};
    assert_eq!(OpKind::Attack.triggers(), [OnPlay, Activated]);
    assert_eq!(OpKind::Attack.selectors(), [HeroOrUnit]);
    assert_eq!(OpKind::DealDamage.triggers(), [OnPlay, Activated]);
    assert_eq!(OpKind::DealDamage.selectors(), [HeroOrUnit, Unit, AllUnits]);
    assert_eq!(OpKind::Draw.triggers(), [OnPlay, Activated]);
    assert_eq!(OpKind::Draw.selectors(), [Hero]);
    assert_eq!(OpKind::ModifyStat.triggers(), [Continuous]);
    assert_eq!(OpKind::ModifyStat.selectors(), [Source, Unit, AllUnits]);
    assert_eq!(OpKind::GrantKeyword.triggers(), [Continuous]);
    assert_eq!(OpKind::GrantKeyword.selectors(), [Source, Unit, AllUnits]);
    assert_eq!(OpKind::ALL.len(), 5);
    assert_eq!(SelectorKind::ALL.len(), 5);
    for (kind, name) in OpKind::ALL.iter().zip(duel_cards::OP_NAMES) {
        assert_eq!(kind.name(), name);
    }
    for (kind, name) in SelectorKind::ALL.iter().zip(duel_cards::SELECT_NAMES) {
        assert_eq!(kind.name(), name);
    }
}

fn modify(select: &str, side: &str, stat: &str, delta: i32) -> Op {
    let target = if select == "self" {
        r#"{"select":"self"}"#.to_owned()
    } else {
        format!(r#"{{"select":"{select}","side":"{side}"}}"#)
    };
    op(&format!(
        r#"{{"op":"modify_stat","target":{target},"stat":"{stat}","delta":{delta}}}"#
    ))
}

#[test]
fn polarity_is_derived_from_sign_and_side() {
    use Polarity::{Bonus, Malus, Neutral};
    let sided = |name: &str, select: &str, side: &str| {
        let target = format!(r#"{{"select":"{select}","side":"{side}"}}"#);
        match name {
            "deal_damage" => op(&format!(
                r#"{{"op":"deal_damage","target":{target},"amount":2}}"#
            )),
            "draw" => op(&format!(r#"{{"op":"draw","target":{target},"count":1}}"#)),
            "attack" => op(&format!(
                r#"{{"op":"attack","target":{target},"amount":2}}"#
            )),
            _ => op(&format!(
                r#"{{"op":"grant_keyword","target":{target},"keyword":"ranged"}}"#
            )),
        }
    };
    // Negative for the affected (damage): good for the owner on the opposing side.
    for (name, select) in [
        ("attack", "hero_or_unit"),
        ("deal_damage", "hero_or_unit"),
        ("deal_damage", "unit"),
        ("deal_damage", "all_units"),
    ] {
        assert_eq!(sided(name, select, "opposing").polarity(), Bonus, "{name}");
        assert_eq!(sided(name, select, "own").polarity(), Malus, "{name}");
        assert_eq!(sided(name, select, "both").polarity(), Neutral, "{name}");
    }
    // Positive for the affected: drawing, a keyword.
    for (name, select) in [("draw", "hero"), ("grant_keyword", "unit")] {
        assert_eq!(sided(name, select, "own").polarity(), Bonus, "{name}");
        assert_eq!(sided(name, select, "opposing").polarity(), Malus, "{name}");
        assert_eq!(sided(name, select, "both").polarity(), Neutral, "{name}");
    }
    assert_eq!(
        op(GRANT).polarity(),
        Bonus,
        "`self` is the owner side: a keyword on the source is a bonus"
    );
    // A stat: the sign of the delta, inverted for the costs.
    for (stat, sign) in [
        ("force", 1),
        ("riposte", 1),
        ("cost", -1),
        ("attack_cost", -1),
    ] {
        // `sign` is the value for the affected of a positive delta.
        let (up, down) = (sign, -sign);
        let pol = |value: i32| if value > 0 { Bonus } else { Malus };
        for (delta, value) in [(2, up), (-2, down)] {
            assert_eq!(
                modify("unit", "own", stat, delta).polarity(),
                pol(value),
                "{stat} {delta} own"
            );
            assert_eq!(
                modify("unit", "opposing", stat, delta).polarity(),
                if value > 0 { Malus } else { Bonus },
                "{stat} {delta} opposing"
            );
            assert_eq!(
                modify("all_units", "both", stat, delta).polarity(),
                Neutral,
                "{stat} {delta} both"
            );
            // `self` is the owner side.
            assert_eq!(
                modify("self", "own", stat, delta).polarity(),
                pol(value),
                "{stat} {delta} self"
            );
        }
    }
    // Spelled out for the two directions that surprise: a cost that goes up is bad for its owner,
    // +2 force on the opponent is a malus for the owner of the card.
    assert_eq!(modify("self", "own", "cost", 1).polarity(), Malus);
    assert_eq!(modify("unit", "opposing", "force", 2).polarity(), Malus);
    assert_eq!(
        modify("unit", "opposing", "attack_cost", 1).polarity(),
        Bonus
    );
}

#[test]
fn polarity_of_a_zero_delta_is_neutral() {
    // Not a valid card (`DeltaZero`), but the function is total.
    assert_eq!(
        modify("unit", "own", "force", 0).polarity(),
        Polarity::Neutral
    );
}

#[test]
fn duration_is_derived_from_the_trigger() {
    assert_eq!(Trigger::OnPlay.duration(), Duration::Instant);
    assert_eq!(Trigger::Activated.duration(), Duration::Instant);
    assert_eq!(Trigger::Continuous.duration(), Duration::WhileSourceInPlay);
    // Never stored, never read.
    let json = serde_json::to_string(&effect(&on_play(&[DRAW]))).unwrap();
    assert!(!json.contains("duration"), "{json}");
    assert!(
        serde_json::from_str::<CardEffect>(&format!(
            r#"{{"trigger":"on_play","duration":"instant","ops":[{DRAW}]}}"#
        ))
        .is_err()
    );
}

fn slot(role: PickRole, kind: PickKind, side: Side) -> PickSlot {
    PickSlot { role, kind, side }
}

#[test]
fn expected_picks_lists_attackers_then_targets_in_order() {
    use PickKind::{HeroOrUnit, Unit};
    use PickRole::{Attacker, Target};
    let own_attacker = slot(Attacker, Unit, Side::Own);
    let target = slot(Target, HeroOrUnit, Side::Opposing);
    let two = effect(&activated(&[&attack_with(
        r#","attackers":2,"mode":"chained""#,
    )]));
    assert_eq!(
        two.expected_picks(false),
        [own_attacker, own_attacker, target]
    );
    assert_eq!(
        two.expected_picks(true),
        [own_attacker, own_attacker, target]
    );
    // One attacker: a unit is its own attacker (no slot), a spell needs one.
    let one = effect(&on_play(&[ATTACK]));
    assert_eq!(one.expected_picks(false), [target]);
    assert_eq!(one.expected_picks(true), [own_attacker, target]);
    // In the order of the operations, attackers before the target of the same operation.
    let both = effect(&activated(&[
        &attack_with(r#","attackers":2,"mode":"simultaneous""#),
        DEAL_UNIT,
    ]));
    assert_eq!(
        both.expected_picks(false),
        [
            own_attacker,
            own_attacker,
            target,
            slot(Target, Unit, Side::Opposing)
        ]
    );
    let reversed = effect(&activated(&[DEAL_UNIT, ATTACK]));
    assert_eq!(
        reversed.expected_picks(true),
        [slot(Target, Unit, Side::Opposing), own_attacker, target]
    );
}

#[test]
fn expected_picks_for_each_selector() {
    // `self`, `hero` and `all_units` ask for no pick; `unit` and `hero_or_unit` ask for one.
    let picks = |op: &str| effect(&on_play(&[op])).expected_picks(false);
    assert!(picks(DRAW).is_empty());
    assert!(picks(&op_with("deal_damage", "all_units")).is_empty());
    for side in ["own", "opposing", "both"] {
        let side_enum = match side {
            "own" => Side::Own,
            "opposing" => Side::Opposing,
            _ => Side::Both,
        };
        let unit = format!(
            r#"{{"op":"deal_damage","target":{{"select":"unit","side":"{side}"}},"amount":1}}"#
        );
        assert_eq!(
            picks(&unit),
            [slot(PickRole::Target, PickKind::Unit, side_enum)]
        );
        let hero_or_unit = format!(
            r#"{{"op":"deal_damage","target":{{"select":"hero_or_unit","side":"{side}"}},"amount":1}}"#
        );
        assert_eq!(
            picks(&hero_or_unit),
            [slot(PickRole::Target, PickKind::HeroOrUnit, side_enum)]
        );
    }
    let on_self = effect(&continuous(&[MODIFY]));
    assert!(on_self.expected_picks(false).is_empty());
}

// ---- wire form -------------------------------------------------------------------------------

#[test]
fn an_effect_round_trips_through_json_and_omits_defaults() {
    // Defaults omitted: ignores_provocation, scales_with_force, attackers = 1, no mode.
    let plain = format!(r#"{{"trigger":"activated","cost":2,"ops":[{ATTACK}]}}"#);
    assert_eq!(serde_json::to_string(&effect(&plain)).unwrap(), plain);
    // Everything written, in the order of the spec.
    let full = r#"{"trigger":"activated","cost":2,"ops":[{"op":"attack","target":{"select":"hero_or_unit","side":"opposing"},"amount":3,"ignores_provocation":true,"scales_with_force":true,"attackers":2,"mode":"simultaneous"}]}"#;
    assert_eq!(serde_json::to_string(&effect(full)).unwrap(), full);
    // Explicit defaults read the same and are written back without them.
    let explicit = r#"{"trigger":"activated","cost":2,"ops":[{"op":"attack","target":{"select":"hero_or_unit","side":"opposing"},"amount":3,"ignores_provocation":false,"scales_with_force":false,"attackers":1}]}"#;
    assert_eq!(effect(explicit), effect(&plain));
    // No cost key when there is no cost; `self` has no side.
    let cont = r#"{"trigger":"continuous","ops":[{"op":"modify_stat","target":{"select":"self"},"stat":"attack_cost","delta":-1},{"op":"grant_keyword","target":{"select":"all_units","side":"own"},"keyword":"ranged"}]}"#;
    assert_eq!(serde_json::to_string(&effect(cont)).unwrap(), cont);
    for json in [
        on_play(&[DRAW]),
        on_play(&[DEAL, DEAL_UNIT]),
        full.to_owned(),
        cont.to_owned(),
    ] {
        let parsed = effect(&json);
        let again: CardEffect =
            serde_json::from_str(&serde_json::to_string(&parsed).unwrap()).unwrap();
        assert_eq!(parsed, again, "{json}");
    }
    assert!(matches!(
        effect(full).ops[0],
        Op::Attack {
            attackers: 2,
            mode: Some(AttackMode::Simultaneous),
            ignores_provocation: true,
            scales_with_force: true,
            ..
        }
    ));
}

#[test]
fn the_closed_sets_have_the_expected_variants() {
    assert_eq!(Keyword::Ranged, serde_json::from_str("\"ranged\"").unwrap());
    assert_eq!(
        Stat::AttackCost,
        serde_json::from_str("\"attack_cost\"").unwrap()
    );
    assert_eq!(
        Selector::Source {},
        serde_json::from_str(r#"{"select":"self"}"#).unwrap()
    );
    assert_eq!(
        Selector::HeroOrUnit {
            side: Side::Opposing
        },
        serde_json::from_str(r#"{"select":"hero_or_unit","side":"opposing"}"#).unwrap()
    );
}

const PLAIN_UNIT: &str = r#"{"kind":"unit","id":"a","name":"A","cost":2,"force":3,"attack_cost":2,"health":5,"provocation":false,"riposte":0}"#;

#[test]
fn a_unit_without_effects_writes_the_same_bytes_as_before() {
    let cards = parse(&unit_json(&[])).unwrap();
    assert_eq!(serde_json::to_string(&cards[0]).unwrap(), PLAIN_UNIT);
    // The spec wire form too.
    assert_eq!(
        serde_json::to_string(&cards[0].spec()).unwrap(),
        r#"{"cost":2,"force":3,"attack_cost":2,"health":5,"provocation":false,"riposte":0}"#
    );
    // And the frozen sets re-serialize without any `effects` key.
    for json in [
        include_str!("../../../data/fixture/1.0.0/cards.json"),
        include_str!("../../../data/starter/1.0.0/cards.json"),
        include_str!("../../../data/starter/1.1.0/cards.json"),
    ] {
        let written = serde_json::to_string(&duel_cards::parse_cards(json).unwrap()).unwrap();
        assert!(!written.contains("effects"));
        assert!(!written.contains("spell"));
    }
}

#[test]
fn the_spec_wire_of_a_spell_and_of_a_unit_with_effects_round_trips() {
    let spell = r#"{"kind":"spell","cost":3,"effects":[{"trigger":"on_play","ops":[{"op":"deal_damage","target":{"select":"hero_or_unit","side":"opposing"},"amount":2}]}]}"#;
    let parsed: CardSpec = serde_json::from_str(spell).unwrap();
    assert!(matches!(parsed, CardSpec::Spell(_)));
    assert_eq!(serde_json::to_string(&parsed).unwrap(), spell);
    // With tags, written between the cost and the effects; omitted when empty.
    let tagged = r#"{"kind":"spell","cost":3,"tags":["magie"],"effects":[{"trigger":"on_play","ops":[{"op":"draw","target":{"select":"hero","side":"own"},"count":1}]}]}"#;
    let parsed: CardSpec = serde_json::from_str(tagged).unwrap();
    assert_eq!(serde_json::to_string(&parsed).unwrap(), tagged);
    // A unit with effects: `effects` last, no `kind`.
    let unit = r#"{"cost":2,"force":3,"attack_cost":2,"health":5,"provocation":false,"riposte":0,"immobile":false,"tags":[],"effects":[{"trigger":"on_play","ops":[{"op":"draw","target":{"select":"hero","side":"own"},"count":1}]}]}"#;
    let parsed: CardSpec = serde_json::from_str(unit).unwrap();
    let written = serde_json::to_string(&parsed).unwrap();
    assert_eq!(
        written,
        r#"{"cost":2,"force":3,"attack_cost":2,"health":5,"provocation":false,"riposte":0,"effects":[{"trigger":"on_play","ops":[{"op":"draw","target":{"select":"hero","side":"own"},"count":1}]}]}"#
    );
    assert_eq!(serde_json::from_str::<CardSpec>(&written).unwrap(), parsed);
    // `kind` may be written on a unit (`unit`, `creature`) and is read; a spell cannot read
    // as a unit.
    let kind_unit = r#"{"kind":"unit","cost":2,"force":3,"attack_cost":2,"health":5}"#;
    assert!(matches!(
        serde_json::from_str::<CardSpec>(kind_unit).unwrap(),
        CardSpec::Unit(_)
    ));
    // The struct form of the spec is the public API.
    let built = CardSpec::Spell(SpellSpec {
        cost: 3,
        tags: vec![],
        effects: vec![effect(&on_play(&[DEAL]))],
    });
    assert_eq!(validate_card_spec(&built), Ok(()));
    assert_eq!(
        serde_json::from_str::<CardSpec>(&serde_json::to_string(&built).unwrap()).unwrap(),
        built
    );
}

#[test]
fn a_spell_spec_with_a_unit_field_is_refused_by_serde() {
    let effects = format!(r#""effects":[{}]"#, on_play(&[DEAL]));
    for (field, value) in [
        ("force", "1"),
        ("force", "0"),
        ("attack_cost", "1"),
        ("health", "1"),
        ("health", "0"),
        ("provocation", "false"),
        ("riposte", "0"),
        ("immobile", "false"),
        ("immobile", "true"),
    ] {
        let json = format!(r#"{{"kind":"spell","cost":2,"{field}":{value},{effects}}}"#);
        let error = serde_json::from_str::<CardSpec>(&json)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("unknown field `{field}`")),
            "{json}: {error}"
        );
    }
    // The same on a card of a data set.
    for field in [
        "force",
        "attack_cost",
        "health",
        "provocation",
        "riposte",
        "immobile",
    ] {
        let json =
            format!(r#"{{"kind":"spell","id":"a","name":"A","cost":2,"{field}":1,{effects}}}"#);
        assert!(matches!(parse(&json), Err(CardsError::Json(_))), "{field}");
    }
    // `cost` is required, `kind` must be known, `effects` absent reads as empty.
    assert!(
        serde_json::from_str::<CardSpec>(r#"{"kind":"spell"}"#)
            .unwrap_err()
            .to_string()
            .contains("missing field `cost`")
    );
    assert!(serde_json::from_str::<CardSpec>(r#"{"kind":"building","cost":1}"#).is_err());
    let empty: CardSpec = serde_json::from_str(r#"{"kind":"spell","cost":2}"#).unwrap();
    assert_eq!(
        validate_card_spec(&empty),
        Err(CardsError::SpellWithoutEffects)
    );
}

#[test]
fn a_spell_card_reads_and_writes_its_kind() {
    let cards = parse(&spell_json(2, &[on_play(&[DEAL])])).unwrap();
    assert!(matches!(cards[0], Card::Spell { .. }));
    assert_eq!(cards[0].id(), "a");
    assert_eq!(cards[0].name(), "A");
    let written: Value = serde_json::to_value(&cards[0]).unwrap();
    assert_eq!(written["kind"], "spell");
    assert_eq!(written["cost"], 2);
    assert!(written.get("tags").is_none());
    // Its stats are the ones an instance carries: the cost and zeros.
    let stats = cards[0].stats();
    assert_eq!(
        (
            stats.cost,
            stats.force,
            stats.attack_cost,
            stats.health,
            stats.riposte
        ),
        (2, 0, 0, 0, 0)
    );
    assert!(!stats.provocation);
    // A card with `force` on a spell is a serde error.
    assert!(matches!(
        parse(r#"{"kind":"spell","id":"a","name":"A","cost":2,"force":1,"effects":[]}"#),
        Err(CardsError::Json(_))
    ));
    // The spec of a spell is not a plain unit and has no creature stats.
    let spec = cards[0].spec();
    assert!(!spec.is_plain_unit());
    assert!(spec.creature().is_none());
}

#[test]
fn a_unit_with_effects_keeps_its_stats_and_is_not_plain() {
    let cards = parse(&unit_json(&[on_play(&[DRAW])])).unwrap();
    let spec = cards[0].spec();
    assert!(!spec.is_plain_unit());
    assert_eq!(spec.creature(), Some(&cards[0].stats()));
}

// ---- the name lists are pinned against serde --------------------------------------------------

fn side_name(s: Side) -> &'static str {
    match s {
        Side::Own => "own",
        Side::Opposing => "opposing",
        Side::Both => "both",
    }
}
fn trigger_literal(t: Trigger) -> &'static str {
    match t {
        Trigger::OnPlay => "on_play",
        Trigger::Activated => "activated",
        Trigger::Continuous => "continuous",
    }
}
fn duration_name(d: Duration) -> &'static str {
    match d {
        Duration::Instant => "instant",
        Duration::WhileSourceInPlay => "while_source_in_play",
    }
}
fn keyword_name(k: Keyword) -> &'static str {
    match k {
        Keyword::Ranged => "ranged",
    }
}
fn stat_name(s: Stat) -> &'static str {
    match s {
        Stat::Force => "force",
        Stat::Riposte => "riposte",
        Stat::Cost => "cost",
        Stat::AttackCost => "attack_cost",
    }
}
fn mode_name(m: AttackMode) -> &'static str {
    match m {
        AttackMode::Simultaneous => "simultaneous",
        AttackMode::Chained => "chained",
    }
}
fn selector_literal(s: &Selector) -> &'static str {
    match s {
        Selector::Source {} => "self",
        Selector::Hero { .. } => "hero",
        Selector::Unit { .. } => "unit",
        Selector::HeroOrUnit { .. } => "hero_or_unit",
        Selector::AllUnits { .. } => "all_units",
    }
}
fn op_literal(o: &Op) -> &'static str {
    match o {
        Op::Attack { .. } => "attack",
        Op::DealDamage { .. } => "deal_damage",
        Op::Draw { .. } => "draw",
        Op::ModifyStat { .. } => "modify_stat",
        Op::GrantKeyword { .. } => "grant_keyword",
    }
}

/// Serializes a unit variant to its bare string.
fn wire<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value).unwrap() {
        Value::String(s) => s,
        other => panic!("not a string: {other}"),
    }
}

#[test]
fn the_name_lists_are_pinned_against_the_serialization_of_every_variant() {
    use duel_cards::{
        DURATION_NAMES, KEYWORD_NAMES, MODE_NAMES, OP_NAMES, SELECT_NAMES, SIDE_NAMES, STAT_NAMES,
        TRIGGER_NAMES,
    };
    let sides = [Side::Own, Side::Opposing, Side::Both];
    assert_eq!(sides.map(side_name), SIDE_NAMES);
    assert_eq!(sides.map(|s| wire(&s)), SIDE_NAMES);

    let triggers = [Trigger::OnPlay, Trigger::Activated, Trigger::Continuous];
    assert_eq!(triggers.map(trigger_literal), TRIGGER_NAMES);
    assert_eq!(triggers.map(|t| wire(&t)), TRIGGER_NAMES);
    assert_eq!(triggers.map(Trigger::name), TRIGGER_NAMES);

    let durations = [Duration::Instant, Duration::WhileSourceInPlay];
    assert_eq!(durations.map(duration_name), DURATION_NAMES);
    assert_eq!(durations.map(|d| wire(&d)), DURATION_NAMES);

    assert_eq!([Keyword::Ranged].map(keyword_name), KEYWORD_NAMES);
    assert_eq!([Keyword::Ranged].map(|k| wire(&k)), KEYWORD_NAMES);

    let stats = [Stat::Force, Stat::Riposte, Stat::Cost, Stat::AttackCost];
    assert_eq!(stats.map(stat_name), STAT_NAMES);
    assert_eq!(stats.map(|s| wire(&s)), STAT_NAMES);

    let modes = [AttackMode::Simultaneous, AttackMode::Chained];
    assert_eq!(modes.map(mode_name), MODE_NAMES);
    assert_eq!(modes.map(|m| wire(&m)), MODE_NAMES);

    let own = Side::Own;
    let selectors = [
        Selector::Source {},
        Selector::Hero { side: own },
        Selector::Unit { side: own },
        Selector::HeroOrUnit { side: own },
        Selector::AllUnits { side: own },
    ];
    assert_eq!(selectors.each_ref().map(selector_literal), SELECT_NAMES);
    for (selector, name) in selectors.iter().zip(SELECT_NAMES) {
        let json = serde_json::to_value(selector).unwrap();
        assert_eq!(json["select"], name);
        assert_eq!(selector.kind().name(), name);
    }

    let ops = [op(ATTACK), op(DEAL), op(DRAW), op(MODIFY), op(GRANT)];
    assert_eq!(ops.each_ref().map(op_literal), OP_NAMES);
    for (o, name) in ops.iter().zip(OP_NAMES) {
        assert_eq!(serde_json::to_value(o).unwrap()["op"], name);
        assert_eq!(o.kind().name(), name);
    }
}
