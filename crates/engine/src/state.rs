//! The game state: private to the crate, observed through minimal accessors.

use duel_cards::{CreatureStats, Hero};
use rand::{RngExt, SeedableRng};
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
/// `stats.health` is the instance's *current* health (damage lowers it; a creature at 0 is
/// buried): there is no second field, which would be a second source of truth.
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
    /// Summoned creatures, in order of summoning.
    pub(crate) board: Vec<CardInstance>,
    /// Dead creatures, in order of death. Public information.
    pub(crate) discard: Vec<CardInstance>,
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
        one: PlayerState,
        two: PlayerState,
        data_versions: Vec<String>,
        seed: u64,
    ) -> Self {
        Self {
            one,
            two,
            active: PlayerId::One,
            budget: 1,
            winner: None,
            data_versions,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
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

    /// Current health of `player`'s hero.
    pub fn hero_health(&self, player: PlayerId) -> u32 {
        self.player(player).health
    }

    /// The cards in `player`'s hand. A client sees only its own hand: filtering is the server's job.
    pub fn hand(&self, player: PlayerId) -> Vec<CardInstanceId> {
        self.player(player).hand.iter().map(|c| c.id).collect()
    }

    /// The creatures `player` has summoned, in order of summoning.
    pub fn board(&self, player: PlayerId) -> Vec<CardInstanceId> {
        self.player(player).board.iter().map(|c| c.id).collect()
    }

    /// The creatures that died on `player`'s side, in order of death (the discard is public).
    pub fn discard(&self, player: PlayerId) -> Vec<CardInstanceId> {
        self.player(player).discard.iter().map(|c| c.id).collect()
    }

    /// Current health of the creature `creature` if it is on a board.
    pub fn creature_health(&self, creature: CardInstanceId) -> Option<u32> {
        [&self.one, &self.two]
            .into_iter()
            .flat_map(|side| side.board.iter())
            .find(|c| c.id == creature)
            .map(|c| c.stats.health)
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
    use crate::testing::{draw, hero, potential, setup, stats, stats_with};
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
    fn a_creature_in_play_keeps_its_owner_alive_after_the_last_card_is_played() {
        let mut state = running(vec![draw(1, 1)]);
        // One is left with a single card in hand and an empty pile. Once played, the creature
        // can attack, so One is not exhausted: attacks count as legal actions.
        state.one.deck.clear();
        state.one.hand.truncate(1);
        let card = state.one.hand[0].id;
        apply(&mut state, Action::Play { card }).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!(state.active(), PlayerId::One);
        assert!(legal_actions(&state).contains(&Action::Attack {
            attacker: card,
            target: crate::Target::Hero
        }));
    }

    #[test]
    fn a_receiver_with_only_a_creature_in_play_is_not_exhausted() {
        let mut state = running(vec![draw(8, 1)]);
        state.two.deck.clear();
        let creature = state.two.hand.remove(0);
        state.two.hand.clear();
        state.two.board.push(creature);
        apply(&mut state, Action::Activate { effect: 0 }).unwrap();
        assert_eq!(state.active(), PlayerId::Two);
        assert_eq!(state.winner(), None);
    }

    #[test]
    fn exhaustion_of_a_receiver_with_an_empty_hand_and_pile() {
        let mut state = running(vec![draw(8, 1)]);
        state.two.deck.clear();
        state.two.hand.clear();
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

    /// `running` (One active, budget 8) with a creature of `attack_cost` already on One's
    /// board, and Two stripped of hand, pile and board. Returns its id.
    fn armed_against_an_empty_two(attack_cost: u32) -> (GameState, crate::CardInstanceId) {
        let mut state = running(vec![draw(1, 1)]);
        let mut creature = state.one.hand.remove(0);
        creature.stats.force = 1;
        creature.stats.attack_cost = attack_cost;
        let id = creature.id;
        state.one.board.push(creature);
        state.two.hand.clear();
        state.two.deck.clear();
        state.two.board.clear();
        (state, id)
    }

    fn hit(attacker: crate::CardInstanceId) -> Action {
        Action::Attack {
            attacker,
            target: crate::Target::Hero,
        }
    }

    #[test]
    fn a_non_lethal_attack_that_hands_over_to_an_exhausted_receiver_ends_the_game() {
        // c >= b (8 >= 8): the token crosses to Two, who has nothing left to do.
        let (mut state, id) = armed_against_an_empty_two(8);
        let events = apply(&mut state, hit(id)).unwrap();
        assert_eq!(
            state.hero_health(PlayerId::Two),
            9,
            "the attack is not lethal"
        );
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
    fn a_non_lethal_attack_that_keeps_the_token_does_not_exhaust_anyone() {
        // c < b (2 < 8): One stays active with its creature on the board.
        let (mut state, id) = armed_against_an_empty_two(2);
        let events = apply(&mut state, hit(id)).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!((state.active(), state.budget()), (PlayerId::One, 6));
        assert!(!events.iter().any(|e| matches!(e, Event::GameWon { .. })));
    }

    /// `running` (One active, 8 time units) with a creature of `attack_cost` and force 5 on
    /// One's board facing a lone 1-health creature on Two's, Two having neither hand nor
    /// pile. Returns the state, the attacker and the victim.
    fn about_to_kill_twos_last_creature(
        attack_cost: u32,
    ) -> (GameState, crate::CardInstanceId, crate::CardInstanceId) {
        let mut state = running(vec![draw(1, 1)]);
        let mut attacker = state.one.hand.remove(0);
        attacker.stats.force = 5;
        attacker.stats.attack_cost = attack_cost;
        let mut victim = state.two.hand.remove(0);
        victim.stats.health = 1;
        let (a, x) = (attacker.id, victim.id);
        state.one.board.push(attacker);
        state.two.board.push(victim);
        state.two.hand.clear();
        state.two.deck.clear();
        (state, a, x)
    }

    fn strike(a: crate::CardInstanceId, x: crate::CardInstanceId) -> Action {
        Action::Attack {
            attacker: a,
            target: crate::Target::Creature(x),
        }
    }

    #[test]
    fn killing_the_last_creature_of_a_receiver_without_cards_ends_the_game_by_exhaustion() {
        // c >= b (8 >= 8): the token crosses to Two, who has nothing left to do.
        let (mut state, a, x) = about_to_kill_twos_last_creature(8);
        let events = apply(&mut state, strike(a, x)).unwrap();
        assert_eq!(state.winner(), Some((PlayerId::One, WinReason::Exhaustion)));
        assert_eq!(state.active(), PlayerId::Two);
        assert_eq!(state.hero_health(PlayerId::Two), 10, "no hero damage");
        let tail: Vec<_> = events.iter().rev().take(4).rev().cloned().collect();
        assert_eq!(
            tail,
            vec![
                Event::Died {
                    player: PlayerId::Two,
                    creature: x
                },
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
        assert!(legal_actions(&state).is_empty());
    }

    #[test]
    fn a_creature_kill_that_keeps_the_token_exhausts_nobody() {
        // c < b (2 < 8): One stays active, and its creature can still attack.
        let (mut state, a, x) = about_to_kill_twos_last_creature(2);
        let events = apply(&mut state, strike(a, x)).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!((state.active(), state.budget()), (PlayerId::One, 6));
        assert!(!events.iter().any(|e| matches!(e, Event::GameWon { .. })));
    }

    #[test]
    fn a_receiver_with_another_creature_is_not_exhausted_by_a_kill() {
        let (mut state, a, x) = about_to_kill_twos_last_creature(8);
        let mut other = state.two.board[0].clone();
        other.id = crate::CardInstanceId(999);
        other.stats.health = 5;
        state.two.board.push(other);
        apply(&mut state, strike(a, x)).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!(state.active(), PlayerId::Two);
    }

    #[test]
    fn a_receiver_with_a_card_in_hand_is_not_exhausted_by_a_kill() {
        let (mut state, a, x) = about_to_kill_twos_last_creature(8);
        let card = state.one.hand.remove(0);
        state.two.hand.push(card);
        apply(&mut state, strike(a, x)).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!(state.active(), PlayerId::Two);
    }

    #[test]
    fn a_receiver_with_a_card_in_the_pile_is_not_exhausted_by_a_kill() {
        // Its hero can still draw.
        let (mut state, a, x) = about_to_kill_twos_last_creature(8);
        let card = state.one.hand.remove(0);
        state.two.deck.push(card);
        apply(&mut state, strike(a, x)).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!(state.active(), PlayerId::Two);
    }

    fn creature(id: u32, health: u32) -> CardInstance {
        CardInstance {
            id: CardInstanceId(id),
            stats: stats(1, 1, 1, health),
        }
    }

    #[test]
    fn bury_dead_discards_the_dead_of_both_boards_in_instance_id_order() {
        let mut state = running(vec![draw(1, 1)]);
        // Board orders and sides deliberately disagree with the id order.
        state.one.board = vec![creature(9, 0), creature(40, 2), creature(5, 0)];
        state.two.board = vec![creature(50, 3), creature(1, 0)];
        let mut events = Vec::new();
        crate::combat::bury_dead(&mut state, &mut events);
        assert_eq!(
            events,
            vec![
                Event::Died {
                    player: PlayerId::Two,
                    creature: CardInstanceId(1)
                },
                Event::Died {
                    player: PlayerId::One,
                    creature: CardInstanceId(5)
                },
                Event::Died {
                    player: PlayerId::One,
                    creature: CardInstanceId(9)
                },
            ]
        );
        assert_eq!(
            state.discard(PlayerId::One),
            vec![CardInstanceId(5), CardInstanceId(9)]
        );
        assert_eq!(state.discard(PlayerId::Two), vec![CardInstanceId(1)]);
        assert_eq!(state.board(PlayerId::One), vec![CardInstanceId(40)]);
        assert_eq!(state.board(PlayerId::Two), vec![CardInstanceId(50)]);
        // Nothing left to bury: a second sweep changes nothing.
        let before = state.clone();
        events.clear();
        crate::combat::bury_dead(&mut state, &mut events);
        assert!(events.is_empty());
        assert_eq!(state, before);
    }

    #[test]
    fn a_non_empty_hand_is_never_exhausted() {
        // Empty pile and no effect at all, but cards in hand: playing is still possible.
        let (state, _) = new_game(
            setup(hero(2, 3, 8, vec![]), hero(1, 2, 5, vec![draw(1, 1)])),
            1,
        )
        .unwrap();
        assert_eq!(state.winner(), None);
        let mut one = hero(2, 3, 8, vec![draw(1, 1)]);
        one.starting_hand = 30;
        let (state, _) = new_game(setup(one, hero(1, 2, 5, vec![draw(1, 1)])), 1).unwrap();
        assert_eq!(state.deck_len(PlayerId::One), 0);
        assert_eq!(state.winner(), None);
        assert!(!legal_actions(&state).is_empty());
    }

    #[test]
    fn new_game_ends_at_once_when_the_first_player_has_nothing_to_do() {
        // One starts (initiative 2) with an empty hand and no effect: exhausted at setup.
        let mut one = hero(2, 3, 8, vec![]);
        one.starting_hand = 0;
        let (state, events) = new_game(setup(one, hero(1, 2, 5, vec![draw(1, 1)])), 3).unwrap();
        assert_eq!(state.active(), PlayerId::One);
        assert_eq!(state.winner(), Some((PlayerId::Two, WinReason::Exhaustion)));
        assert!(matches!(events.first(), Some(Event::GameStarted { .. })));
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
    fn check_exhaustion_ends_a_running_game_whose_active_player_has_nothing_to_do() {
        let mut state = running(vec![draw(1, 1)]);
        state.one.hand.clear();
        state.one.deck.clear();
        let mut events = Vec::new();
        crate::rules::check_exhaustion(&mut state, &mut events);
        assert_eq!(state.winner(), Some((PlayerId::Two, WinReason::Exhaustion)));
        assert!(matches!(events.last(), Some(Event::GameWon { .. })));
        assert!(legal_actions(&state).is_empty());
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
        assert!(!legal_actions(&state).contains(&Action::Activate { effect: 0 }));
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

    #[test]
    fn the_potential_strictly_decreases_at_every_action_even_when_creatures_are_attacked() {
        // Everything `legal_actions` offers is drawn at random, creature targets included.
        let (mut games, mut strikes, mut hero_hits, mut deaths) = (0, 0, 0, 0);
        for seed in 0..250u64 {
            let (mut state, _) = new_game(
                setup(
                    hero(1 + (seed % 2) as u32, 3, 8, vec![draw(1, 1), draw(4, 3)]),
                    hero(1, 2, 6, vec![draw(2, 2)]),
                ),
                seed,
            )
            .unwrap();
            let mut policy = ChaCha8Rng::seed_from_u64(seed ^ 0x5A5A);
            let mut steps = 0;
            while state.winner().is_none() {
                let playable: Vec<Action> = legal_actions(&state)
                    .into_iter()
                    .filter(|a| *a != Action::Concede)
                    .collect();
                let pick = playable
                    .get(policy.next_u32() as usize % playable.len().max(1))
                    .copied()
                    .unwrap_or(Action::Concede);
                match pick {
                    Action::Attack {
                        target: crate::Target::Creature(_),
                        ..
                    } => strikes += 1,
                    Action::Attack { .. } => hero_hits += 1,
                    _ => {}
                }
                let before = potential(&state);
                let events = apply(&mut state, pick).unwrap();
                deaths += events
                    .iter()
                    .filter(|e| matches!(e, Event::Died { .. }))
                    .count();
                if pick != Action::Concede {
                    assert!(
                        potential(&state) < before,
                        "seed {seed}: {pick:?} did not decrease the potential"
                    );
                }
                steps += 1;
                assert!(steps < 2000, "seed {seed}: the game does not end");
            }
            games += 1;
        }
        // Not vacuous: creatures really are attacked and really die along the way.
        assert!(games >= 200, "{games} games");
        assert!(strikes > 200, "attacks on creatures: {strikes}");
        assert!(hero_hits > 200, "attacks on heroes: {hero_hits}");
        assert!(deaths > 200, "deaths: {deaths}");
    }

    #[test]
    fn a_summoning_that_hands_over_to_an_exhausted_receiver_ends_the_game() {
        // One's summoning costs 8 against a budget of 8: the token crosses to Two, who has
        // neither card nor creature to act with.
        let mut state = running(vec![]);
        state.two.deck.clear();
        state.two.hand.clear();
        state.one.hand[0].stats.cost = 8;
        let card = state.one.hand[0].id;
        let events = apply(&mut state, Action::Play { card }).unwrap();
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

    // ---- riposte (S2b-T3) ---------------------------------------------------------------

    /// A creature of id `id` with `force`, `health`, `riposte` and `attack_cost`.
    fn riposter(id: u32, force: u32, health: u32, riposte: u32, attack_cost: u32) -> CardInstance {
        CardInstance {
            id: CardInstanceId(id),
            stats: stats_with(1, force, attack_cost, health, false, riposte),
        }
    }

    fn died(player: PlayerId, id: u32) -> Event {
        Event::Died {
            player,
            creature: CardInstanceId(id),
        }
    }

    /// `running` (One active with 8 time units, a draw effect) with One's hand and pile
    /// emptied, `attacker` on One's board and `target` on Two's, Two's hand and pile emptied.
    fn riposte_scene(attacker: CardInstance, target: CardInstance) -> GameState {
        let mut state = running(vec![draw(1, 1)]);
        state.one.hand.clear();
        state.one.deck.clear();
        state.one.board = vec![attacker];
        state.two.hand.clear();
        state.two.deck.clear();
        state.two.board = vec![target];
        state
    }

    /// The attacker (id 100, 1 health, attack cost `attack_cost`) dies of the riposte of the
    /// target (id 101, 5 health).
    fn doomed_attacker(attack_cost: u32) -> GameState {
        riposte_scene(
            riposter(100, 1, 1, 0, attack_cost),
            riposter(101, 1, 5, 1, 1),
        )
    }

    fn doomed_strike() -> Action {
        strike(CardInstanceId(100), CardInstanceId(101))
    }

    #[test]
    fn a_riposte_killing_the_last_creature_of_a_player_who_stays_active_exhausts_them() {
        // c < b (1 < 8): One stays active with no card, no creature and an effect that
        // cannot be activated (empty pile).
        let mut state = doomed_attacker(1);
        let events = apply(&mut state, doomed_strike()).unwrap();
        assert_eq!(state.winner(), Some((PlayerId::Two, WinReason::Exhaustion)));
        assert_eq!((state.active(), state.budget()), (PlayerId::One, 7));
        let tail: Vec<_> = events.iter().rev().take(3).rev().cloned().collect();
        assert_eq!(
            tail,
            vec![
                died(PlayerId::One, 100),
                Event::TokenMoved {
                    player: PlayerId::One,
                    budget: 7
                },
                Event::GameWon {
                    winner: PlayerId::Two,
                    reason: WinReason::Exhaustion
                },
            ]
        );
        assert!(legal_actions(&state).is_empty());
    }

    #[test]
    fn a_riposte_killing_the_last_creature_does_not_exhaust_a_player_with_a_card_in_hand() {
        let mut state = doomed_attacker(1);
        let extra = running(vec![draw(1, 1)]).one.hand.remove(0);
        state.one.hand.push(extra);
        apply(&mut state, doomed_strike()).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!(state.active(), PlayerId::One);
    }

    #[test]
    fn a_riposte_killing_the_last_creature_does_not_exhaust_a_player_with_another_creature() {
        let mut state = doomed_attacker(1);
        state.one.board.push(riposter(102, 1, 4, 0, 1));
        apply(&mut state, doomed_strike()).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!(state.active(), PlayerId::One);
    }

    #[test]
    fn a_riposte_killing_the_last_creature_does_not_exhaust_a_player_whose_effect_applies() {
        // The hero can still draw: the pile is not empty.
        let mut state = doomed_attacker(1);
        let extra = running(vec![draw(1, 1)]).one.hand.remove(0);
        state.one.deck.push(extra);
        apply(&mut state, doomed_strike()).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!(state.active(), PlayerId::One);
        assert!(legal_actions(&state).contains(&Action::Activate { effect: 0 }));
    }

    /// Both creatures die (force 1 against 1 health, riposte 1 against 1 health), the attack
    /// costing the whole budget: the token crosses to Two, whose board is now empty.
    fn mutual_kill_handing_over() -> GameState {
        riposte_scene(riposter(100, 1, 1, 0, 8), riposter(101, 1, 1, 1, 1))
    }

    #[test]
    fn a_handover_after_a_mutual_kill_to_a_receiver_left_with_nothing_ends_the_game() {
        let mut state = mutual_kill_handing_over();
        let events = apply(&mut state, doomed_strike()).unwrap();
        assert_eq!(state.winner(), Some((PlayerId::One, WinReason::Exhaustion)));
        assert_eq!(state.active(), PlayerId::Two);
        let tail: Vec<_> = events.iter().rev().take(5).rev().cloned().collect();
        assert_eq!(
            tail,
            vec![
                died(PlayerId::One, 100),
                died(PlayerId::Two, 101),
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
    fn a_handover_after_a_mutual_kill_to_a_receiver_with_a_card_does_not_end_the_game() {
        let mut state = mutual_kill_handing_over();
        let extra = running(vec![draw(1, 1)]).one.hand.remove(0);
        state.two.hand.push(extra);
        apply(&mut state, doomed_strike()).unwrap();
        assert_eq!(state.winner(), None);
        assert_eq!(state.active(), PlayerId::Two);
    }

    /// One and Two each have one creature that kills the other, ids as given; `attacker_side`
    /// is the side that attacks. Returns the log of the attack and the final state.
    fn mutual_kill(attacker_side: PlayerId, attacker: u32, target: u32) -> (Vec<Event>, GameState) {
        let mut state = running(vec![draw(1, 1)]);
        let (a, t) = (riposter(attacker, 2, 2, 2, 1), riposter(target, 2, 2, 2, 1));
        let (mine, theirs) = match attacker_side {
            PlayerId::One => (&mut state.one, &mut state.two),
            PlayerId::Two => (&mut state.two, &mut state.one),
        };
        mine.board = vec![a];
        theirs.board = vec![t];
        state.active = attacker_side;
        state.budget = 3; // within both heroes' max_time; an attack costs 1
        let events = apply(
            &mut state,
            strike(CardInstanceId(attacker), CardInstanceId(target)),
        )
        .unwrap();
        (events, state)
    }

    fn assert_deaths_in_id_order(attacker_side: PlayerId, attacker: u32, target: u32) {
        let (events, state) = mutual_kill(attacker_side, attacker, target);
        let deaths: Vec<Event> = events
            .into_iter()
            .filter(|e| matches!(e, Event::Died { .. }))
            .collect();
        let (low, high) = (attacker.min(target), attacker.max(target));
        let owner = |id| {
            if id == attacker {
                attacker_side
            } else {
                attacker_side.opponent()
            }
        };
        assert_eq!(deaths, vec![died(owner(low), low), died(owner(high), high)]);
        for side in [PlayerId::One, PlayerId::Two] {
            assert!(state.board(side).is_empty());
        }
        assert_eq!(state.discard(attacker_side), vec![CardInstanceId(attacker)]);
        assert_eq!(
            state.discard(attacker_side.opponent()),
            vec![CardInstanceId(target)]
        );
    }

    #[test]
    fn simultaneous_deaths_follow_the_id_order_when_the_attacker_has_the_lower_id() {
        // Side order (One, Two) and id order disagree for the second case.
        assert_deaths_in_id_order(PlayerId::One, 5, 40);
        assert_deaths_in_id_order(PlayerId::Two, 5, 40);
    }

    #[test]
    fn simultaneous_deaths_follow_the_id_order_when_the_attacker_has_the_higher_id() {
        assert_deaths_in_id_order(PlayerId::One, 40, 5);
        assert_deaths_in_id_order(PlayerId::Two, 40, 5);
    }

    /// Random games with ripostes of 0..=4 and about a third of provocateurs, on both sides.
    fn riposting_game(seed: u64) -> GameState {
        let mut s = setup(
            hero(1 + (seed % 2) as u32, 3, 8, vec![draw(1, 1), draw(4, 3)]),
            hero(1, 2, 6, vec![draw(2, 2)]),
        );
        let cards = |offset: u32| {
            (0..30u32)
                .map(|n| {
                    let n = n + offset;
                    stats_with(
                        1 + n % 3,
                        1 + n % 3,
                        1 + n % 2,
                        1 + n % 4,
                        n.is_multiple_of(3),
                        n % 5,
                    )
                })
                .collect::<Vec<_>>()
        };
        s.one.cards = cards(0);
        s.two.cards = cards(2);
        new_game(s, seed).unwrap().0
    }

    #[test]
    fn the_potential_strictly_decreases_at_every_action_with_ripostes_and_provocation() {
        let (mut ripostes, mut riposte_deaths, mut mutual_deaths) = (0, 0, 0);
        for seed in 0..250u64 {
            let mut state = riposting_game(seed);
            let mut policy = ChaCha8Rng::seed_from_u64(seed ^ 0xB10C);
            let mut steps = 0;
            while state.winner.is_none() {
                let playable: Vec<Action> = legal_actions(&state)
                    .into_iter()
                    .filter(|a| *a != Action::Concede)
                    .collect();
                assert!(!playable.is_empty(), "seed {seed}: exhausted yet not over");
                let pick = playable[policy.next_u32() as usize % playable.len()];
                let before = potential(&state);
                let events = apply(&mut state, pick).unwrap();
                assert!(
                    potential(&state) < before,
                    "seed {seed}: {pick:?} did not decrease the potential"
                );
                ripostes += events
                    .iter()
                    .filter(|e| matches!(e, Event::Riposted { .. }))
                    .count();
                let deaths: Vec<&Event> = events
                    .iter()
                    .filter(|e| matches!(e, Event::Died { .. }))
                    .collect();
                for e in &events {
                    if let Event::Riposted {
                        player, creature, ..
                    } = e
                        && deaths.contains(&&Event::Died {
                            player: *player,
                            creature: *creature,
                        })
                    {
                        riposte_deaths += 1;
                    }
                }
                if deaths.len() >= 2 && events.iter().any(|e| matches!(e, Event::Riposted { .. })) {
                    mutual_deaths += 1;
                }
                steps += 1;
                assert!(steps < 2000, "seed {seed}: the game does not end");
            }
        }
        // Not vacuous.
        assert!(ripostes > 0, "ripostes: {ripostes}");
        assert!(
            riposte_deaths > 0,
            "attackers killed by a riposte: {riposte_deaths}"
        );
        assert!(mutual_deaths > 0, "simultaneous deaths: {mutual_deaths}");
    }
}
