//! The fixture `tests/data/effects_cards.json` (#207): outside `data/`, so the starter set and
//! the fixture set stay untouched. It must cover every operation, trigger and statistic of the
//! closed sets: an operation without a test card fails `every_operation_has_a_fixture_card`, and
//! a variant added to `Op` without being named here fails to compile.
use duel_cards::{
    Card, DURATION_NAMES, KEYWORD_NAMES, MODE_NAMES, OP_NAMES, Op, OpKind, PhiClass, SELECT_NAMES,
    SIDE_NAMES, STAT_NAMES, Selector, Side, Stat, TRIGGER_NAMES, Trigger, parse_card_set,
};
use std::collections::BTreeSet;

const FIXTURE: &str = include_str!("data/effects_cards.json");

fn cards() -> Vec<Card> {
    parse_card_set(FIXTURE, &["bâtiment".to_owned()]).expect("the fixture validates")
}

/// `match` exhaustive and without a wildcard arm: a new operation must be named here.
fn op_name(op: &Op) -> &'static str {
    match op {
        Op::Attack { .. } => "attack",
        Op::DealDamage { .. } => "deal_damage",
        Op::Draw { .. } => "draw",
        Op::ModifyStat { .. } => "modify_stat",
        Op::GrantKeyword { .. } => "grant_keyword",
    }
}

fn stat_name(stat: Stat) -> &'static str {
    match stat {
        Stat::Force => "force",
        Stat::Riposte => "riposte",
        Stat::Cost => "cost",
        Stat::AttackCost => "attack_cost",
    }
}

fn effects_of(card: &Card) -> &[duel_cards::CardEffect] {
    match card {
        Card::Creature { effects, .. } | Card::Spell { effects, .. } => effects,
    }
}

fn all_ops() -> Vec<(Trigger, Op)> {
    cards()
        .iter()
        .flat_map(|c| effects_of(c).to_vec())
        .flat_map(|e| e.ops.into_iter().map(move |o| (e.trigger, o)))
        .collect()
}

fn selector_of(op: &Op) -> &Selector {
    match op {
        Op::Attack { target, .. }
        | Op::DealDamage { target, .. }
        | Op::Draw { target, .. }
        | Op::ModifyStat { target, .. }
        | Op::GrantKeyword { target, .. } => target,
    }
}

#[test]
fn every_operation_has_a_fixture_card() {
    let used: BTreeSet<&str> = all_ops().iter().map(|(_, o)| op_name(o)).collect();
    for name in OP_NAMES {
        assert!(used.contains(name), "no fixture card uses `{name}`");
    }
}

#[test]
fn every_trigger_and_stat_is_covered() {
    let ops = all_ops();
    let triggers: BTreeSet<&str> = ops.iter().map(|(t, _)| t.name()).collect();
    for name in TRIGGER_NAMES {
        assert!(triggers.contains(name), "trigger `{name}` is not covered");
    }
    let stats: BTreeSet<&str> = ops
        .iter()
        .filter_map(|(_, o)| match o {
            Op::ModifyStat { stat, .. } => Some(stat_name(*stat)),
            _ => None,
        })
        .collect();
    for name in STAT_NAMES {
        assert!(stats.contains(name), "stat `{name}` is not covered");
    }
    let selectors: BTreeSet<&str> = ops
        .iter()
        .map(|(_, o)| selector_of(o).kind().name())
        .collect();
    for name in SELECT_NAMES {
        assert!(selectors.contains(name), "selector `{name}` is not covered");
    }
    let sides: BTreeSet<&str> = ops
        .iter()
        .map(|(_, o)| match selector_of(o).side() {
            Side::Own => "own",
            Side::Opposing => "opposing",
            Side::Both => "both",
        })
        .collect();
    // `both` is exercised by the polarity table of `effects_validation.rs`, not by a card.
    for name in SIDE_NAMES.iter().filter(|n| **n != "both") {
        assert!(sides.contains(name), "side `{name}` is not covered");
    }
    assert_eq!(KEYWORD_NAMES, ["ranged"]);
    assert!(
        ops.iter()
            .any(|(_, o)| matches!(o, Op::GrantKeyword { .. }))
    );
    let modes: BTreeSet<String> = ops
        .iter()
        .filter_map(|(_, o)| match o {
            Op::Attack { mode: Some(m), .. } => serde_json::to_value(m)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned)),
            _ => None,
        })
        .collect();
    for name in MODE_NAMES {
        assert!(modes.contains(name), "mode `{name}` is not covered");
    }
    assert_eq!(DURATION_NAMES, ["instant", "while_source_in_play"]);
    let cards = cards();
    assert!(cards.iter().any(|c| matches!(c, Card::Spell { .. })));
    assert!(
        cards
            .iter()
            .any(|c| matches!(c, Card::Creature { immobile: true, .. }))
    );
    assert!(
        cards
            .iter()
            .any(|c| matches!(c, Card::Creature { tags, .. } if !tags.is_empty()))
    );
}

#[test]
fn the_fixture_cards_all_validate_and_each_op_has_a_phi_class() {
    let cards = cards();
    assert!(cards.len() >= 8);
    for card in &cards {
        duel_cards::validate_card_spec(&card.spec()).expect(card.id());
    }
    for (trigger, op) in all_ops() {
        let class = op.phi_class();
        // Corollary of the table: continuous is static, the others decrease Phi.
        let expected = if trigger == Trigger::Continuous {
            PhiClass::S
        } else {
            PhiClass::D
        };
        assert_eq!(class, expected, "{}", op_name(&op));
        assert_eq!(class, op.kind().phi_class());
        assert!(OpKind::ALL.contains(&op.kind()));
    }
    // An activated effect always has a D operation (termination rule of the catalog, section 5).
    for card in &cards {
        for effect in effects_of(card) {
            if effect.trigger == Trigger::Activated {
                assert!(effect.ops.iter().any(|o| o.phi_class() == PhiClass::D));
            }
        }
    }
}

#[test]
fn the_fixture_round_trips() {
    let cards = cards();
    let again: Vec<Card> =
        serde_json::from_str(&serde_json::to_string(&cards).expect("serializes")).expect("reads");
    assert_eq!(cards, again);
}
