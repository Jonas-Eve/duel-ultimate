//! The greedy policy, the noise combinator and the policy registry (#159, story #126).

mod support;

use duel_ai::{POLICIES, Policy, choose, greedy_policy, noisy, policy_named, random_policy};
use duel_engine::{
    Action, GameResult, GameSetup, GameState, Observation, PlayerId, Target, WinReason, apply,
    game_setup_from_data, new_game,
};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use support::*;

const CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
const DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");

fn rng() -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(0)
}

/// The action the greedy policy picks in `state`.
fn pick(state: &GameState) -> Action {
    choose(state, &greedy_policy, &mut rng()).expect("an action")
}

// Story line: one registry of policy names, read by the simulator and the binding.
#[test]
fn registry_resolves_every_name_and_nothing_else() {
    let mut seen = Vec::new();
    for (name, _) in POLICIES {
        assert!(policy_named(name).is_some(), "{name} resolves");
        assert!(!name.is_empty());
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'),
            "{name} fits the report's name alphabet"
        );
        assert!(!seen.contains(&name), "{name} is listed once");
        seen.push(name);
    }
    assert_eq!(seen, ["random", "greedy"]);
    for bad in ["Greedy", "", "random ", "noisy", "greedy-noise20"] {
        assert!(policy_named(bad).is_none(), "{bad:?} is not a policy");
    }
}

#[test]
fn the_registry_entries_are_the_exported_policies() {
    let state = game_from(
        clones(&Hero::first(10), &card("a", 1, 1, 1), 5),
        clones(&Hero::second(10), &card("b", 1, 1, 1), 5),
    );
    let obs = Observation::of(&state).unwrap();
    let greedy = policy_named("greedy").unwrap();
    let random = policy_named("random").unwrap();
    assert_eq!(greedy(&obs, &mut rng()), greedy_policy(&obs, &mut rng()));
    assert_eq!(random(&obs, &mut rng()), random_policy(&obs, &mut rng()));
}

// Story line: a lethal hit comes first, without any coded "hit the hero" rule.
#[test]
fn lethal_on_the_hero_comes_first() {
    let state = opening(
        (
            Hero::first(10),
            vec![card("hitter", 1, 5, 3), card("big", 1, 9, 9)],
        ),
        (Hero::second(5), vec![card("x", 1, 2, 2)]),
        &["hitter"],
        &["x"],
    );
    let hitter = on_board(&state, PlayerId::One, "hitter");
    assert_eq!(
        pick(&state),
        Action::Attack {
            attacker: hitter,
            target: Target::Hero
        }
    );
}

// Story line: a provocateur must be killed before the hero can be reached.
#[test]
fn kills_a_provocateur_before_hitting_the_hero() {
    let state = opening(
        (
            Hero::first(10),
            vec![card("hitter", 1, 4, 3), card("mid", 1, 2, 2)],
        ),
        (
            Hero::second(10),
            vec![
                card("wall", 1, 1, 2).provocation(),
                card("bystander", 1, 3, 3),
            ],
        ),
        &["hitter"],
        &["wall", "bystander"],
    );
    let hitter = on_board(&state, PlayerId::One, "hitter");
    let wall = on_board(&state, PlayerId::Two, "wall");
    assert_eq!(
        pick(&state),
        Action::Attack {
            attacker: hitter,
            target: Target::Creature(wall)
        }
    );
}

// Story line: the outcome decides: a kill that costs nothing beats a trade that loses the attacker.
#[test]
fn kills_a_creature_without_losing_its_own_before_a_trade_that_loses_it() {
    let state = opening(
        (Hero::first(30), vec![card("hitter", 1, 3, 4)]),
        (
            Hero::second(30),
            vec![card("soft", 1, 2, 3), card("thorny", 1, 3, 3).riposte(4)],
        ),
        &["hitter"],
        &["soft", "thorny"],
    );
    let hitter = on_board(&state, PlayerId::One, "hitter");
    let soft = on_board(&state, PlayerId::Two, "soft");
    assert_eq!(
        pick(&state),
        Action::Attack {
            attacker: hitter,
            target: Target::Creature(soft)
        }
    );
}

