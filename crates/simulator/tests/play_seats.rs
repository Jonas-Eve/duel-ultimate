//! `play_game_seats`: one policy per seat. `play_game` is its special case (the same policy
//! on both seats), with the same per-seat policy generators and the same results (#190).

use duel_ai::policy_named;
use duel_engine::GameSetup;
use duel_simulator::{
    DEFAULT_MAX_ACTIONS, Policy, play_game, play_game_seats, random_policy, setup_from,
};

fn setups() -> Vec<GameSetup> {
    let fixture = setup_from(
        include_str!("../../../data/fixture/1.0.0/cards.json"),
        include_str!("../../../data/fixture/1.0.0/decks.json"),
        vec!["fixture@1.0.0".to_owned()],
    )
    .expect("the fixture is valid");
    let starter = setup_from(
        include_str!("../../../data/starter/1.1.0/cards.json"),
        include_str!("../../../data/starter/1.1.0/decks.json"),
        vec!["starter@1.1.0".to_owned()],
    )
    .expect("the starter set is valid");
    vec![fixture, starter]
}

#[test]
fn play_game_seats_with_one_policy_twice_equals_play_game() {
    let greedy = policy_named("greedy").expect("greedy is registered");
    let policies: [(&str, &dyn Policy); 2] = [("random", &random_policy), ("greedy", &greedy)];
    for (name, policy) in policies {
        for setup in setups() {
            for seed in 0..50 {
                let mut steps_a = 0usize;
                let mut steps_b = 0usize;
                let single = play_game(setup.clone(), seed, policy, DEFAULT_MAX_ACTIONS, |_, _| {
                    steps_a += 1
                });
                let seats = play_game_seats(
                    setup.clone(),
                    seed,
                    policy,
                    policy,
                    DEFAULT_MAX_ACTIONS,
                    |_, _| steps_b += 1,
                );
                assert_eq!(single, seats, "{name}, seed {seed}");
                assert_eq!(steps_a, steps_b, "{name}, seed {seed}");
            }
        }
    }
}
