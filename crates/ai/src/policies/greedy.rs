//! The greedy policy: one move ahead, no search, no chance.
//!
//! For each legal action the policy asks the observation for its outcome (what the seat may know
//! after it: its own view with a drawn card only counted, the opponent's public side, the
//! preview) and takes the action whose outcome scores best (`eval.rs`). Priorities are not coded
//! rules but consequences of that score:
//!
//! 1. an action that ends the game is worth plus or minus a very large constant, so a lethal hit
//!    comes first and a move the preview shows losing is avoided while another exists;
//! 2. otherwise the sum of hero health, board value (own minus opposing, a dead creature no
//!    longer counts, a killed provocateur is a gain), hand value and time decides;
//! 3. a tie goes to the cheaper action, then to the first in the engine's order.
//!
//! It never consumes its generator and reads nothing hidden: the observation cannot show it.

use super::eval::{DEFAULT_WEIGHTS, choose_best};
use duel_engine::Observation;
use rand_chacha::ChaCha8Rng;

/// The greedy policy, same signature as `random_policy`. The generator is left untouched.
pub fn greedy_policy(obs: &Observation<'_>, _rng: &mut ChaCha8Rng) -> usize {
    choose_best(obs, &DEFAULT_WEIGHTS)
}
