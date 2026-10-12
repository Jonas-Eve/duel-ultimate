//! The game state: private to the crate, observed through minimal accessors.

use duel_cards::{CreatureStats, Hero};
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerId {
    One,
    Two,
}

impl PlayerId {
    pub fn opponent(self) -> Self {
        match self {
            Self::One => Self::Two,
            Self::Two => Self::One,
        }
    }
}

/// Identifies one card of one game. Assigned in a deterministic order at setup (One's deck
/// then Two's, before shuffling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CardInstanceId(pub u32);

/// Why a game ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WinReason {
    HeroDown,
    Exhaustion,
    Concede,
}

/// A card of a game, carrying a copy of its stats: the engine never queries a data store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CardInstance {
    pub(crate) id: CardInstanceId,
    pub(crate) stats: CreatureStats,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PlayerState {
    pub(crate) hero: Hero,
    /// Current hero health.
    pub(crate) health: u32,
    /// Draw pile; the top is the end of the `Vec`.
    pub(crate) deck: Vec<CardInstance>,
    pub(crate) hand: Vec<CardInstance>,
}

/// Invariant while the game runs: `1 <= budget <= max_time` of the active player's hero.
#[derive(Debug, Clone, PartialEq)]
pub struct GameState {
    pub(crate) one: PlayerState,
    pub(crate) two: PlayerState,
    pub(crate) active: PlayerId,
    pub(crate) budget: u32,
    pub(crate) winner: Option<(PlayerId, WinReason)>,
    pub(crate) data_versions: Vec<String>,
    rng: ChaCha8Rng,
}

impl GameState {
    pub(crate) fn from_parts(
        _one: PlayerState,
        _two: PlayerState,
        _data_versions: Vec<String>,
        _seed: u64,
    ) -> Self {
        todo!()
    }

    pub fn active(&self) -> PlayerId {
        self.active
    }

    /// Time the active player still has.
    pub fn budget(&self) -> u32 {
        self.budget
    }

    pub fn winner(&self) -> Option<(PlayerId, WinReason)> {
        self.winner
    }

    /// The data set versions this game was started with (opaque to the engine).
    pub fn data_versions(&self) -> &[String] {
        &self.data_versions
    }

    pub fn hand_len(&self, player: PlayerId) -> usize {
        self.player(player).hand.len()
    }

    pub fn deck_len(&self, player: PlayerId) -> usize {
        self.player(player).deck.len()
    }

    pub(crate) fn player(&self, player: PlayerId) -> &PlayerState {
        match player {
            PlayerId::One => &self.one,
            PlayerId::Two => &self.two,
        }
    }

    pub(crate) fn player_mut(&mut self, player: PlayerId) -> &mut PlayerState {
        match player {
            PlayerId::One => &mut self.one,
            PlayerId::Two => &mut self.two,
        }
    }

    /// Draws from the game's own seeded generator, the only source of randomness rules may
    /// use. `bound` must be at least 1.
    pub(crate) fn random_below(&mut self, bound: u32) -> u32 {
        debug_assert!(bound >= 1, "random_below needs a bound of at least 1");
        self.rng.random_range(0..bound.max(1))
    }
}

