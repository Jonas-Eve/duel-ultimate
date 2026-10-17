use duel_engine::Observation;
use rand_chacha::ChaCha8Rng;

/// How a player picks an action: it sees only its [`Observation`] (the legal actions other
/// than `Concede`, its own view) and its own generator, and returns an index into
/// `obs.actions()`. The signature is the guarantee: a policy cannot read the game state, the
/// other seat or the engine's generator. The generator is the concrete [`ChaCha8Rng`].
pub trait Policy {
    fn choose(&self, obs: &Observation<'_>, rng: &mut ChaCha8Rng) -> usize;
}

impl<F> Policy for F
where
    F: Fn(&Observation<'_>, &mut ChaCha8Rng) -> usize,
{
    fn choose(&self, obs: &Observation<'_>, rng: &mut ChaCha8Rng) -> usize {
        self(obs, rng)
    }
}
