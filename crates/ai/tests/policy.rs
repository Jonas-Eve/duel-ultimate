//! `random_policy` and the `choose` harness (#140).

use duel_ai::{AiError, Policy, choose, random_policy};
use duel_engine::{Action, GameState, Observation, PlayerId, apply, game_setup_from_data, new_game};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;

const CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
const DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");

fn game(seed: u64) -> GameState {
    let setup = game_setup_from_data(CARDS, DECKS, "rush", "bulwark", vec![]).unwrap();
    new_game(setup, seed).unwrap().0
}

#[test]
fn random_policy_consumes_the_generator_like_the_former_formula() {
    let state = game(1);
    let obs = Observation::of(&state).unwrap();
    let n = obs.actions().len();
    assert!(n > 1);
    let mut new = ChaCha8Rng::seed_from_u64(77);
    let mut old = ChaCha8Rng::seed_from_u64(77);
    for _ in 0..200 {
        assert_eq!(random_policy(&obs, &mut new), old.random_range(0..n));
    }
    // After the same draws the two generators are at the same point.
    assert_eq!(new.random_range(0..1000u32), old.random_range(0..1000u32));
}

#[test]
fn a_policy_is_any_closure_over_an_observation() {
    let state = game(1);
    let first = |_: &Observation<'_>, _: &mut ChaCha8Rng| 0;
    let policy: &dyn Policy = &first;
    let mut rng = ChaCha8Rng::seed_from_u64(0);
    let obs = Observation::of(&state).unwrap();
    assert_eq!(policy.choose(&obs, &mut rng), 0);
    let _: &dyn Policy = &random_policy;
}

#[test]
fn choose_returns_an_action_the_engine_accepts() {
    let mut state = game(2);
    let mut rng = ChaCha8Rng::seed_from_u64(5);
    for _ in 0..30 {
        if state.winner().is_some() {
            break;
        }
        let action = choose(&state, &random_policy, &mut rng).unwrap();
        assert_ne!(action, Action::Concede);
        apply(&mut state, action).unwrap();
    }
}

#[test]
fn choose_on_a_finished_game_has_no_playable_action() {
    let mut state = game(2);
    apply(&mut state, Action::Concede).unwrap();
    assert_eq!(state.winner().map(|(w, _)| w), Some(PlayerId::Two));
    let mut rng = ChaCha8Rng::seed_from_u64(0);
    assert_eq!(
        choose(&state, &random_policy, &mut rng),
        Err(AiError::NoPlayableAction)
    );
}

#[test]
fn an_index_outside_the_list_is_an_error_never_a_panic() {
    let state = game(2);
    let len = Observation::of(&state).unwrap().actions().len();
    let past_the_end = |obs: &Observation<'_>, _: &mut ChaCha8Rng| obs.actions().len();
    let mut rng = ChaCha8Rng::seed_from_u64(0);
    assert_eq!(
        choose(&state, &past_the_end, &mut rng),
        Err(AiError::IndexOutOfRange { index: len, len })
    );
    let huge = |_: &Observation<'_>, _: &mut ChaCha8Rng| usize::MAX;
    assert_eq!(
        choose(&state, &huge, &mut rng),
        Err(AiError::IndexOutOfRange {
            index: usize::MAX,
            len
        })
    );
}
