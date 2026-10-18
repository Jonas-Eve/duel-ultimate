//! #171: the board limit (`RuleSet::board_limit`), through the public API.
//!
//! A full board refuses a summoning with `RuleError::BoardFull`; the card is then neither listed,
//! nor previewed, nor offered to an AI, and it does not count against exhaustion: the limit acts
//! in `validate` alone. The rules are always written out (`RuleSet { board_limit, ..legacy() }`):
//! the engine default is not what these tests are about.
//!
//! Card ids are assigned in setup order, One's deck first (0..30) then Two's (30..60).

use duel_cards::{CreatureStats, Effect, Hero, HeroEffect};
use duel_engine::{
    Action, CardInstanceId, Event, GameSetup, GameState, Observation, PlayerId, PlayerSetup,
    RuleError, RuleSet, Target, WinReason, apply, legal_actions, new_game, preview_legal_actions,
};
use serde_json::Value;

struct Xorshift(u64);

impl Xorshift {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % u64::try_from(bound).unwrap()).unwrap()
    }
}

fn limited(n: u32) -> RuleSet {
    RuleSet {
        board_limit: Some(n),
        ..RuleSet::legacy()
    }
}

fn id(n: u32) -> CardInstanceId {
    CardInstanceId(n)
}

fn play(n: u32) -> Action {
    Action::Play { card: id(n) }
}

/// A creature of summoning cost 1.
fn unit(force: u32, health: u32, attack_cost: u32) -> CreatureStats {
    CreatureStats {
        cost: 1,
        force,
        attack_cost,
        health,
        provocation: false,
        riposte: 0,
    }
}

fn hero(initiative: u32, start_budget: u32, starting_hand: u32, with_draw: bool) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time: 30,
        starting_hand,
        initiative,
        start_budget,
        health: 10,
        effects: if with_draw {
            vec![HeroEffect {
                name: "Piocher".into(),
                cost: 1,
                effect: Effect::Draw { count: 1 },
            }]
        } else {
            vec![]
        },
    }
}

/// A varied deck: costs 1 to 3, forces 1 to 3, health 1 to 4.
fn mixed(offset: u32) -> Vec<CreatureStats> {
    (0..30)
        .map(|n| {
            let n = n + offset;
            CreatureStats {
                cost: 1 + n % 3,
                force: 1 + n % 3,
                attack_cost: 1 + n % 2,
                health: 1 + n % 4,
                provocation: n.is_multiple_of(5),
                riposte: n % 3,
            }
        })
        .collect()
}

fn game(
    rules: RuleSet,
    seed: u64,
    one: (Hero, Vec<CreatureStats>),
    two: (Hero, Vec<CreatureStats>),
) -> (GameState, Vec<Event>) {
    new_game(
        GameSetup {
            one: PlayerSetup {
                hero: one.0,
                cards: one.1,
                card_names: Vec::new(),
            },
            two: PlayerSetup {
                hero: two.0,
                cards: two.1,
                card_names: Vec::new(),
            },
            data_versions: vec!["fixture@1.0.0".into()],
            rules,
        },
        seed,
    )
    .unwrap()
}

/// One (active, budget 20) has its whole deck in hand: every card costs 1, so it stays active
/// through the first summonings. Two is a spectator with 10 health.
fn arena(rules: RuleSet, starting_hand: u32, with_draw: bool) -> GameState {
    let deck = vec![unit(2, 5, 1); 30];
    game(
        rules,
        7,
        (hero(2, 20, starting_hand, with_draw), deck.clone()),
        (hero(1, 1, 30, false), deck),
    )
    .0
}

fn is_play(action: &Action) -> bool {
    matches!(action, Action::Play { .. })
}

#[test]
fn a_full_board_refuses_a_summoning_with_board_full_and_leaves_the_state_intact() {
    let mut state = arena(limited(2), 30, false);
    apply(&mut state, play(0)).unwrap();
    apply(&mut state, play(1)).unwrap();
    assert_eq!(state.board(PlayerId::One).len(), 2);
    assert_eq!(state.active(), PlayerId::One);

    let before = state.clone(); // the generator included
    let error = apply(&mut state, play(2)).unwrap_err();
    assert_eq!(error, RuleError::BoardFull);
    assert_eq!(error.code(), "board_full");
    assert_eq!(state, before, "a refusal changes nothing");
}

