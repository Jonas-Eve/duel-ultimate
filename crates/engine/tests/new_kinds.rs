//! #208 (task 3 of #137): the engine receives spells, immobile units, tags and declared effects
//! by the setup, carries them, and draws **no new behavior** from them. A spell is never
//! playable (`unsupported_effect`, temporary), an immobile unit has a force of 0 and never
//! attacks (`attack_force_zero`), a declared effect of a unit is never resolved.

use duel_cards::{CardEffect, CardSpec, CreatureStats, Hero, MAX_STAT, SpellSpec, UnitSpec};
use duel_engine::{
    Action, CardInstanceId, Event, GameSetup, GameState, Pick, PlayerId, PlayerSetup, RuleError,
    RuleSet, Target, WinReason, apply, legal_actions, new_game, player_view,
};

const OWN_DRAW: &str = r#"{"trigger":"on_play","ops":[{"op":"draw","target":{"select":"hero","side":"own"},"count":1}]}"#;

fn effect(json: &str) -> CardEffect {
    serde_json::from_str(json).unwrap()
}

fn hero(initiative: u32, starting_hand: u32, start_budget: u32) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time: 10,
        starting_hand,
        initiative,
        start_budget,
        health: 10,
        effects: Vec::new(),
    }
}

fn stats(cost: u32, force: u32, attack_cost: u32, health: u32) -> CreatureStats {
    CreatureStats {
        cost,
        force,
        attack_cost,
        health,
        provocation: false,
        riposte: 0,
    }
}

fn plain(cost: u32) -> CardSpec {
    stats(cost, 1, 1, 3).into()
}

fn immobile(cost: u32) -> CardSpec {
    CardSpec::Unit(UnitSpec {
        stats: stats(cost, 0, 0, 3),
        immobile: true,
        tags: Vec::new(),
        effects: Vec::new(),
    })
}

fn spell(cost: u32) -> CardSpec {
    CardSpec::Spell(SpellSpec {
        cost,
        tags: Vec::new(),
        effects: vec![effect(OWN_DRAW)],
    })
}

/// 30 cards, named after their kind, in the order given by `kind(position)`.
fn side(hero: Hero, kind: impl Fn(usize) -> &'static str) -> PlayerSetup {
    let names: Vec<String> = (0..30).map(|n| kind(n).to_owned()).collect();
    let cards = names
        .iter()
        .map(|name| match name.as_str() {
            "spell" => spell(1),
            "immobile" => immobile(1),
            _ => plain(1),
        })
        .collect();
    PlayerSetup {
        hero,
        cards,
        card_names: names,
    }
}

fn mixed(n: usize) -> &'static str {
    ["spell", "immobile", "unit"][n % 3]
}

fn setup_of(one: PlayerSetup, two: PlayerSetup) -> GameSetup {
    GameSetup {
        one,
        two,
        data_versions: Vec::new(),
        rules: RuleSet::default(),
    }
}

/// The ids named `name` in the hand of `seat` (every card is in the hand when `starting_hand` is 30).
fn hand_named(state: &GameState, seat: PlayerId, name: &str) -> Vec<CardInstanceId> {
    player_view(state, seat)
        .you
        .hand
        .iter()
        .filter(|c| c.name.as_deref() == Some(name))
        .map(|c| c.id)
        .collect()
}

/// One starts with everything in hand, 10 units of time, and a mix of kinds on both sides.
fn mixed_game() -> GameState {
    let setup = setup_of(side(hero(2, 30, 10), mixed), side(hero(1, 30, 10), mixed));
    let (state, _) = new_game(setup, 1).unwrap();
    assert_eq!(state.active(), PlayerId::One);
    assert_eq!(state.winner(), None);
    state
}

fn code(error: RuleError) -> &'static str {
    error.code()
}