// #170: with the hero's riposte, a hit on the hero that does not kill it costs the attacker.
// The sacrifice is already weighed by `Observation::outcome` (the own board goes down), so no
// weight of the evaluation changes: the greedy policy simply stops finding the hit free.
fn riposting() -> serde_json::Value {
    serde_json::json!({"hero_riposte": true, "board_limit": null})
}

/// One has summoned its `hitter` (force `force`, health 5) and is still to move with a
/// `recruit` in hand; Two has nothing on the board, so the hero is the only target. Two's hero
/// has `two_health` health.
fn hitter_ready(two_health: u32, force: u32, rules: serde_json::Value) -> GameState {
    let mut state = scenario_with_rules(
        (
            Hero::first(30),
            vec![card("hitter", 1, force, 5), card("recruit", 1, 1, 1)],
        ),
        (Hero::second(two_health), vec![]),
        rules,
    );
    play(&mut state, "hitter");
    assert_eq!(state.active(), PlayerId::One);
    state
}

#[test]
fn with_the_hero_riposte_a_non_lethal_hit_on_the_hero_is_a_sacrifice_greedy_refuses() {
    let state = hitter_ready(30, 3, riposting());
    let recruit = in_hand(&state, PlayerId::One, "recruit");
    assert_eq!(pick(&state), Action::play(recruit));
}

#[test]
fn without_the_hero_riposte_the_same_hit_is_taken() {
    // The control of the test above: the very same position, the option written off.
    let state = hitter_ready(
        30,
        3,
        serde_json::json!({"hero_riposte": false, "board_limit": null}),
    );
    let hitter = on_board(&state, PlayerId::One, "hitter");
    assert_eq!(
        pick(&state),
        Action::Attack {
            attacker: hitter,
            target: Target::Hero
        }
    );
}

#[test]
fn with_the_hero_riposte_greedy_still_takes_a_lethal_hit() {
    // Hero health <= force: no riposte, the game is won, whatever the other options.
    let state = hitter_ready(5, 5, riposting());
    let hitter = on_board(&state, PlayerId::One, "hitter");
    assert_eq!(
        pick(&state),
        Action::Attack {
            attacker: hitter,
            target: Target::Hero
        }
    );
}

// Story line: hitting the hero is a term like another, not a rule.
#[test]
fn hitting_the_hero_is_not_a_coded_priority() {
    let state = opening(
        (Hero::first(30), vec![card("hitter", 1, 2, 4)]),
        (Hero::second(50), vec![card("soft", 1, 3, 2)]),
        &["hitter"],
        &["soft"],
    );
    let hitter = on_board(&state, PlayerId::One, "hitter");
    let soft = on_board(&state, PlayerId::Two, "soft");
    assert_eq!(
        pick(&state),
        Action::Attack {
            attacker: hitter,
            target: Target::Creature(soft)
        }
    );
}

// Story line: the card to play is ranked on the statistics of its view only.
#[test]
fn plays_the_best_payable_card() {
    let mut hero = Hero::first(20);
    hero.start_budget = 5;
    let state = scenario(
        (
            hero,
            vec![
                card("weak", 1, 1, 1),
                card("strong", 2, 4, 4),
                card("middling", 1, 2, 2),
            ],
        ),
        (Hero::second(20), vec![]),
    );
    let strong = in_hand(&state, PlayerId::One, "strong");
    assert_eq!(pick(&state), Action::play(strong));
}

// Story line: the pile cannot be evaluated; drawing is chosen on the counts.
#[test]
fn draws_when_the_hand_is_empty() {
    let mut hero = Hero::first(20);
    hero.effects = vec![(2, 1), (2, 3)];
    let one = clones(&hero, &card("c", 1, 1, 1), 0);
    let two = clones(&Hero::second(20), &card("d", 1, 1, 1), 5);
    let state = game_from(one, two);
    assert_eq!(state.active(), PlayerId::One);
    assert_eq!(pick(&state), Action::activate(1));
}

