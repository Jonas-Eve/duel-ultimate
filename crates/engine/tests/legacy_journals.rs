//! The reference journals of `starter@1.1.0` with every rule option off, pinned **before** the
//! options of story #127 acted on anything. They never change: they are the "before #127"
//! behaviour that the disabled options must keep reproducing byte for byte.

use duel_engine::{
    Action, Event, GameState, PlayerId, RuleSet, WinReason, apply, game_setup_from_data,
    legal_actions, new_game,
};

const CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
const DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");

/// FNV-1a (64 bits) of a byte string: a stable, dependency-free fingerprint.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Plays one game, picking at each step a pseudo-random action among `legal_actions` other
/// than `Concede`, from a generator owned by the test (a 64-bit LCG), so that the policy does
/// not depend on the engine's own seed (same player as `play_scripted_s2a_from` in `engine.rs`).
fn play_scripted(setup: duel_engine::GameSetup, seed: u64) -> (GameState, Vec<Event>) {
    let (mut state, mut log) = new_game(setup, seed).unwrap();
    let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0xdead_beef;
    while state.winner().is_none() {
        let actions: Vec<_> = legal_actions(&state)
            .into_iter()
            .filter(|a| *a != Action::Concede)
            .collect();
        if actions.is_empty() {
            log.extend(apply(&mut state, Action::Concede).unwrap());
            continue;
        }
        rng = rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let pick = usize::try_from((rng >> 33) % actions.len() as u64).unwrap();
        log.extend(apply(&mut state, actions[pick]).unwrap());
    }
    (state, log)
}

/// `(winner, reason, number of events, FNV-1a of the events as JSON)` per seed 0..20 for
/// `rush` (One) against `bulwark` (Two) of `starter@1.1.0`, `RuleSet::legacy()` written out.
const STARTER_1_1_0_LEGACY_GOLDEN: [(PlayerId, WinReason, usize, u64); 20] = [
    (PlayerId::One, WinReason::HeroDown, 160, 0x523cf73d1979ce34),
    (PlayerId::One, WinReason::HeroDown, 131, 0x9ca84211c46ca2d1),
    (PlayerId::One, WinReason::HeroDown, 219, 0x13a4c81a211315d8),
    (PlayerId::One, WinReason::HeroDown, 133, 0x9134178744af6ab0),
    (PlayerId::One, WinReason::HeroDown, 104, 0x4d6b2f1667df111f),
    (PlayerId::One, WinReason::HeroDown, 102, 0xe0003b74bdf669c1),
    (PlayerId::One, WinReason::HeroDown, 279, 0x8cfc74c64b24eaf8),
    (PlayerId::One, WinReason::HeroDown, 119, 0x6046f3b1b315dbfe),
    (PlayerId::One, WinReason::HeroDown, 300, 0x0756a4efbca22df9),
    (PlayerId::One, WinReason::HeroDown, 312, 0x56b73c2469e5e6e4),
    (PlayerId::One, WinReason::HeroDown, 123, 0x022688f920949716),
    (PlayerId::One, WinReason::HeroDown, 102, 0x7a43d96cd82e270d),
    (PlayerId::One, WinReason::HeroDown, 85, 0x842a2fecb16a61ce),
    (PlayerId::One, WinReason::HeroDown, 138, 0x5dad2397d397588f),
    (PlayerId::One, WinReason::HeroDown, 116, 0xacdd8fe154d0dea5),
    (PlayerId::One, WinReason::HeroDown, 151, 0xdf8f31e8e20f9f4c),
    (PlayerId::Two, WinReason::HeroDown, 126, 0x8f9896c126ae0cae),
    (PlayerId::One, WinReason::HeroDown, 207, 0xd607460ee62a2a5e),
    (PlayerId::One, WinReason::HeroDown, 287, 0xcba66cc032f38ecf),
    (PlayerId::One, WinReason::HeroDown, 218, 0xf560c3deb82edf93),
];

#[test]
fn starter_1_1_0_legacy_journals_are_unchanged() {
    let observed: Vec<_> = (0..20)
        .map(|seed| {
            let mut setup = game_setup_from_data(
                CARDS,
                DECKS,
                "rush",
                "bulwark",
                vec!["starter@1.1.0".to_string()],
            )
            .unwrap();
            setup.rules = RuleSet::legacy();
            let (state, log) = play_scripted(setup, seed);
            let (winner, reason) = state.winner().unwrap();
            let json = serde_json::to_string(&log).unwrap();
            (winner, reason, log.len(), fnv1a(json.as_bytes()))
        })
        .collect();
    assert_eq!(observed, STARTER_1_1_0_LEGACY_GOLDEN);
}

/// The same twenty games under `RuleSet::standard()` written out, recorded on the engine of
/// #172 (the switch of the default, #127).
const STARTER_1_1_0_STANDARD_GOLDEN: [(PlayerId, WinReason, usize, u64); 20] =
    [(PlayerId::One, WinReason::HeroDown, 0, 0); 20];

#[test]
fn starter_1_1_0_standard_journals_have_their_golden() {
    let observed: Vec<_> = (0..20)
        .map(|seed| {
            let mut setup = game_setup_from_data(
                CARDS,
                DECKS,
                "rush",
                "bulwark",
                vec!["starter@1.1.0".to_string()],
            )
            .unwrap();
            setup.rules = RuleSet::standard();
            let (state, log) = play_scripted(setup, seed);
            let (winner, reason) = state.winner().unwrap();
            let json = serde_json::to_string(&log).unwrap();
            (winner, reason, log.len(), fnv1a(json.as_bytes()))
        })
        .collect();
    assert_eq!(observed, STARTER_1_1_0_STANDARD_GOLDEN);
}
