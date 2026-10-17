use super::random::random_policy;
use crate::Policy;
use duel_engine::Observation;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;

/// Wraps `inner` so that, `percent` times out of a hundred, the action is chosen at random
/// instead (`percent` above 100 reads as 100).
///
/// Consumption of the generator, fixed: one `random_range(0..100u8)` per decision, the coin;
/// if it is below `percent`, one more draw by `random_policy`, and `inner` is not asked;
/// otherwise `inner` chooses with the same generator. `percent == 0` takes the decisions of
/// `inner` (after one draw of the coin).
pub fn noisy<P: Policy>(inner: P, percent: u8) -> impl Policy {
    let percent = percent.min(100);
    move |obs: &Observation<'_>, rng: &mut ChaCha8Rng| {
        if rng.random_range(0..100u8) < percent {
            random_policy(obs, rng)
        } else {
            inner.choose(obs, rng)
        }
    }
}
