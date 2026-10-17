use duel_cards::{
    Card, CardsError, CreatureStats, DECK_SIZE, Deck, Effect, Hero, HeroEffect, MAX_HERO_EFFECTS,
    MAX_STAT, parse_cards, parse_decks, resolve_deck, validate_creature, validate_deck_shape,
    validate_hero,
};

const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

fn hero() -> Hero {
    Hero {
        name: "H".into(),
        max_time: 8,
        starting_hand: 4,
        initiative: 1,
        start_budget: 3,
        health: 10,
        effects: vec![draw_effect(2, 1)],
    }
}

fn draw_effect(cost: u32, count: u32) -> HeroEffect {
    HeroEffect {
        name: "Piocher".into(),
        cost,
        effect: Effect::Draw { count },
    }
}

fn stats() -> CreatureStats {
    stats_with(1, 1, 1, 1, false, 0)
}

fn stats_with(
    cost: u32,
    force: u32,
    attack_cost: u32,
    health: u32,
    provocation: bool,
    riposte: u32,
) -> CreatureStats {
    CreatureStats {
        cost,
        force,
        attack_cost,
        health,
        provocation,
        riposte,
    }
}

fn card(id: &str) -> Card {
    Card::Creature {
        id: id.into(),
        name: id.into(),
        cost: 1,
        force: 1,
        attack_cost: 1,
        health: 1,
        provocation: false,
        riposte: 0,
    }
}

fn deck_of(ids: &[&str]) -> Deck {
    Deck {
        id: "d".into(),
        name: "D".into(),
        hero: hero(),
        cards: ids.iter().map(|s| s.to_string()).collect(),
    }
}

/// Strips the `card `x`:` / `deck `x`:` context wrappers to reach the rule that failed.
fn root(mut err: CardsError) -> CardsError {
    while let CardsError::InvalidCard { source, .. } | CardsError::InvalidDeck { source, .. } = err
    {
        err = *source;
    }
    err
}

#[test]
fn fixture_loads_and_every_deck_resolves_to_30_stats() {
    let cards = parse_cards(FIXTURE_CARDS).unwrap();
    let decks = parse_decks(FIXTURE_DECKS).unwrap();
    assert_eq!(decks.len(), 2);
    assert_ne!(decks[0].hero.initiative, decks[1].hero.initiative);
    assert_ne!(decks[0].hero.max_time, decks[1].hero.max_time);
    assert_ne!(decks[0].hero.effects, decks[1].hero.effects);
    assert!(decks.iter().any(|d| d.hero.effects.len() >= 2));
    assert!(decks.iter().all(|d| !d.hero.effects.is_empty()));
    for deck in &decks {
        let resolved = resolve_deck(deck, &cards).unwrap();
        assert_eq!(resolved.len(), DECK_SIZE);
    }
}

#[test]
fn resolve_deck_copies_the_stats_of_each_card_in_order() {
    let cards = vec![
        Card::Creature {
            id: "a".into(),
            name: "A".into(),
            cost: 2,
            force: 3,
            attack_cost: 4,
            health: 5,
            provocation: false,
            riposte: 0,
        },
        card("b"),
    ];
    let mut ids = vec!["b"; DECK_SIZE];
    ids[0] = "a";
    let resolved = resolve_deck(&deck_of(&ids), &cards).unwrap();
    assert_eq!(resolved[0], stats_with(2, 3, 4, 5, false, 0));
    assert_eq!(resolved[1], stats());
}

#[test]
fn rejects_creature_costs_below_one() {
    for field in ["cost", "attack_cost"] {
        let mut s = stats();
        match field {
            "cost" => s.cost = 0,
            _ => s.attack_cost = 0,
        }
        assert_eq!(
            validate_creature(&s),
            Err(CardsError::CostBelowOne { field })
        );
    }
}

