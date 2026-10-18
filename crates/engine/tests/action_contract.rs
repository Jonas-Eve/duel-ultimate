//! #128 (task #180): the additive `Action` contract. `Play { card, picks }` and
//! `Activate { effect, source, picks }` carry a list of choices and an optional source, both
//! omitted from the JSON when empty, so the JSON of every existing action stays byte for byte
//! what it was (`legal_actions_json_is_byte_identical_to_before_128` pins whole games). No legal
//! action carries `picks` or `source` at this stage: only a hostile caller sends them, and the
//! engine refuses them with three stable codes, in a fixed order, without touching the state.

use std::borrow::Borrow;

use duel_cards::{CreatureStats, Effect, Hero, HeroEffect};
use duel_engine::{
    Action, CardInstanceId, GameSetup, GameState, MAX_PICKS, Pick, PlayerId, PlayerSetup,
    RuleError, RuleSet, Target, apply, legal_actions, new_game,
};

fn id(n: u32) -> CardInstanceId {
    CardInstanceId(n)
}

fn action(json: &str) -> Action {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

fn json(action: &Action) -> String {
    serde_json::to_string(action).unwrap()
}

fn picks(n: usize) -> Vec<Pick> {
    vec![Pick::Hero(PlayerId::One); n]
}

fn play_with(card: u32, picks: Vec<Pick>) -> Action {
    Action::Play {
        card: id(card),
        picks,
    }
}

fn activate_with(effect: u32, source: Option<u32>, picks: Vec<Pick>) -> Action {
    Action::Activate {
        effect,
        source: source.map(id),
        picks,
    }
}

// ---- the JSON ----

#[test]
fn existing_actions_serialize_exactly_as_before() {
    assert_eq!(json(&Action::play(id(3))), r#"{"play":{"card":3}}"#);
    assert_eq!(json(&Action::activate(1)), r#"{"activate":{"effect":1}}"#);
    assert_eq!(
        json(&Action::Attack {
            attacker: id(2),
            target: Target::Hero
        }),
        r#"{"attack":{"attacker":2,"target":"hero"}}"#
    );
    assert_eq!(
        json(&Action::Attack {
            attacker: id(2),
            target: Target::Creature(id(7))
        }),
        r#"{"attack":{"attacker":2,"target":{"creature":7}}}"#
    );
    assert_eq!(json(&Action::Concede), r#""concede""#);
}

#[test]
fn empty_picks_and_absent_source_are_accepted_and_omitted_on_output() {
    let play = action(r#"{"play":{"card":3,"picks":[]}}"#);
    assert_eq!(play, Action::play(id(3)));
    assert_eq!(json(&play), r#"{"play":{"card":3}}"#);
    let activate = action(r#"{"activate":{"effect":1,"picks":[]}}"#);
    assert_eq!(activate, Action::activate(1));
    assert_eq!(json(&activate), r#"{"activate":{"effect":1}}"#);
}

#[test]
fn activate_with_source_and_picks_round_trips() {
    let text = r#"{"activate":{"effect":0,"source":7,"picks":[{"hero":"One"},{"creature":3}]}}"#;
    let parsed = action(text);
    assert_eq!(
        parsed,
        activate_with(
            0,
            Some(7),
            vec![Pick::Hero(PlayerId::One), Pick::Creature(id(3))]
        )
    );
    assert_eq!(json(&parsed), text);
    let played = r#"{"play":{"card":3,"picks":[{"creature":9}]}}"#;
    assert_eq!(json(&action(played)), played);
}

#[test]
fn pick_json_forms_and_refusals() {
    let pick = |text: &str| serde_json::from_str::<Pick>(text);
    assert_eq!(
        pick(r#"{"hero":"Two"}"#).unwrap(),
        Pick::Hero(PlayerId::Two)
    );
    assert_eq!(pick(r#"{"creature":7}"#).unwrap(), Pick::Creature(id(7)));
    assert_eq!(
        serde_json::to_string(&Pick::Hero(PlayerId::Two)).unwrap(),
        r#"{"hero":"Two"}"#
    );
    assert_eq!(
        serde_json::to_string(&Pick::Creature(id(7))).unwrap(),
        r#"{"creature":7}"#
    );
    for refused in [r#"{"hero":"one"}"#, r#"{"hero":"Three"}"#, r#"{"card":1}"#] {
        assert!(pick(refused).is_err(), "{refused}");
    }
}

#[test]
fn an_unknown_field_is_still_refused() {
    for refused in [
        r#"{"play":{"card":3,"x":1}}"#,
        r#"{"activate":{"effect":1,"x":1}}"#,
        r#"{"activate":{"effect":1,"source":2,"pick":[]}}"#,
    ] {
        assert!(
            serde_json::from_str::<Action>(refused).is_err(),
            "{refused}"
        );
    }
}

// ---- the refusals ----

fn unit() -> CreatureStats {
    CreatureStats {
        cost: 1,
        force: 2,
        attack_cost: 1,
        health: 1,
        provocation: false,
        riposte: 0,
    }
}

fn hero(initiative: u32, start_budget: u32, starting_hand: u32) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time: 30,
        starting_hand,
        initiative,
        start_budget,
        health: 10,
        effects: vec![HeroEffect {
            name: "Piocher".into(),
            cost: 1,
            effect: Effect::Draw { count: 1 },
        }],
    }
}

fn game(rules: RuleSet, starting_hand: u32, budget: u32) -> GameState {
    let side = |initiative| PlayerSetup {
        hero: hero(initiative, budget, starting_hand),
        cards: vec![unit(); 30].into_iter().map(Into::into).collect(),
        card_names: Vec::new(),
    };
    new_game(
        GameSetup {
            one: side(2),
            two: side(1),
            data_versions: vec!["fixture@1.0.0".into()],
            rules,
        },
        7,
    )
    .unwrap()
    .0
}

/// Refused with `error`, and nothing changed (`GameState: PartialEq` covers the generator).
fn refused(state: &GameState, action: &Action, error: impl Borrow<RuleError>) {
    let mut copy = state.clone();
    assert_eq!(
        apply(&mut copy, action.clone()).as_ref(),
        Err(error.borrow()),
        "{action:?}"
    );
    assert_eq!(&copy, state, "{action:?} changed the state");
}

#[test]
fn nine_picks_are_too_many_whatever_the_card() {
    let state = game(RuleSet::standard(), 30, 20);
    refused(
        &state,
        &play_with(10_000, picks(9)),
        RuleError::TooManyPicks,
    );
    let in_hand = state.hand(PlayerId::One)[0];
    refused(
        &state,
        &Action::Play {
            card: in_hand,
            picks: picks(9),
        },
        RuleError::TooManyPicks,
    );
    refused(
        &state,
        &activate_with(0, None, picks(9)),
        RuleError::TooManyPicks,
    );
    refused(
        &state,
        &activate_with(99, Some(10_000), picks(9)),
        RuleError::TooManyPicks,
    );
    assert_eq!(MAX_PICKS, 8);
    // Eight is not too many: the next check speaks (unknown card, then invalid picks).
    refused(
        &state,
        &play_with(10_000, picks(8)),
        RuleError::CardNotInHand,
    );
    refused(
        &state,
        &Action::Play {
            card: in_hand,
            picks: picks(8),
        },
        RuleError::InvalidPicks,
    );
}

#[test]
fn a_finished_game_says_game_over_before_anything_else() {
    let mut state = game(RuleSet::standard(), 30, 20);
    apply(&mut state, Action::Concede).unwrap();
    refused(&state, &play_with(10_000, picks(9)), RuleError::GameOver);
    refused(
        &state,
        &activate_with(0, Some(1), picks(9)),
        RuleError::GameOver,
    );
}

#[test]
fn any_pick_on_a_play_or_a_hero_effect_is_invalid_picks() {
    let state = game(RuleSet::standard(), 25, 20);
    let card = state.hand(PlayerId::One)[0];
    for pick in [
        Pick::Hero(PlayerId::One),
        Pick::Hero(PlayerId::Two),
        Pick::Creature(id(0)),
        Pick::Creature(id(10_000)),
    ] {
        refused(
            &state,
            &Action::Play {
                card,
                picks: vec![pick],
            },
            RuleError::InvalidPicks,
        );
        refused(
            &state,
            &activate_with(0, None, vec![pick]),
            RuleError::InvalidPicks,
        );
    }
}

#[test]
fn play_checks_hand_then_picks_then_board_full() {
    let rules = RuleSet {
        board_limit: Some(1),
        ..RuleSet::legacy()
    };
    let mut state = game(rules, 30, 20);
    let first = state.hand(PlayerId::One)[0];
    apply(&mut state, Action::play(first)).unwrap();
    let second = state.hand(PlayerId::One)[0];
    refused(&state, &Action::play(second), RuleError::BoardFull);
    refused(
        &state,
        &Action::Play {
            card: second,
            picks: picks(1),
        },
        RuleError::InvalidPicks,
    );
    refused(
        &state,
        &Action::Play {
            card: id(10_000),
            picks: picks(1),
        },
        RuleError::CardNotInHand,
    );
}

#[test]
fn hero_activate_checks_effect_then_picks_then_applicability() {
    // The whole deck is in hand: the draw has nothing to draw.
    let state = game(RuleSet::standard(), 30, 20);
    refused(&state, &Action::activate(0), RuleError::EffectNotApplicable);
    refused(
        &state,
        &activate_with(0, None, picks(1)),
        RuleError::InvalidPicks,
    );
    refused(
        &state,
        &activate_with(5, None, picks(1)),
        RuleError::UnknownEffect,
    );
}

/// One plays, Two plays, One plays, Two kills One's first creature: One is active again with
/// a creature on its board (`mine`), a creature in its discard, Two's creature on Two's board,
/// cards in hand and cards in its pile. Every cost is 1 against a budget of 1: the hand changes
/// at each action.
struct Table {
    state: GameState,
    mine: CardInstanceId,
    theirs: CardInstanceId,
    dead: CardInstanceId,
}

fn table() -> Table {
    let mut state = game(RuleSet::legacy(), 25, 1);
    assert_eq!(state.active(), PlayerId::One);
    let dead = state.hand(PlayerId::One)[0];
    apply(&mut state, Action::play(dead)).unwrap();
    let theirs = state.hand(PlayerId::Two)[0];
    apply(&mut state, Action::play(theirs)).unwrap();
    let mine = state.hand(PlayerId::One)[0];
    apply(&mut state, Action::play(mine)).unwrap();
    apply(
        &mut state,
        Action::Attack {
            attacker: theirs,
            target: Target::Creature(dead),
        },
    )
    .unwrap();
    assert_eq!(state.active(), PlayerId::One);
    assert_eq!(state.board(PlayerId::One), vec![mine]);
    assert_eq!(state.discard(PlayerId::One), vec![dead]);
    assert_eq!(state.board(PlayerId::Two), vec![theirs]);
    Table {
        state,
        mine,
        theirs,
        dead,
    }
}

#[test]
fn a_source_that_is_not_on_my_board_gets_one_code_for_every_zone() {
    let t = table();
    let in_hand = t.state.hand(PlayerId::One)[0];
    let in_pile = (0..30)
        .map(id)
        .find(|c| {
            ![t.mine, t.dead, in_hand].contains(c) && !t.state.hand(PlayerId::One).contains(c)
        })
        .expect("a card of One's pile");
    let mut results = Vec::new();
    for (what, source) in [
        ("opposing board", t.theirs),
        ("own hand", in_hand),
        ("own pile", in_pile),
        ("own discard", t.dead),
        ("opposing hand", id(30 + 29)),
        ("unknown id", id(10_000)),
    ] {
        let mut copy = t.state.clone();
        let result = apply(&mut copy, activate_with(0, Some(source.0), Vec::new()));
        assert_eq!(copy, t.state, "{what}: a refusal changes nothing");
        results.push((what, result));
    }
    for (what, result) in &results {
        assert_eq!(result, &Err(RuleError::SourceNotOnBoard), "{what}");
    }
    // With picks too: the source is judged, never the picks.
    refused(
        &t.state,
        &activate_with(0, Some(t.theirs.0), picks(1)),
        RuleError::SourceNotOnBoard,
    );
}

#[test]
fn a_source_on_my_board_has_no_declared_effect() {
    let t = table();
    for effect in [0, 1, 99] {
        refused(
            &t.state,
            &activate_with(effect, Some(t.mine.0), Vec::new()),
            RuleError::UnknownEffect,
        );
    }
    refused(
        &t.state,
        &activate_with(0, Some(t.mine.0), picks(3)),
        RuleError::UnknownEffect,
    );
}

#[test]
fn every_refusal_leaves_state_and_generator_untouched() {
    let t = table();
    let card = t.state.hand(PlayerId::One)[0];
    let hostile = [
        (play_with(10_000, picks(9)), RuleError::TooManyPicks),
        (
            Action::Play {
                card,
                picks: picks(2),
            },
            RuleError::InvalidPicks,
        ),
        (activate_with(0, None, picks(1)), RuleError::InvalidPicks),
        (
            activate_with(0, Some(t.theirs.0), Vec::new()),
            RuleError::SourceNotOnBoard,
        ),
        (
            activate_with(0, Some(t.mine.0), Vec::new()),
            RuleError::UnknownEffect,
        ),
    ];
    for (action, error) in &hostile {
        refused(&t.state, action, error);
    }
}

#[test]
fn rule_error_codes_are_stable() {
    assert_eq!(RuleError::TooManyPicks.code(), "too_many_picks");
    assert_eq!(RuleError::InvalidPicks.code(), "invalid_picks");
    assert_eq!(RuleError::SourceNotOnBoard.code(), "source_not_on_board");
    for error in [
        RuleError::TooManyPicks,
        RuleError::InvalidPicks,
        RuleError::SourceNotOnBoard,
    ] {
        assert!(error.to_string().is_ascii(), "{error}");
    }
}

#[test]
fn legal_actions_never_carry_picks_or_source() {
    let mut inspected = 0;
    for seed in 0..250u64 {
        let (mut state, _) = new_game(
            GameSetup {
                one: PlayerSetup {
                    hero: hero(2, 3, 4),
                    cards: vec![unit(); 30].into_iter().map(Into::into).collect(),
                    card_names: Vec::new(),
                },
                two: PlayerSetup {
                    hero: hero(1, 3, 4),
                    cards: vec![unit(); 30].into_iter().map(Into::into).collect(),
                    card_names: Vec::new(),
                },
                data_versions: Vec::new(),
                rules: RuleSet::standard(),
            },
            seed,
        )
        .unwrap();
        let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        while state.winner().is_none() {
            let legal = legal_actions(&state);
            for action in &legal {
                inspected += 1;
                match action {
                    Action::Play { picks, .. } => assert!(picks.is_empty(), "{action:?}"),
                    Action::Activate { source, picks, .. } => {
                        assert!(source.is_none() && picks.is_empty(), "{action:?}");
                    }
                    Action::Attack { .. } | Action::Concede => {}
                }
                let text = json(action);
                assert!(
                    !text.contains("picks") && !text.contains("source"),
                    "{text}"
                );
            }
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let playable: Vec<_> = legal
                .into_iter()
                .filter(|a| *a != Action::Concede)
                .collect();
            let chosen = if playable.is_empty() {
                Action::Concede
            } else {
                let index = usize::try_from(rng % playable.len() as u64).unwrap();
                playable.into_iter().nth(index).unwrap()
            };
            apply(&mut state, chosen).unwrap();
        }
    }
    assert!(inspected > 1000);
}
