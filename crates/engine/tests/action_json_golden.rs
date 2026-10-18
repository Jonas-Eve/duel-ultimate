//! The JSON of `legal_actions`, pinned **before** story #128 changed the `Action` contract.
//!
//! The event goldens (`S2A_GOLDEN`, `STARTER_1_1_0_*_GOLDEN`) fingerprint the journals only, never
//! the lists of legal actions a client reads. This test fingerprints those lists: at every step of
//! twenty `starter@1.1.0` games (`rush` against `bulwark`, `RuleSet::standard()` written out), the
//! list is serialized with `serde_json::to_string` and the stream of those strings is hashed.
//! `LEGAL_ACTIONS_JSON_GOLDEN` was captured on the code that predates #128 and never changes: the
//! JSON of an existing action stays identical byte for byte (picks and source are omitted when
//! empty).

use duel_engine::{Action, RuleSet, apply, game_setup_from_data, legal_actions, new_game};

const CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
const DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");

/// FNV-1a (64 bits) of a byte string: a stable, dependency-free fingerprint.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// `(steps, bytes, FNV-1a of the JSON of every legal-action list, in order)` per seed 0..20.
const LEGAL_ACTIONS_JSON_GOLDEN: [(usize, usize, u64); 20] = [
    (45, 20713, 0xa458f0dc53ffe3be),
    (57, 16938, 0xb2f5722b7d4c6ce5),
    (67, 34728, 0x6ac98065c50219c0),
    (60, 30375, 0x37a34042ebbe44c6),
    (71, 42647, 0x26a4dbaf537f5891),
    (53, 24946, 0xd5477097d66c679f),
    (54, 24157, 0xe1fc59e3a8f0570f),
    (46, 17380, 0xd887a6524bede0d2),
    (49, 20352, 0x833a51842d816b0e),
    (72, 35967, 0xc466945d6848971f),
    (37, 12208, 0x45ab0b6aa3c66b0f),
    (59, 19781, 0x7f625b6b19adc990),
    (53, 17172, 0xdff7e9a5771f14ec),
    (49, 21128, 0x525e1e748a03af4e),
    (46, 14674, 0xebdb21be73376240),
    (47, 22030, 0x66130ac86f39c99f),
    (47, 16802, 0x4c020d195c4d26dd),
    (54, 26971, 0xe6b4c958211a7217),
    (48, 23391, 0x5c18bfe31c5c5c82),
    (48, 28654, 0xb43db144eaca9a8b),
];

/// Same player as `play_scripted` of `legacy_journals.rs` (a 64-bit LCG owned by the test), but it
/// fingerprints the JSON of the whole legal-action list at each step instead of the events.
fn fingerprint(seed: u64) -> (usize, usize, u64) {
    let mut setup = game_setup_from_data(
        CARDS,
        DECKS,
        "rush",
        "bulwark",
        vec!["starter@1.1.0".to_string()],
    )
    .unwrap();
    setup.rules = RuleSet::standard();
    let (mut state, _) = new_game(setup, seed).unwrap();
    let mut rng = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0xdead_beef;
    let mut stream = Vec::new();
    let mut steps = 0;
    while state.winner().is_none() {
        let all = legal_actions(&state);
        stream.extend_from_slice(serde_json::to_string(&all).unwrap().as_bytes());
        steps += 1;
        let actions: Vec<Action> = all.into_iter().filter(|a| *a != Action::Concede).collect();
        if actions.is_empty() {
            apply(&mut state, Action::Concede).unwrap();
            continue;
        }
        rng = rng
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let pick = usize::try_from((rng >> 33) % actions.len() as u64).unwrap();
        let chosen = actions.into_iter().nth(pick).unwrap();
        apply(&mut state, chosen).unwrap();
    }
    (steps, stream.len(), fnv1a(&stream))
}

#[test]
fn legal_actions_json_is_byte_identical_to_before_128() {
    let observed: Vec<_> = (0..20).map(fingerprint).collect();
    assert_eq!(observed, LEGAL_ACTIONS_JSON_GOLDEN);
}
