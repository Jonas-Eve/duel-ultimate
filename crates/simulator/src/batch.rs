//! Batches of seeded AI-vs-AI games: matchups, derived seeds, alternating seats, action cap.

use crate::report::Report;
use duel_engine::{Action, Event, GameSetup, PlayerId, PlayerSetup, WinReason};
use rand_chacha::ChaCha8Rng;

pub type Policy = fn(&[Action], &mut ChaCha8Rng) -> usize;

pub const DEFAULT_MAX_ACTIONS: u32 = 10_000;
pub const TAG_POLICY_ONE: u64 = 0x504F_4C49_4359_0001;
pub const TAG_POLICY_TWO: u64 = 0x504F_4C49_4359_0002;

pub fn random_policy(_actions: &[Action], _rng: &mut ChaCha8Rng) -> usize {
    unimplemented!()
}

#[derive(Debug, Clone)]
pub struct Contender {
    pub id: String,
    pub player: PlayerSetup,
}

pub fn contenders_from(_cards: &str, _decks: &str) -> Result<Vec<Contender>, String> {
    unimplemented!()
}

#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Games per matchup.
    pub games: u64,
    pub seed: u64,
    pub max_actions: u32,
    pub data_versions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameRecord {
    pub matchup: usize,
    pub index: u64,
    pub seat_of_a: PlayerId,
    pub first: PlayerId,
    pub winner: Option<(PlayerId, WinReason)>,
    pub actions: u32,
    pub deaths: usize,
    pub interrupted: bool,
}

pub fn derive_seed(_parent: u64, _tag: u64) -> u64 {
    unimplemented!()
}

pub fn game_seed(_batch_seed: u64, _matchup: usize, _index: u64) -> u64 {
    unimplemented!()
}

pub fn policy_rng(_game_seed: u64, _seat: PlayerId) -> ChaCha8Rng {
    unimplemented!()
}

pub fn play_game(
    _setup: GameSetup,
    _seed: u64,
    _policy: Policy,
    _max_actions: u32,
    _on_step: impl FnMut(&[Action], &[Event]),
) -> GameRecord {
    unimplemented!()
}

pub fn run_batch(contenders: &[Contender], config: &BatchConfig, policy: Policy) -> Report {
    run_batch_observed(contenders, config, policy, |_| {})
}

pub fn run_batch_observed(
    _contenders: &[Contender],
    _config: &BatchConfig,
    _policy: Policy,
    _on_game: impl FnMut(&GameRecord),
) -> Report {
    unimplemented!()
}