// Story line: never walks into a loss that the preview announces, when another action exists.
#[test]
fn never_hands_over_into_a_loss_by_exhaustion_when_another_action_exists() {
    let mut one = Hero::first(30);
    one.start_budget = 3;
    let mut two = Hero::second(30);
    two.start_budget = 3;
    let hitter = card("hitter", 1, 1, 1).attack_cost(3);
    // The options off: the hitter survives its hit on the hero (the scenario is a trap by
    // exhaustion; the sacrifice by hero riposte is weighed by the dry run, measured in #127 T4).
    let mut state = legacy_game_from(
        clones(&one, &hitter, 1),
        side(&two, &[card("t", 1, 5, 1).riposte(5)], 30),
    );
    play(&mut state, "hitter");
    let me = on_board(&state, PlayerId::One, "hitter");
    attack(&mut state, me, Target::Hero);
    play(&mut state, "t");
    pass(&mut state);
    assert_eq!(state.active(), PlayerId::One);
    let t = on_board(&state, PlayerId::Two, "t");
    let suicide = Action::Attack {
        attacker: me,
        target: Target::Creature(t),
    };
    let obs = Observation::of(&state).unwrap();
    let index = obs.actions().iter().position(|a| *a == suicide).unwrap();
    assert_eq!(
        obs.preview(index).unwrap().ends_game,
        Some(GameResult {
            winner: PlayerId::Two,
            reason: WinReason::Exhaustion
        }),
        "the scenario is a real trap"
    );
    assert!(obs.actions().len() > 1);
    assert_ne!(pick(&state), suicide);
}

// Story line: determinism, and the generator is left alone.
#[test]
fn greedy_is_deterministic_and_leaves_the_generator_alone() {
    for seed in 0..20u64 {
        let setup = starter_setup();
        let (mut state, _) = new_game(setup, seed).unwrap();
        for step in 0..12 {
            let Some(obs) = Observation::of(&state) else {
                break;
            };
            let mut used = ChaCha8Rng::seed_from_u64(seed);
            let untouched = ChaCha8Rng::seed_from_u64(seed);
            let first = greedy_policy(&obs, &mut used);
            assert_eq!(first, greedy_policy(&obs, &mut used), "seed {seed}");
            assert_eq!(used, untouched, "seed {seed} step {step}");
            let action = obs.actions().get(first).cloned().expect("in range");
            apply(&mut state, action).unwrap();
        }
    }
}

fn starter_setup() -> GameSetup {
    game_setup_from_data(CARDS, DECKS, "rush", "bulwark", vec![]).unwrap()
}

// Story line: it reads nothing hidden: the opposing hand and pile (content, order) are out of reach.
#[test]
fn greedy_sees_nothing_hidden() {
    for seed in 0..30u64 {
        let a = starter_setup();
        let mut b = a.clone();
        // Same hero, same counts, other cards in another order.
        b.two.cards.rotate_left(1 + (seed as usize % 7));
        for card in b.two.cards.iter_mut().step_by(3) {
            let duel_cards::CardSpec::Unit(unit) = card else {
                panic!("a unit was expected");
            };
            unit.stats.force += 1;
            unit.stats.health += 1;
        }
        b.two.card_names.clear();
        let mut a = a;
        a.two.card_names.clear();
        let (mut sa, _) = new_game(a, seed).unwrap();
        let (mut sb, _) = new_game(b, seed).unwrap();
        assert_eq!(sa.active(), sb.active());
        for _ in 0..(seed % 4) {
            if sa.active() != PlayerId::One || sa.winner().is_some() {
                break;
            }
            let (oa, ob) = (Observation::of(&sa).unwrap(), Observation::of(&sb).unwrap());
            assert_eq!(oa.actions(), ob.actions());
            let (ia, ib) = (
                greedy_policy(&oa, &mut rng()),
                greedy_policy(&ob, &mut rng()),
            );
            assert_eq!(ia, ib, "seed {seed}");
            let action = oa.actions()[ia].clone();
            apply(&mut sa, action.clone()).unwrap();
            apply(&mut sb, action).unwrap();
        }
    }
}

