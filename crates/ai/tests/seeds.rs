//! Golden values of the seed derivation, moved from the simulator (#140): they must never
//! change, a batch seed's whole report depends on them.

use duel_ai::{TAG_COMPUTER, TAG_POLICY_ONE, TAG_POLICY_TWO, derive_seed, policy_rng};
use duel_engine::PlayerId;
use rand::Rng;

#[test]
fn derive_seed_matches_known_splitmix64_values() {
    assert_eq!(derive_seed(0, 0), 0x0);
    assert_eq!(derive_seed(0, 1), 0xE220_A839_7B1D_CDAF);
    assert_eq!(derive_seed(1, 0), 0x5692_161D_100B_05E5);
    assert_eq!(derive_seed(42, 7), 0x53AD_348A_F3DD_AF4B);
    assert_eq!(derive_seed(0xDEAD_BEEF, u64::MAX), 0x6D90_8474_B340_945D);
}

#[test]
fn the_tags_keep_their_values_and_are_distinct() {
    assert_eq!(TAG_POLICY_ONE, 0x504F_4C49_4359_0001);
    assert_eq!(TAG_POLICY_TWO, 0x504F_4C49_4359_0002);
    assert_eq!(TAG_COMPUTER, 0x504F_4C49_4359_0003);
    assert_ne!(TAG_POLICY_ONE, TAG_POLICY_TWO);
    assert_ne!(TAG_COMPUTER, TAG_POLICY_ONE);
    assert_ne!(TAG_COMPUTER, TAG_POLICY_TWO);
}

#[test]
fn policy_rng_is_distinct_per_seat_and_from_the_engine_seed() {
    let first = |mut rng: rand_chacha::ChaCha8Rng| rng.next_u64();
    let one = first(policy_rng(9, PlayerId::One));
    let two = first(policy_rng(9, PlayerId::Two));
    assert_ne!(one, two);
    // Same derivation as the formula, not the game seed itself.
    assert_eq!(
        one,
        first(rand::SeedableRng::seed_from_u64(derive_seed(9, TAG_POLICY_ONE)))
    );
    assert_eq!(
        two,
        first(rand::SeedableRng::seed_from_u64(derive_seed(9, TAG_POLICY_TWO)))
    );
}
