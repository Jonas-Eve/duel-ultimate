use crate::Policy;
use duel_engine::{Action, GameState, Observation};
use rand_chacha::ChaCha8Rng;

/// Why [`choose`] has no action to give.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AiError {
    /// The game is over, or nothing but `Concede` is playable.
    #[error("no playable action")]
    NoPlayableAction,
    /// The policy returned an index outside the list it was given (a bug of the policy).
    #[error("the policy returned index {index} for {len} actions")]
    IndexOutOfRange { index: usize, len: usize },
}

/// The action `policy` picks for the active seat of `state`: builds the observation, asks the
/// policy, and looks the index up (never an indexing panic). Never `Concede`. This harness
/// lives outside `policies/` because it holds the state; the policy only gets the observation.
pub fn choose(
    state: &GameState,
    policy: &dyn Policy,
    rng: &mut ChaCha8Rng,
) -> Result<Action, AiError> {
    let obs = Observation::of(state).ok_or(AiError::NoPlayableAction)?;
    if obs.actions().is_empty() {
        return Err(AiError::NoPlayableAction);
    }
    let index = policy.choose(&obs, rng);
    obs.actions()
        .get(index)
        .copied()
        .ok_or(AiError::IndexOutOfRange {
            index,
            len: obs.actions().len(),
        })
}