#[test]
fn rejects_creature_with_zero_health_or_force() {
    let mut s = stats();
    s.health = 0;
    assert_eq!(
        validate_creature(&s),
        Err(CardsError::NonPositiveStat { field: "health" })
    );
    let mut s = stats();
    s.force = 0;
    assert_eq!(
        validate_creature(&s),
        Err(CardsError::NonPositiveStat { field: "force" })
    );
}

#[test]
fn rejects_effect_cost_below_one() {
    let mut h = hero();
    h.effects = vec![draw_effect(2, 1), draw_effect(0, 1)];
    assert_eq!(
        validate_hero(&h),
        Err(CardsError::EffectCostBelowOne { effect: 1 })
    );
}

#[test]
fn rejects_effect_count_below_one() {
    let mut h = hero();
    h.effects = vec![draw_effect(2, 0)];
    assert_eq!(
        validate_hero(&h),
        Err(CardsError::EffectCountBelowOne { effect: 0 })
    );
}

#[test]
fn rejects_effect_cost_or_count_above_max_stat() {
    let mut h = hero();
    h.effects = vec![draw_effect(MAX_STAT + 1, 1)];
    assert_eq!(
        validate_hero(&h),
        Err(CardsError::EffectStatTooHigh {
            effect: 0,
            field: "cost",
            value: MAX_STAT + 1,
            max: MAX_STAT
        })
    );
    h.effects = vec![draw_effect(1, MAX_STAT + 1)];
    assert_eq!(
        validate_hero(&h),
        Err(CardsError::EffectStatTooHigh {
            effect: 0,
            field: "count",
            value: MAX_STAT + 1,
            max: MAX_STAT
        })
    );
}

#[test]
fn rejects_too_many_effects_and_accepts_the_limit() {
    let mut h = hero();
    h.effects = vec![draw_effect(1, 1); MAX_HERO_EFFECTS];
    assert_eq!(validate_hero(&h), Ok(()));
    h.effects.push(draw_effect(1, 1));
    assert_eq!(
        validate_hero(&h),
        Err(CardsError::TooManyEffects {
            found: MAX_HERO_EFFECTS + 1,
            max: MAX_HERO_EFFECTS
        })
    );
}

#[test]
fn accepts_a_hero_with_no_effect_and_one_with_several() {
    let mut h = hero();
    h.effects = vec![];
    assert_eq!(validate_hero(&h), Ok(()));
    h.effects = vec![draw_effect(3, 1), draw_effect(5, 2)];
    assert_eq!(validate_hero(&h), Ok(()));
}

#[test]
fn effect_limits_are_inclusive() {
    let mut h = hero();
    h.effects = vec![draw_effect(MAX_STAT, MAX_STAT), draw_effect(1, 1)];
    assert_eq!(validate_hero(&h), Ok(()));
}

#[test]
fn parse_decks_applies_the_effect_rules() {
    let mut decks = parse_decks(FIXTURE_DECKS).unwrap();
    decks[0].hero.effects = vec![draw_effect(0, 1)];
    let json = serde_json::to_string(&decks).unwrap();
    assert_eq!(
        root(parse_decks(&json).unwrap_err()),
        CardsError::EffectCostBelowOne { effect: 0 }
    );
}

