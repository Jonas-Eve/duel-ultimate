//! `Observation` through the public API (#140): the active seat's window on a running game.

use duel_engine::{
    Action, GameSetup, GameState, Observation, PlayerId, apply, game_setup_from_data,
    legal_actions, new_game, player_view,
};

const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");
const STARTER_CARDS: &str = include_str!("../../../data/starter/1.0.0/cards.json");
const STARTER_DECKS: &str = include_str!("../../../data/starter/1.0.0/decks.json");

struct Xorshift(u64);

impl Xorshift {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % u64::try_from(bound).unwrap()).unwrap()
    }
}

fn setup_of(cards: &str, decks: &str, one: &str, two: &str) -> GameSetup {
    game_setup_from_data(cards, decks, one, two, vec!["test@1".into()]).unwrap()
}

fn setups() -> Vec<GameSetup> {
    vec![
        setup_of(STARTER_CARDS, STARTER_DECKS, "rush", "bulwark"),
        setup_of(STARTER_CARDS, STARTER_DECKS, "bulwark", "rush"),
        setup_of(FIXTURE_CARDS, FIXTURE_DECKS, "aggro", "control"),
    ]
}

/// Plays `seed`'s game at random (including `Concede` once in a while is not needed: it would
/// only end the game) and shows every running state to `check`; then the final state.
fn for_every_state(seed: u64, mut check: impl FnMut(&GameState)) -> GameState {
    let setups = setups();
    let setup = setups[usize::try_from(seed).unwrap() % setups.len()].clone();
    let (mut state, _) = new_game(setup, seed).unwrap();
    let mut rng = Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    while state.winner().is_none() {
        check(&state);
        let playable: Vec<Action> = legal_actions(&state)
            .into_iter()
            .filter(|a| *a != Action::Concede)
            .collect();
        apply(&mut state, playable[rng.below(playable.len())]).unwrap();
    }
    state
}

#[test]
fn a_finished_game_has_no_observation() {
    for seed in 0..10 {
        let last = for_every_state(seed, |_| {});
        assert!(last.winner().is_some());
        assert!(Observation::of(&last).is_none(), "seed {seed}");
    }
}

#[test]
fn the_actions_are_the_legal_actions_without_concede_in_the_same_order() {
    let mut steps = 0;
    for seed in 0..50 {
        for_every_state(seed, |state| {
            let expected: Vec<Action> = legal_actions(state)
                .into_iter()
                .filter(|a| *a != Action::Concede)
                .collect();
            let obs = Observation::of(state).expect("the game runs");
            assert_eq!(obs.actions(), expected.as_slice(), "seed {seed}");
            assert!(!obs.actions().contains(&Action::Concede));
            steps += 1;
        });
    }
    assert!(steps > 500, "{steps} steps observed");
}

#[test]
fn the_seat_is_the_active_player() {
    for seed in 0..10 {
        for_every_state(seed, |state| {
            assert_eq!(Observation::of(state).unwrap().seat(), state.active());
        });
    }
    let (state, _) = new_game(setups().remove(0), 3).unwrap();
    assert_eq!(Observation::of(&state).unwrap().seat(), PlayerId::One);
}

#[test]
fn the_view_is_the_player_view_of_the_active_seat() {
    for seed in 0..10 {
        for_every_state(seed, |state| {
            let obs = Observation::of(state).unwrap();
            let expected = serde_json::to_string(&player_view(state, state.active())).unwrap();
            assert_eq!(serde_json::to_string(&obs.view()).unwrap(), expected);
        });
    }
}

#[test]
fn observing_changes_neither_the_state_nor_the_generator() {
    for seed in 0..10 {
        for_every_state(seed, |state| {
            let before = state.clone();
            {
                let obs = Observation::of(state).unwrap();
                let _ = obs.actions();
                let _ = obs.view();
            }
            assert_eq!(state, &before);
        });
    }
}

#[test]
fn observing_does_not_change_what_follows() {
    // The same game, observed at every step or never, ends the same way.
    for seed in 0..10 {
        let setups = setups();
        let setup = setups[usize::try_from(seed).unwrap() % setups.len()].clone();
        let (mut watched, _) = new_game(setup.clone(), seed).unwrap();
        let (mut plain, _) = new_game(setup, seed).unwrap();
        while plain.winner().is_none() {
            let action = legal_actions(&plain)[0];
            let obs = Observation::of(&watched).unwrap();
            let _ = obs.view();
            assert_eq!(apply(&mut watched, action), apply(&mut plain, action));
        }
        assert_eq!(watched, plain);
    }
}
