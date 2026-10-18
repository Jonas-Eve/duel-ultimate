//! Tags are display text (#208, criteria 2 and 9 of #137): the same setups with and without tags
//! on every card, the same seeds and the same actions give event journals identical byte for byte
//! (as `card_names.rs` does for the names).

use duel_cards::CardSpec;
use duel_engine::{Action, GameSetup, apply, game_setup_from_data, legal_actions, new_game};

const CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
const DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");

fn tagged(mut setup: GameSetup) -> GameSetup {
    for side in [&mut setup.one, &mut setup.two] {
        for (n, card) in side.cards.iter_mut().enumerate() {
            let tags = vec![format!("tag-{n}"), "bâtiment".to_owned()];
            match card {
                CardSpec::Unit(unit) => unit.tags = tags,
                CardSpec::Spell(spell) => spell.tags = tags,
            }
        }
    }
    setup
}

/// The journal of a whole game: the setup events then those of every action, as JSON text. The
/// action is picked by a small generator of the test (never by the engine's).
fn journal(setup: GameSetup, seed: u64) -> String {
    let (mut state, mut events) = new_game(setup, seed).unwrap();
    let mut pick = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    for _ in 0..2_000 {
        let legal = legal_actions(&state);
        let playable: Vec<&Action> = legal.iter().filter(|a| **a != Action::Concede).collect();
        if state.winner().is_some() || playable.is_empty() {
            break;
        }
        pick = pick
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let action = playable[usize::try_from(pick >> 33).unwrap() % playable.len()].clone();
        events.extend(apply(&mut state, action).unwrap());
    }
    serde_json::to_string(&events).unwrap()
}

#[test]
fn a_game_with_tags_is_the_same_game_as_without() {
    let setup = game_setup_from_data(CARDS, DECKS, "rush", "bulwark", vec![]).unwrap();
    let with_tags = tagged(setup.clone());
    assert_ne!(setup, with_tags);
    for seed in 0..40 {
        let plain = journal(setup.clone(), seed);
        assert!(
            plain.contains("game_won"),
            "seed {seed}: the game is played to its end"
        );
        assert_eq!(plain, journal(with_tags.clone(), seed), "seed {seed}");
    }
}