#[test]
fn effects_are_read_from_json_with_a_kind_tag() {
    let decks = parse_decks(FIXTURE_DECKS).unwrap();
    let effects: Vec<&HeroEffect> = decks.iter().flat_map(|d| &d.hero.effects).collect();
    assert!(
        effects
            .iter()
            .all(|e| matches!(e.effect, Effect::Draw { .. }))
    );
    let json = serde_json::to_string(&decks[0].hero).unwrap();
    assert!(json.contains(r#""kind":"draw""#), "{json}");
}

#[test]
fn rejects_unknown_effect_field_kind_and_the_removed_draw_cost() {
    let decks = parse_decks(FIXTURE_DECKS).unwrap();
    let base = serde_json::to_value(&decks).unwrap();
    let mutate = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = base.clone();
        f(&mut v[0]["hero"]);
        parse_decks(&v.to_string())
    };
    let err = mutate(&|h| h["effects"][0]["effect"]["bonus"] = 1.into()).unwrap_err();
    assert!(
        matches!(&err, CardsError::Json(m) if m.contains("bonus")),
        "{err}"
    );
    let err = mutate(&|h| h["effects"][0]["bonus"] = 1.into()).unwrap_err();
    assert!(
        matches!(&err, CardsError::Json(m) if m.contains("bonus")),
        "{err}"
    );
    let err = mutate(&|h| h["effects"][0]["effect"]["kind"] = "heal".into()).unwrap_err();
    assert!(
        matches!(&err, CardsError::Json(m) if m.contains("heal")),
        "{err}"
    );
    let err = mutate(&|h| h["draw_cost"] = 2.into()).unwrap_err();
    assert!(
        matches!(&err, CardsError::Json(m) if m.contains("draw_cost")),
        "{err}"
    );
    let err = mutate(&|h| {
        h.as_object_mut().unwrap().remove("effects");
    })
    .unwrap_err();
    assert!(
        matches!(&err, CardsError::Json(m) if m.contains("effects")),
        "{err}"
    );
}

#[test]
fn rejects_max_time_below_one() {
    let mut h = hero();
    h.max_time = 0;
    assert_eq!(validate_hero(&h), Err(CardsError::MaxTimeBelowOne));
}

#[test]
fn rejects_start_budget_outside_one_to_max_time() {
    for budget in [0, 9] {
        let mut h = hero();
        h.start_budget = budget;
        assert_eq!(
            validate_hero(&h),
            Err(CardsError::StartBudgetOutOfRange {
                start_budget: budget,
                max_time: 8
            })
        );
    }
}

#[test]
fn rejects_hero_with_zero_health() {
    let mut h = hero();
    h.health = 0;
    assert_eq!(
        validate_hero(&h),
        Err(CardsError::NonPositiveStat { field: "health" })
    );
}

#[test]
fn rejects_starting_hand_larger_than_deck() {
    let mut h = hero();
    h.starting_hand = 31;
    assert_eq!(
        validate_deck_shape(&h, DECK_SIZE),
        Err(CardsError::StartingHandExceedsDeck {
            starting_hand: 31,
            deck_len: DECK_SIZE
        })
    );
}

#[test]
fn rejects_deck_that_is_not_30_cards() {
    for len in [0, 29, 31] {
        assert_eq!(
            validate_deck_shape(&hero(), len),
            Err(CardsError::WrongDeckSize {
                found: len,
                expected: DECK_SIZE
            })
        );
    }
}

#[test]
fn accepts_exact_limits() {
    let mut h = hero();
    h.start_budget = h.max_time;
    h.starting_hand = DECK_SIZE as u32;
    assert_eq!(validate_hero(&h), Ok(()));
    assert_eq!(validate_deck_shape(&h, DECK_SIZE), Ok(()));
    let at_max = stats_with(MAX_STAT, MAX_STAT, MAX_STAT, MAX_STAT, true, MAX_STAT);
    assert_eq!(validate_creature(&at_max), Ok(()));
}

#[test]
fn rejects_values_above_max_stat() {
    let mut s = stats();
    s.health = MAX_STAT + 1;
    assert_eq!(
        validate_creature(&s),
        Err(CardsError::StatTooHigh {
            field: "health",
            value: MAX_STAT + 1,
            max: MAX_STAT
        })
    );
    let mut h = hero();
    h.max_time = MAX_STAT + 1;
    assert_eq!(
        validate_hero(&h),
        Err(CardsError::StatTooHigh {
            field: "max_time",
            value: MAX_STAT + 1,
            max: MAX_STAT
        })
    );
}

#[test]
fn rejects_unknown_card_in_deck() {
    let mut ids = vec!["a"; DECK_SIZE];
    ids[7] = "zzz";
    assert_eq!(
        resolve_deck(&deck_of(&ids), &[card("a")]),
        Err(CardsError::UnknownCard {
            deck: "d".into(),
            card: "zzz".into()
        })
    );
}

#[test]
fn rejects_duplicate_card_ids() {
    let json = r#"[
        {"kind":"creature","id":"a","name":"A","cost":1,"force":1,"attack_cost":1,"health":1},
        {"kind":"creature","id":"a","name":"A2","cost":1,"force":1,"attack_cost":1,"health":1}
    ]"#;
    assert_eq!(
        parse_cards(json),
        Err(CardsError::DuplicateCard("a".into()))
    );
}

