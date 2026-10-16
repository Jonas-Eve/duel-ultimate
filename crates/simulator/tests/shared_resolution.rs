//! The simulator resolves decks through the engine's single implementation (`player_setup`),
//! never through a copy of its own (S2-T1 of #52).

use duel_cards::{parse_cards, parse_decks};
use duel_engine::player_setup;
use duel_simulator::{contenders_from, setup_from};

const SETS: [(&str, &str, &str); 2] = [
    (
        "fixture",
        include_str!("../../../data/fixture/1.0.0/cards.json"),
        include_str!("../../../data/fixture/1.0.0/decks.json"),
    ),
    (
        "starter",
        include_str!("../../../data/starter/1.0.0/cards.json"),
        include_str!("../../../data/starter/1.0.0/decks.json"),
    ),
];

#[test]
fn contenders_are_the_engine_player_setups_names_included() {
    for (set, cards, decks) in SETS {
        let contenders = contenders_from(cards, decks).unwrap();
        let parsed_cards = parse_cards(cards).unwrap();
        let parsed_decks = parse_decks(decks).unwrap();
        assert_eq!(contenders.len(), parsed_decks.len(), "{set}");
        for (contender, deck) in contenders.iter().zip(&parsed_decks) {
            assert_eq!(contender.id, deck.id);
            let expected = player_setup(deck, &parsed_cards).unwrap();
            assert_eq!(contender.player, expected, "{set}/{}", deck.id);
            assert_eq!(contender.player.card_names.len(), 30, "{set}/{}", deck.id);
        }
    }
}

#[test]
fn setup_from_is_the_engine_player_setups_of_the_first_two_decks() {
    for (set, cards, decks) in SETS {
        let setup = setup_from(cards, decks, vec![format!("{set}@1.0.0")]).unwrap();
        let parsed_cards = parse_cards(cards).unwrap();
        let parsed_decks = parse_decks(decks).unwrap();
        assert_eq!(
            setup.one,
            player_setup(&parsed_decks[0], &parsed_cards).unwrap()
        );
        assert_eq!(
            setup.two,
            player_setup(&parsed_decks[1], &parsed_cards).unwrap()
        );
        assert_eq!(setup.data_versions, [format!("{set}@1.0.0")]);
        assert!(!setup.one.card_names.is_empty() && !setup.two.card_names.is_empty());
    }
}

/// Policy: the simulator keeps no deck resolution of its own. The call it replaced is named
/// here by what it is spelled like, so that a copy cannot come back unnoticed.
#[test]
fn no_source_of_the_simulator_calls_resolve_deck() {
    let sources = [
        ("batch.rs", include_str!("../src/batch.rs")),
        ("lib.rs", include_str!("../src/lib.rs")),
        ("main.rs", include_str!("../src/main.rs")),
        ("report.rs", include_str!("../src/report.rs")),
        ("stats.rs", include_str!("../src/stats.rs")),
    ];
    let call = ["resolve_deck", "("].concat();
    for (name, source) in sources {
        assert!(!source.contains(&call), "{name} resolves decks itself");
    }
}
