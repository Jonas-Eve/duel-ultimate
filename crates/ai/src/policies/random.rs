use duel_engine::Observation;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;

/// Uniformly random among the actions offered. Reads nothing but how many there are, and
/// consumes the generator with one `random_range(0..len)` (the same call as the simulator's
/// former policy, so every recorded batch is unchanged).
///
/// The list must not be empty (callers check it, see `choose`).
pub fn random_policy(obs: &Observation<'_>, rng: &mut ChaCha8Rng) -> usize {
    rng.random_range(0..obs.actions().len())
}
