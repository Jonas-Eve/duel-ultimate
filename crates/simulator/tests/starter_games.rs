//! Games between the decks of the `starter` set (`data/starter/1.0.0/`), through the public
//! library API; the files are embedded the way `crates/engine-wasm/tests/e2e_duel.rs` does.

use duel_engine::{
    Action, Event, GameSetup, GameState, PlayerId, RuleSet, Target, WinReason, apply,
    legal_actions, new_game,
};
use duel_simulator::{DEFAULT_MAX_ACTIONS, GameRecord, play_game, random_policy, setup_from};

/// One game with the random policy and the default cap, showing `on_step` each step.
fn play_inspected(
    setup: GameSetup,
    seed: u64,
    on_step: impl FnMut(&[Action], &[Event]),
) -> GameRecord {
    play_game(setup, seed, &random_policy, DEFAULT_MAX_ACTIONS, on_step)
}

fn play_with(setup: GameSetup, seed: u64) -> GameRecord {
    play_inspected(setup, seed, |_, _| {})
}

/// How the game ended; an interrupted game (the cap) has no end.
fn reason_of(record: &GameRecord) -> Option<WinReason> {
    record.winner.map(|(_, reason)| reason)
}

fn started_versions(setup: GameSetup) -> Vec<String> {
    let (_, events) = new_game(setup, 0).expect("the setup is valid");
    events
        .into_iter()
        .find_map(|event| match event {
            Event::GameStarted { data_versions, .. } => Some(data_versions),
            _ => None,
        })
        .expect("a game starts with GameStarted")
}
fn starter_setup() -> GameSetup {
    setup_from(
        include_str!("../../../data/starter/1.0.0/cards.json"),
        include_str!("../../../data/starter/1.0.0/decks.json"),
        vec!["starter@1.0.0".to_owned()],
    )
    .expect("the starter set is valid")
}

#[test]
fn the_starter_setup_is_valid() {
    let setup = starter_setup();
    assert_eq!(setup.one.cards.len(), 30);
    assert_eq!(setup.two.cards.len(), 30);
    assert_eq!(setup.one.hero.name, "Capitaine d'avant-garde");
    assert_eq!(setup.two.hero.name, "Gardienne du rempart");
    assert_eq!(started_versions(setup), ["starter@1.0.0"]);
}
/// The starter decks in both orders: rush as One, then as Two.
fn starter_setups() -> [GameSetup; 2] {
    let setup = starter_setup();
    let swapped = GameSetup {
        one: setup.two.clone(),
        two: setup.one.clone(),
        data_versions: setup.data_versions.clone(),
        rules: RuleSet::default(),
    };
    [setup, swapped]
}
#[test]
fn starter_games_all_end_by_a_hero_going_down_or_exhaustion_and_use_both_mechanics() {
    let (mut hero_down, mut deaths, mut ripostes, mut withheld) = (0, 0, 0, 0);
    for setup in starter_setups() {
        for seed in 0..200 {
            let outcome = play_inspected(setup.clone(), seed, |actions, events| {
                let attacks = |target: fn(&Target) -> bool| {
                    actions.iter().any(
                        |action| matches!(action, Action::Attack { target: t, .. } if target(t)),
                    )
                };
                if attacks(|t| matches!(t, Target::Creature(_))) && !attacks(|t| *t == Target::Hero)
                {
                    withheld += 1;
                }
                ripostes += events
                    .iter()
                    .filter(|event| matches!(event, Event::Riposted { .. }))
                    .count();
            });
            match reason_of(&outcome) {
                Some(WinReason::HeroDown) => hero_down += 1,
                Some(WinReason::Exhaustion) => {}
                Some(WinReason::Concede) | None => {
                    panic!("seed {seed}: conceded or hit the step cap")
                }
            }
            deaths += outcome.deaths;
        }
    }
    assert!(hero_down > 0, "heroes fall");
    assert!(deaths > 0, "creatures die");
    assert!(ripostes > 0, "ripostes happen");
    assert!(
        withheld > 0,
        "a provoking creature hides the hero from attackers"
    );
}

#[test]
fn a_starter_seed_always_gives_the_same_game() {
    for setup in starter_setups() {
        for seed in 0..20 {
            assert_eq!(
                play_with(setup.clone(), seed),
                play_with(setup.clone(), seed)
            );
        }
    }
}

/// Without any randomness: the first creature to summon, else the first attack offered
/// (the hero first, or the provoking creature when there is one), else the first effect.
fn scripted(state: &GameState) -> Action {
    let actions: Vec<Action> = legal_actions(state)
        .into_iter()
        .filter(|action| *action != Action::Concede)
        .collect();
    let first = |wanted: fn(&Action) -> bool| actions.iter().find(|a| wanted(a)).cloned();
    first(|a| matches!(a, Action::Play { .. }))
        .or_else(|| first(|a| matches!(a, Action::Attack { .. })))
        .or_else(|| first(|a| matches!(a, Action::Activate { .. })))
        .expect("a running game always offers an action")
}

/// Plays the scripted policy to the end; returns the whole journal.
fn scripted_game(setup: GameSetup, seed: u64) -> (GameState, Vec<Event>) {
    let (mut state, mut journal) = new_game(setup, seed).expect("the setup is valid");
    for _ in 0..DEFAULT_MAX_ACTIONS {
        if state.winner().is_some() {
            return (state, journal);
        }
        let action = scripted(&state);
        journal.extend(apply(&mut state, action).expect("a legal action applies"));
    }
    panic!("seed {seed}: the scripted game did not end");
}

#[test]
fn the_scripted_starter_game_ends_by_a_hero_going_down() {
    let [straight, swapped] = starter_setups();
    for (setup, rush_is) in [(straight, PlayerId::One), (swapped, PlayerId::Two)] {
        let rush_health = if rush_is == PlayerId::One {
            setup.one.hero.health
        } else {
            setup.two.hero.health
        };
        let bulwark_health = if rush_is == PlayerId::One {
            setup.two.hero.health
        } else {
            setup.one.hero.health
        };
        for seed in 0..10 {
            let (state, journal) = scripted_game(setup.clone(), seed);
            assert_eq!(journal, scripted_game(setup.clone(), seed).1, "seed {seed}");
            assert!(matches!(
                journal.first(),
                Some(Event::GameStarted { first, .. }) if *first == rush_is
            ));
            let Some(Event::GameWon { winner, reason }) = journal.last().cloned() else {
                panic!("seed {seed}: the journal ends with GameWon");
            };
            assert_eq!(reason, WinReason::HeroDown, "seed {seed}");
            assert_eq!(state.winner(), Some((winner, reason)));
            let loser = winner.opponent();
            assert_eq!(state.hero_health(loser), 0, "seed {seed}");
            let start = if loser == rush_is {
                rush_health
            } else {
                bulwark_health
            };
            let taken: u32 = journal
                .iter()
                .filter_map(|event| match event {
                    Event::Damaged { player, amount } if *player == loser => Some(*amount),
                    _ => None,
                })
                .sum();
            assert!(
                taken >= start,
                "seed {seed}: {taken} damage for {start} health"
            );
            let lethal = journal
                .iter()
                .rposition(|event| matches!(event, Event::Attacked { .. }))
                .expect("the lethal attack");
            assert!(
                journal[lethal..].iter().all(|event| !matches!(
                    event,
                    Event::TokenMoved { .. } | Event::ActivePlayerChanged { .. }
                )),
                "seed {seed}: no hand-over after the lethal attack"
            );
            assert!(legal_actions(&state).is_empty(), "seed {seed}");
        }
    }
}