#[test]
fn the_order_of_the_checks_keeps_card_not_in_hand_first() {
    // Four cards in hand, the rest in the pile: every kind of id that is not in the hand.
    let mut state = arena(limited(1), 4, false);
    let on_board = state.hand(PlayerId::One)[0];
    apply(&mut state, Action::Play { card: on_board }).unwrap();
    assert_eq!(state.board(PlayerId::One), vec![on_board]);
    let in_hand = state.hand(PlayerId::One)[0];
    let in_pile = (0..30)
        .map(id)
        .find(|c| *c != on_board && !state.hand(PlayerId::One).contains(c))
        .unwrap();

    // Control: a card of the hand meets the full board.
    assert_eq!(
        apply(&mut state.clone(), Action::Play { card: in_hand }),
        Err(RuleError::BoardFull)
    );
    for (what, card) in [
        ("unknown", id(10_000)),
        ("opposing hand", id(30)),
        ("own pile", in_pile),
        ("own board", on_board),
    ] {
        let before = state.clone();
        assert_eq!(
            apply(&mut state, Action::Play { card }),
            Err(RuleError::CardNotInHand),
            "{what}"
        );
        assert_eq!(state, before, "{what}");
    }
}

#[test]
fn a_card_that_cannot_be_played_is_not_listed_previewed_or_offered_to_the_ai() {
    let mut state = arena(limited(2), 20, true);
    // Control: with a free place, the summonings are listed, previewed and offered.
    let first = state.hand(PlayerId::One)[0];
    apply(&mut state, Action::Play { card: first }).unwrap();
    let observation = Observation::of(&state).unwrap();
    assert!(legal_actions(&state).iter().any(is_play));
    assert!(
        preview_legal_actions(&state)
            .iter()
            .any(|p| is_play(&p.action))
    );
    assert!(observation.actions().iter().any(is_play));
    drop(observation);

    let second = state.hand(PlayerId::One)[0];
    apply(&mut state, Action::Play { card: second }).unwrap();
    assert!(!legal_actions(&state).iter().any(is_play));
    assert!(
        !preview_legal_actions(&state)
            .iter()
            .any(|p| is_play(&p.action))
    );
    assert!(
        !Observation::of(&state)
            .unwrap()
            .actions()
            .iter()
            .any(is_play)
    );
    // The hand is not empty, and other actions remain: the hero effect and the attacks.
    assert!(state.hand_len(PlayerId::One) > 0);
    assert!(
        legal_actions(&state)
            .iter()
            .any(|a| matches!(a, Action::Activate { .. }))
    );
    assert!(
        legal_actions(&state)
            .iter()
            .any(|a| matches!(a, Action::Attack { .. }))
    );
}

#[test]
fn a_creature_dying_frees_a_place() {
    // Limit 1; with the riposte of the hero, a non-lethal hit on the hero destroys the attacker.
    let rules = RuleSet {
        hero_riposte: true,
        board_limit: Some(1),
    };
    let mut state = arena(rules, 30, false);
    apply(&mut state, play(0)).unwrap();
    assert!(!legal_actions(&state).iter().any(is_play));
    let events = apply(
        &mut state,
        Action::Attack {
            attacker: id(0),
            target: Target::Hero,
        },
    )
    .unwrap();
    assert!(events.iter().any(|e| matches!(e, Event::Died { .. })));
    assert!(state.board(PlayerId::One).is_empty());
    assert_eq!(state.active(), PlayerId::One);
    assert!(legal_actions(&state).contains(&play(1)));
    apply(&mut state, play(1)).unwrap();
    assert_eq!(state.board(PlayerId::One), vec![id(1)]);
}

/// Plays a game with a pseudo-random policy; the whole journal, `GameStarted` included.
fn journal(rules: RuleSet, seed: u64) -> (GameState, Vec<Value>) {
    let (mut state, mut log) = game(
        rules,
        seed,
        (hero(1 + (seed % 2) as u32, 3, 8, true), mixed(0)),
        (hero(1, 2, 6, true), mixed(1)),
    );
    let mut rng = Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    while state.winner().is_none() {
        let actions: Vec<Action> = legal_actions(&state)
            .into_iter()
            .filter(|a| *a != Action::Concede)
            .collect();
        let action = actions
            .get(rng.below(actions.len().max(1)))
            .copied()
            .unwrap_or(Action::Concede);
        log.extend(apply(&mut state, action).unwrap());
    }
    let mut values: Vec<Value> = log
        .iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect();
    // Without the `rules` key of `GameStarted`, the only difference the options may make here.
    if let Some(started) = values[0].get_mut("game_started") {
        started.as_object_mut().unwrap().remove("rules");
    }
    (state, values)
}