/// Tests living next to `GameState` so they can set up states the public API cannot reach
/// (empty piles on a running game, exact generator comparison).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{draw, hero, setup};
    use crate::{Action, Event, RuleError, apply, legal_actions, new_game};
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    /// One starts (initiative 2) with budget 8; Two has max_time 5.
    fn running(one_effects: Vec<duel_cards::HeroEffect>) -> GameState {
        let (state, _) = new_game(
            setup(hero(2, 8, 8, one_effects), hero(1, 2, 5, vec![draw(1, 1)])),
            3,
        )
        .unwrap();
        assert_eq!(state.active(), PlayerId::One);
        state
    }

    #[test]
    fn exhaustion_after_an_action_that_keeps_the_player_active() {
        let mut state = running(vec![draw(1, 1)]);
        state.one.deck.truncate(1);
        let events = apply(&mut state, Action::Activate { effect: 0 }).unwrap();
        assert_eq!(state.winner(), Some((PlayerId::Two, WinReason::Exhaustion)));
        assert_eq!(state.active(), PlayerId::One, "no handover happened");
        assert_eq!(
            events.last(),
            Some(&Event::GameWon {
                winner: PlayerId::Two,
                reason: WinReason::Exhaustion
            })
        );
        assert!(legal_actions(&state).is_empty());
    }

    #[test]
    fn exhaustion_of_a_receiver_with_an_empty_pile() {
        let mut state = running(vec![draw(8, 1)]);
        state.two.deck.clear();
        let events = apply(&mut state, Action::Activate { effect: 0 }).unwrap();
        assert_eq!(state.winner(), Some((PlayerId::One, WinReason::Exhaustion)));
        assert_eq!(state.active(), PlayerId::Two);
        let tail: Vec<_> = events.iter().rev().take(3).rev().cloned().collect();
        assert_eq!(
            tail,
            vec![
                Event::TokenMoved {
                    player: PlayerId::Two,
                    budget: 1
                },
                Event::ActivePlayerChanged {
                    player: PlayerId::Two
                },
                Event::GameWon {
                    winner: PlayerId::One,
                    reason: WinReason::Exhaustion
                },
            ]
        );
    }

    #[test]
    fn exhaustion_is_checked_right_after_setup() {
        let mut one = hero(2, 3, 8, vec![draw(1, 1)]);
        one.starting_hand = 30;
        let (state, events) = new_game(setup(one, hero(1, 2, 5, vec![draw(1, 1)])), 1).unwrap();
        assert_eq!(state.winner(), Some((PlayerId::Two, WinReason::Exhaustion)));
        assert!(matches!(events.first(), Some(Event::GameStarted { .. })));
        assert!(matches!(events.last(), Some(Event::GameWon { .. })));
        assert!(legal_actions(&state).is_empty());
        // A hero without any effect has nothing to do either.
        let (state, _) = new_game(
            setup(hero(2, 3, 8, vec![]), hero(1, 2, 5, vec![draw(1, 1)])),
            1,
        )
        .unwrap();
        assert_eq!(state.winner(), Some((PlayerId::Two, WinReason::Exhaustion)));
    }

    #[test]
    fn drawing_from_an_empty_pile_is_refused_and_changes_nothing() {
        let mut state = running(vec![draw(1, 1)]);
        state.one.deck.clear();
        let before = state.clone();
        assert_eq!(
            apply(&mut state, Action::Activate { effect: 0 }),
            Err(RuleError::EffectNotApplicable)
        );
        assert_eq!(state, before);
        assert_eq!(legal_actions(&state), vec![Action::Concede]);
    }

    #[test]
    fn apply_never_consumes_the_generator() {
        let mut state = running(vec![draw(1, 1), draw(9, 3)]);
        let rng_before = state.rng.clone();
        apply(&mut state, Action::Activate { effect: 1 }).unwrap();
        assert_eq!(state.rng, rng_before);
        let _ = apply(&mut state, Action::Activate { effect: 77 });
        assert_eq!(state.rng, rng_before);
    }

    #[test]
    fn budget_stays_within_one_and_the_active_max_time_in_random_games() {
        for seed in 0..200u64 {
            let (mut state, _) = new_game(
                setup(
                    hero(
                        1 + (seed % 2) as u32,
                        3,
                        8,
                        vec![draw(1, 1), draw(4, 2), draw(20, 1)],
                    ),
                    hero(1, 2, 5, vec![draw(2, 1), draw(9, 3)]),
                ),
                seed,
            )
            .unwrap();
            let mut policy = ChaCha8Rng::seed_from_u64(seed ^ 0xA5A5);
            let mut steps = 0;
            while state.winner().is_none() {
                let max_time = state.player(state.active()).hero.max_time;
                assert!(
                    (1..=max_time).contains(&state.budget()),
                    "seed {seed}: budget {} outside 1..={max_time}",
                    state.budget()
                );
                let legal = legal_actions(&state);
                let playable: Vec<_> = legal
                    .iter()
                    .copied()
                    .filter(|a| *a != Action::Concede)
                    .collect();
                let pick = playable
                    .get(policy.next_u32() as usize % playable.len().max(1))
                    .copied()
                    .unwrap_or(Action::Concede);
                apply(&mut state, pick).unwrap();
                steps += 1;
                assert!(steps < 1000, "seed {seed}: the game does not end");
            }
        }
    }
}
