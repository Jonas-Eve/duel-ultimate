//! The policies. Sources in this folder only ever see an `Observation` (guarded by
//! `tests/policies_isolation.rs`).

pub(crate) mod greedy;
pub(crate) mod noisy;
pub(crate) mod random;

use duel_engine::Observation;
use rand_chacha::ChaCha8Rng;

/// A policy as a plain function pointer (what the registry stores).
pub type PolicyFn = fn(&Observation<'_>, &mut ChaCha8Rng) -> usize;

/// Every named policy, in a fixed order.
pub const POLICIES: [(&str, PolicyFn); 2] = [
    ("random", random::random_policy),
    ("greedy", greedy::greedy_policy),
];

/// Stub, replaced by the implementation.
pub fn policy_named(_name: &str) -> Option<PolicyFn> {
    None
}
