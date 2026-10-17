use duel_engine::PlayerId;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Tag of the generator handed to the policy of seat One.
pub const TAG_POLICY_ONE: u64 = 0x504F_4C49_4359_0001;
/// Tag of the generator handed to the policy of seat Two.
pub const TAG_POLICY_TWO: u64 = 0x504F_4C49_4359_0002;
/// Tag of the generator handed to the computer opponent of the client.
pub const TAG_COMPUTER: u64 = 0x504F_4C49_4359_0003;

/// SplitMix64's finalizer applied to `parent ^ tag * golden-gamma`: a child seed that depends
/// on both and looks unrelated to either.
pub fn derive_seed(parent: u64, tag: u64) -> u64 {
    let mut z = parent ^ tag.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The generator of the policy of `seat` in a game seeded `game_seed`: distinct from the
/// engine's and from the other seat's, so a seat's choices never depend on how many draws the
/// other one made.
pub fn policy_rng(game_seed: u64, seat: PlayerId) -> ChaCha8Rng {
    let tag = match seat {
        PlayerId::One => TAG_POLICY_ONE,
        PlayerId::Two => TAG_POLICY_TWO,
    };
    ChaCha8Rng::seed_from_u64(derive_seed(game_seed, tag))
}