#[test]
fn parse_cards_applies_the_stat_rules() {
    let json = r#"[{"kind":"creature","id":"a","name":"A","cost":0,"force":1,"attack_cost":1,"health":1}]"#;
    let err = parse_cards(json).unwrap_err();
    assert_eq!(root(err), CardsError::CostBelowOne { field: "cost" });
}

#[test]
fn parse_decks_applies_hero_and_shape_rules() {
    let mut decks = parse_decks(FIXTURE_DECKS).unwrap();
    decks[0].cards.pop();
    let json = serde_json::to_string(&decks).unwrap();
    assert_eq!(
        root(parse_decks(&json).unwrap_err()),
        CardsError::WrongDeckSize {
            found: 29,
            expected: DECK_SIZE
        }
    );
}

#[test]
fn rejects_unknown_field() {
    let json = r#"[{"kind":"creature","id":"a","name":"A","cost":1,"force":1,"attack_cost":1,"health":1,"bonus":9}]"#;
    let err = parse_cards(json).unwrap_err();
    assert!(
        matches!(&err, CardsError::Json(m) if m.contains("bonus")),
        "{err}"
    );
}

#[test]
fn rejects_unknown_kind() {
    let json = r#"[{"kind":"spell","id":"a","name":"A"}]"#;
    assert!(matches!(parse_cards(json), Err(CardsError::Json(_))));
}

#[test]
fn rejects_negative_value_with_readable_message() {
    let json = r#"[{"kind":"creature","id":"a","name":"A","cost":-1,"force":1,"attack_cost":1,"health":1}]"#;
    let err = parse_cards(json).unwrap_err();
    assert!(
        matches!(&err, CardsError::Json(m) if m.contains("-1")),
        "{err}"
    );
}

#[test]
fn truncated_json_is_an_error_not_a_panic() {
    for cut in [0, 1, 20, FIXTURE_CARDS.len() - 3] {
        let truncated = FIXTURE_CARDS.get(..cut).unwrap_or("");
        assert!(matches!(parse_cards(truncated), Err(CardsError::Json(_))));
    }
    assert!(matches!(
        parse_decks(FIXTURE_DECKS.get(..40).unwrap_or("")),
        Err(CardsError::Json(_))
    ));
}

#[test]
fn rejects_duplicate_card_names() {
    let json = r#"[
        {"kind":"creature","id":"a","name":"Same","cost":1,"force":1,"attack_cost":1,"health":1},
        {"kind":"creature","id":"b","name":"Same","cost":1,"force":1,"attack_cost":1,"health":1}
    ]"#;
    assert_eq!(
        parse_cards(json),
        Err(CardsError::DuplicateCardName("Same".into()))
    );
}

#[test]
fn duplicate_card_id_is_reported_before_duplicate_name() {
    let json = r#"[
        {"kind":"creature","id":"a","name":"Same","cost":1,"force":1,"attack_cost":1,"health":1},
        {"kind":"creature","id":"a","name":"Same","cost":1,"force":1,"attack_cost":1,"health":1}
    ]"#;
    assert_eq!(
        parse_cards(json),
        Err(CardsError::DuplicateCard("a".into()))
    );
}

#[test]
fn card_names_differing_by_case_are_distinct() {
    let json = r#"[
        {"kind":"creature","id":"a","name":"Same","cost":1,"force":1,"attack_cost":1,"health":1},
        {"kind":"creature","id":"b","name":"same","cost":1,"force":1,"attack_cost":1,"health":1}
    ]"#;
    assert!(parse_cards(json).is_ok());
}