#[test]
fn new_game_accepts_spells_immobile_units_tags_and_effects() {
    let mut setup = setup_of(side(hero(2, 4, 3), mixed), side(hero(1, 4, 3), mixed));
    // Tags and declared effects on units too.
    for side in [&mut setup.one, &mut setup.two] {
        for (n, card) in side.cards.iter_mut().enumerate() {
            if let CardSpec::Unit(unit) = card
                && n % 3 == 2
            {
                unit.tags = vec!["bâtiment".into(), "machine".into()];
                unit.effects = vec![effect(OWN_DRAW)];
            }
        }
    }
    assert!(new_game(setup.clone(), 1).is_ok());
    // ...and through the JSON of the setup, the way the binding reads it.
    let json = serde_json::to_string(&setup).unwrap();
    let back: GameSetup = serde_json::from_str(&json).unwrap();
    assert_eq!(back, setup);
    assert!(new_game(back, 1).is_ok());
    // An invalid one is still refused by the validation, not by a guard.
    let mut bad = setup;
    bad.one.cards[0] = CardSpec::Spell(SpellSpec {
        cost: MAX_STAT + 1,
        tags: Vec::new(),
        effects: vec![effect(OWN_DRAW)],
    });
    assert_eq!(new_game(bad, 1).unwrap_err().code(), "invalid_setup");
}

#[test]
fn a_spell_is_never_playable_for_now() {
    let mut state = mixed_game();
    let spells = hand_named(&state, PlayerId::One, "spell");
    assert_eq!(spells.len(), 10);
    let legal = legal_actions(&state);
    for id in &spells {
        assert!(
            !legal
                .iter()
                .any(|a| matches!(a, Action::Play { card, .. } if card == id)),
            "{id:?}"
        );
    }
    assert!(
        legal.iter().any(|a| matches!(a, Action::Play { .. })),
        "the units of the same hand are playable"
    );

    let before = state.clone();
    let error = apply(&mut state, Action::play(spells[0])).unwrap_err();
    assert_eq!(error, RuleError::UnsupportedEffect);
    assert_eq!(code(error), "unsupported_effect");
    assert_eq!(
        state, before,
        "a refusal leaves the state and the generator untouched"
    );

    // The order of the checks: the hand first, then the picks, then the spell.
    let opposing = hand_named(&state, PlayerId::Two, "spell")[0];
    for absent in [opposing, CardInstanceId(9_999)] {
        assert_eq!(
            apply(&mut state, Action::play(absent)).unwrap_err(),
            RuleError::CardNotInHand
        );
    }
    let with_picks = Action::Play {
        card: spells[0],
        picks: vec![Pick::Hero(PlayerId::One)],
    };
    assert_eq!(
        apply(&mut state, with_picks).unwrap_err(),
        RuleError::InvalidPicks
    );
    assert_eq!(state, before);

    // The game being over comes first.
    apply(&mut state, Action::Concede).unwrap();
    assert_eq!(
        apply(&mut state, Action::play(spells[0])).unwrap_err(),
        RuleError::GameOver
    );
}

#[test]
fn a_spell_in_hand_does_not_count_for_exhaustion() {
    // Only spells, in hand, and no hero effect: the first player has no action, as with no card.
    let all_spells = |hero| side(hero, |_| "spell");
    let (state, events) = new_game(
        setup_of(all_spells(hero(2, 30, 10)), all_spells(hero(1, 30, 10))),
        1,
    )
    .unwrap();
    assert_eq!(state.winner(), Some((PlayerId::Two, WinReason::Exhaustion)));
    assert!(matches!(
        events.last(),
        Some(Event::GameWon {
            reason: WinReason::Exhaustion,
            ..
        })
    ));
    assert_eq!(legal_actions(&state), Vec::<Action>::new());
    // With one playable unit in the same hand the game runs: the spells are the only difference.
    let one_unit = |hero| side(hero, |n| if n == 0 { "unit" } else { "spell" });
    let (state, _) = new_game(
        setup_of(one_unit(hero(2, 30, 10)), one_unit(hero(1, 30, 10))),
        1,
    )
    .unwrap();
    assert_eq!(state.winner(), None);
}

