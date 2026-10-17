use crate::Policy;
use duel_engine::Observation;
use rand_chacha::ChaCha8Rng;

/// Stub, replaced by the implementation.
pub fn noisy<P: Policy>(inner: P, _percent: u8) -> impl Policy {
    move |obs: &Observation<'_>, rng: &mut ChaCha8Rng| inner.choose(obs, rng)
}