// Story line: the noise combinator, its generator use pinned and its limits.
#[test]
fn noisy_consumption_and_golden_values() {
    let state = game_from(
        clones(&Hero::first(10), &card("a", 1, 1, 1), 5),
        clones(&Hero::second(10), &card("b", 1, 1, 1), 5),
    );
    let obs = Observation::of(&state).unwrap();
    let n = obs.actions().len();
    assert!(n > 2);

    // percent 0 is the inner policy, whatever the generator says.
    let zero = noisy(greedy_policy, 0);
    let mut r = rng();
    for _ in 0..200 {
        assert_eq!(zero.choose(&obs, &mut r), greedy_policy(&obs, &mut rng()));
    }

    // percent 100 never reads the inner policy: a panicking one is never called.
    let always = noisy(
        |_: &Observation<'_>, _: &mut ChaCha8Rng| -> usize { unreachable!("inner is not read") },
        100,
    );
    let capped = noisy(
        |_: &Observation<'_>, _: &mut ChaCha8Rng| -> usize { unreachable!("inner is not read") },
        250,
    );
    let mut r = rng();
    let mut w = rng();
    for _ in 0..50 {
        let picked = always.choose(&obs, &mut r);
        let coin = w.random_range(0..100u8);
        assert!(coin < 100);
        assert_eq!(picked, random_policy(&obs, &mut w));
    }
    assert_eq!(r, w, "one draw for the coin, one for the random choice");
    let (mut c1, mut c2) = (rng(), rng());
    assert_eq!(capped.choose(&obs, &mut c1), always.choose(&obs, &mut c2));

    // Exact consumption: a coin always, a second draw only on noise.
    let half = noisy(greedy_policy, 50);
    let mut r = ChaCha8Rng::seed_from_u64(7);
    let mut witness = ChaCha8Rng::seed_from_u64(7);
    let mut picks = Vec::new();
    for _ in 0..12 {
        picks.push(half.choose(&obs, &mut r));
        let coin = witness.random_range(0..100u8);
        if coin < 50 {
            let _ = random_policy(&obs, &mut witness);
        }
    }
    assert_eq!(r, witness);
    // Golden values: (seed 7, percent 50) on this state.
    assert_eq!(picks, GOLDEN_NOISY_50, "noisy(greedy, 50), seed 7");
}

const GOLDEN_NOISY_50: [usize; 12] = [0, 0, 3, 3, 3, 0, 1, 1, 0, 1, 4, 1];

/// Plays one game; returns the winner and the number of actions taken.
fn play_game(one: &dyn Policy, two: &dyn Policy, seed: u64) -> (PlayerId, usize) {
    let (mut state, _) = new_game(starter_setup(), seed).unwrap();
    let mut rngs = [
        duel_ai::policy_rng(seed, PlayerId::One),
        duel_ai::policy_rng(seed, PlayerId::Two),
    ];
    for count in 0..2000 {
        if let Some((winner, _)) = state.winner() {
            return (winner, count);
        }
        let (policy, rng) = match state.active() {
            PlayerId::One => (one, &mut rngs[0]),
            PlayerId::Two => (two, &mut rngs[1]),
        };
        let action = choose(&state, policy, rng).expect("never stuck");
        assert_ne!(action, Action::Concede);
        apply(&mut state, action).expect("the engine accepts it");
    }
    panic!("seed {seed}: no end in 2000 actions");
}

// Story line: never stalls, never concedes, every game ends.
#[test]
fn greedy_never_stalls_over_200_games_against_random() {
    for seed in 0..200u64 {
        if seed % 2 == 0 {
            play_game(&greedy_policy, &random_policy, seed);
        } else {
            play_game(&random_policy, &greedy_policy, seed);
        }
    }
}

// Story line: the greedy policy is actually better than chance on the starter set.
#[test]
fn greedy_beats_random_on_starter() {
    let mut wins = 0;
    for seed in 0..200u64 {
        let greedy_seat = if seed < 100 {
            PlayerId::One
        } else {
            PlayerId::Two
        };
        let (winner, _) = if greedy_seat == PlayerId::One {
            play_game(&greedy_policy, &random_policy, seed)
        } else {
            play_game(&random_policy, &greedy_policy, seed)
        };
        wins += usize::from(winner == greedy_seat);
    }
    println!("greedy won {wins} of 200");
    assert!(wins >= 140, "greedy won only {wins} of 200 games");
}