#[test]
fn an_unreached_limit_changes_nothing() {
    for seed in 0..20 {
        let (plain_state, plain) = journal(RuleSet::legacy(), seed);
        // A limit equal to the size of a deck never binds; an explicit `None` is the legacy.
        for rules in [
            limited(30),
            RuleSet {
                board_limit: None,
                ..RuleSet::legacy()
            },
        ] {
            let (state, log) = journal(rules, seed);
            assert_eq!(log, plain, "seed {seed} {rules:?}");
            assert_eq!(state.winner(), plain_state.winner(), "seed {seed}");
        }
    }
}

#[test]
fn a_limit_that_is_reached_does_change_the_games() {
    let differing = (0..20)
        .filter(|seed| journal(limited(1), *seed).1 != journal(RuleSet::legacy(), *seed).1)
        .count();
    assert!(differing >= 15, "{differing} of 20 games differ");
}

#[test]
fn the_limit_alone_never_exhausts_a_player() {
    let (mut full_boards, mut exhaustions) = (0, 0);
    for board_limit in [1, 2, 5] {
        for hero_riposte in [false, true] {
            let rules = RuleSet {
                hero_riposte,
                board_limit: Some(board_limit),
            };
            for seed in 0..60u64 {
                let mut state = game(
                    rules,
                    seed,
                    (hero(1 + (seed % 2) as u32, 3, 8, seed % 3 != 0), mixed(0)),
                    (hero(1, 2, 6, seed % 3 != 0), mixed(1)),
                )
                .0;
                let mut rng = Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
                let mut steps = 0;
                while state.winner().is_none() {
                    let active = state.active();
                    let legal = legal_actions(&state);
                    if state.board(active).len() >= board_limit as usize {
                        full_boards += 1;
                        assert!(
                            legal.iter().any(|a| matches!(a, Action::Attack { .. })),
                            "{rules:?}, seed {seed}: a full board but no attack"
                        );
                    }
                    let playable: Vec<Action> = legal
                        .into_iter()
                        .filter(|a| *a != Action::Concede)
                        .collect();
                    assert!(
                        !playable.is_empty(),
                        "{rules:?}, seed {seed}: exhausted yet not over"
                    );
                    apply(&mut state, playable[rng.below(playable.len())]).unwrap();
                    steps += 1;
                    assert!(
                        steps < 3000,
                        "{rules:?}, seed {seed}: the game does not end"
                    );
                }
                if let Some((winner, WinReason::Exhaustion)) = state.winner() {
                    exhaustions += 1;
                    let loser = winner.opponent();
                    assert!(
                        state.board(loser).is_empty(),
                        "{rules:?}, seed {seed}: exhausted with a living creature"
                    );
                }
            }
        }
    }
    assert!(full_boards > 500, "full boards seen: {full_boards}");
    assert!(exhaustions > 0, "exhaustions seen: {exhaustions}");
}

#[test]
fn no_randomness_is_consumed_by_a_full_board() {
    // Two games, one with the limit: the same actions and, in the limited one, a refused
    // summoning in the middle. The cards drawn afterwards (the order of the pile, which only
    // the generator decided) are the same.
    let run = |rules: RuleSet, refused: bool| {
        let mut state = arena(rules, 4, true);
        let first = state.hand(PlayerId::One)[0];
        apply(&mut state, Action::Play { card: first }).unwrap();
        if refused {
            let other = state.hand(PlayerId::One)[0];
            assert_eq!(
                apply(&mut state, Action::Play { card: other }),
                Err(RuleError::BoardFull)
            );
        }
        let mut drawn = Vec::new();
        for _ in 0..6 {
            let effect = Action::Activate { effect: 0 };
            drawn.extend(apply(&mut state, effect).unwrap());
        }
        drawn
    };
    assert_eq!(run(limited(1), true), run(RuleSet::legacy(), false));
}