#[test]
fn an_immobile_unit_never_attacks() {
    let mut state = mixed_game();
    let immobiles = hand_named(&state, PlayerId::One, "immobile");
    let mobiles = hand_named(&state, PlayerId::One, "unit");
    apply(&mut state, Action::play(immobiles[0])).unwrap();
    apply(&mut state, Action::play(mobiles[0])).unwrap();
    assert_eq!(state.board(PlayerId::One), vec![immobiles[0], mobiles[0]]);

    let legal = legal_actions(&state);
    let attackers: Vec<CardInstanceId> = legal
        .iter()
        .filter_map(|a| match a {
            Action::Attack { attacker, .. } => Some(*attacker),
            _ => None,
        })
        .collect();
    assert_eq!(
        attackers,
        vec![mobiles[0]],
        "the mobile unit attacks, the other never"
    );

    let before = state.clone();
    for target in [
        Target::Hero,
        Target::Creature(CardInstanceId(9_999)),
        Target::Creature(mobiles[0]),
    ] {
        let error = apply(
            &mut state,
            Action::Attack {
                attacker: immobiles[0],
                target,
            },
        )
        .unwrap_err();
        assert_eq!(error, RuleError::AttackForceZero, "{target:?}");
        assert_eq!(code(error), "attack_force_zero");
    }
    assert_eq!(state, before);
    // The attacker is judged first: an unknown attacker is not on the board, whatever the force.
    let ghost = Action::Attack {
        attacker: CardInstanceId(9_999),
        target: Target::Hero,
    };
    assert_eq!(
        apply(&mut state, ghost).unwrap_err(),
        RuleError::AttackerNotOnBoard
    );
    // The mobile unit attacks normally.
    apply(
        &mut state,
        Action::Attack {
            attacker: mobiles[0],
            target: Target::Hero,
        },
    )
    .unwrap();
}

#[test]
fn an_immobile_unit_takes_a_slot_of_the_board_like_any_unit() {
    let mut setup = setup_of(side(hero(2, 30, 10), mixed), side(hero(1, 30, 10), mixed));
    setup.rules.board_limit = Some(1);
    let (mut state, _) = new_game(setup, 1).unwrap();
    let immobiles = hand_named(&state, PlayerId::One, "immobile");
    let mobiles = hand_named(&state, PlayerId::One, "unit");
    apply(&mut state, Action::play(immobiles[0])).unwrap();
    // One slot, taken by the immobile unit: a second unit is refused, then allowed nowhere.
    assert_eq!(
        apply(&mut state, Action::play(mobiles[0])).unwrap_err(),
        RuleError::BoardFull
    );
    assert_eq!(
        apply(&mut state, Action::play(immobiles[1])).unwrap_err(),
        RuleError::BoardFull
    );
}

#[test]
fn a_player_with_only_immobile_units_and_nothing_else_is_exhausted() {
    // Thirty immobile units of cost 1 each, one card in the opening hand, no hero effect. One
    // plays its card (cost 1 = the budget: the token passes), Two plays its own and gives the
    // token back: One has a board of immobile units only, no hand and no way to draw, no attack:
    // it loses by exhaustion.
    let (mut state, _) = new_game(
        setup_of(
            side(hero(2, 1, 1), |_| "immobile"),
            side(hero(1, 1, 1), |_| "immobile"),
        ),
        1,
    )
    .unwrap();
    assert_eq!(state.active(), PlayerId::One);
    let card = player_view(&state, PlayerId::One).you.hand[0].id;
    apply(&mut state, Action::play(card)).unwrap();
    assert_eq!(state.winner(), None, "Two still has its card to play");
    assert_eq!(state.active(), PlayerId::Two);
    let card = player_view(&state, PlayerId::Two).you.hand[0].id;
    let events = apply(&mut state, Action::play(card)).unwrap();
    assert_eq!(state.winner(), Some((PlayerId::Two, WinReason::Exhaustion)));
    assert_eq!(state.board(PlayerId::One).len(), 1);
    assert!(matches!(
        events.last(),
        Some(Event::GameWon {
            winner: PlayerId::Two,
            reason: WinReason::Exhaustion
        })
    ));
}

#[test]
fn a_unit_with_declared_effects_is_played_like_any_unit() {
    // The same game, with and without a declared effect on every unit: the same events, the
    // same views, the same legal actions, step after step.
    let build = |with_effects: bool| {
        let mut setup = setup_of(
            side(hero(2, 8, 10), |_| "unit"),
            side(hero(1, 8, 10), |_| "unit"),
        );
        if with_effects {
            for side in [&mut setup.one, &mut setup.two] {
                for card in &mut side.cards {
                    if let CardSpec::Unit(unit) = card {
                        unit.effects = vec![effect(OWN_DRAW)];
                    }
                }
            }
        }
        new_game(setup, 7).unwrap()
    };
    let ((mut a, events_a), (mut b, events_b)) = (build(false), build(true));
    assert_eq!(events_a, events_b);
    for step in 0..40 {
        let legal = legal_actions(&a);
        assert_eq!(legal, legal_actions(&b), "step {step}");
        if legal.len() == 1 {
            break;
        }
        let action = legal[(step * 7) % (legal.len() - 1)].clone();
        assert_eq!(
            apply(&mut a, action.clone()).unwrap(),
            apply(&mut b, action).unwrap(),
            "step {step}"
        );
        for seat in [PlayerId::One, PlayerId::Two] {
            assert_eq!(player_view(&a, seat), player_view(&b, seat), "step {step}");
        }
        if a.winner().is_some() {
            break;
        }
    }
}

#[test]
fn activating_a_declared_effect_of_a_board_unit() {
    let activated = effect(
        r#"{"trigger":"activated","cost":2,"ops":[{"op":"draw","target":{"select":"hero","side":"own"},"count":1}]}"#,
    );
    let continuous = effect(
        r#"{"trigger":"continuous","ops":[{"op":"grant_keyword","target":{"select":"self"},"keyword":"ranged"}]}"#,
    );
    let mut one = side(hero(2, 30, 10), |_| "unit");
    if let CardSpec::Unit(unit) = &mut one.cards[0] {
        unit.effects = vec![effect(OWN_DRAW), activated, continuous];
    }
    one.card_names[0] = "carrier".into();
    let (mut state, _) = new_game(setup_of(one, side(hero(1, 30, 10), |_| "unit")), 1).unwrap();
    let carrier = hand_named(&state, PlayerId::One, "carrier")[0];
    let other = hand_named(&state, PlayerId::One, "unit")[0];
    let activate = |source: CardInstanceId, effect: u32| Action::Activate {
        effect,
        source: Some(source),
        picks: Vec::new(),
    };
    // Not on the board yet: the carrier is in the hand, like any id that is not on my board.
    let opposing = hand_named(&state, PlayerId::Two, "unit")[0];
    for source in [carrier, opposing, CardInstanceId(9_999)] {
        for effect in [0, 1, 7] {
            assert_eq!(
                apply(&mut state, activate(source, effect)).unwrap_err(),
                RuleError::SourceNotOnBoard,
                "{source:?} {effect}"
            );
        }
    }
    apply(&mut state, Action::play(carrier)).unwrap();
    let before = state.clone();
    // Index 1 is the `activated` effect: declared, not resolvable yet (temporary code).
    let error = apply(&mut state, activate(carrier, 1)).unwrap_err();
    assert_eq!(error, RuleError::UnsupportedEffect);
    assert_eq!(code(error), "unsupported_effect");
    // `on_play` (0), `continuous` (2) and an index past the list: not an activable effect.
    for index in [0, 2, 3, u32::MAX] {
        assert_eq!(
            apply(&mut state, activate(carrier, index)).unwrap_err(),
            RuleError::UnknownEffect,
            "{index}"
        );
    }
    // A unit without declared effect: the same, an effect that does not exist.
    apply(&mut state, Action::play(other)).unwrap();
    assert_eq!(
        apply(&mut state, activate(other, 0)).unwrap_err(),
        RuleError::UnknownEffect
    );
    // Nothing the refusals did changed the carrier's board (only the second summoning did).
    assert_eq!(state.board(PlayerId::One), vec![carrier, other]);
    assert_ne!(state, before);
    // No declared effect is offered as an action.
    assert!(!legal_actions(&state).iter().any(|a| matches!(
        a,
        Action::Activate {
            source: Some(_),
            ..
        }
    )));
}

#[test]
fn the_new_cards_cost_nothing_in_the_log_of_a_plain_game() {
    // A plain game written with every optional key of a unit spelled out is the same game, with
    // the same journal, as the one written the short way.
    let short = setup_of(
        side(hero(2, 4, 3), |_| "unit"),
        side(hero(1, 4, 3), |_| "unit"),
    );
    let mut value = serde_json::to_value(&short).unwrap();
    for seat in ["one", "two"] {
        for card in value[seat]["cards"].as_array_mut().unwrap() {
            card["kind"] = "unit".into();
        }
    }
    let spelled: GameSetup = serde_json::from_value(value).unwrap();
    assert_eq!(spelled, short);
    let run = |setup: GameSetup| {
        let (mut state, mut journal) = new_game(setup, 9).unwrap();
        for step in 0..200 {
            let legal = legal_actions(&state);
            if state.winner().is_some() || legal.len() == 1 {
                break;
            }
            let action = legal[(step * 5) % (legal.len() - 1)].clone();
            journal.extend(apply(&mut state, action).unwrap());
        }
        serde_json::to_string(&journal).unwrap()
    };
    assert_eq!(run(short), run(spelled));
}
