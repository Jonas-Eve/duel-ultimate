//! Game creation: validation, deterministic shuffle, opening hands, who starts.

use crate::rules::{self, Event};
use crate::state::{CardInstance, CardInstanceId, GameState, PlayerId, PlayerState};
use duel_cards::{
    CardsError, CreatureStats, Hero, validate_creature, validate_deck_shape, validate_hero,
};
use serde::{Deserialize, Serialize};

/// One side of a game: a hero and the resolved stats of its 30 cards, in deck order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerSetup {
    pub hero: Hero,
    pub cards: Vec<CreatureStats>,
}

/// Everything a game needs. The engine reads neither disk nor identifiers: callers resolve
/// decks with `duel-cards` first. `data_versions` is recorded as is (replays, balance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameSetup {
    pub one: PlayerSetup,
    pub two: PlayerSetup,
    pub data_versions: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SetupError {
    #[error("invalid game data: {0}")]
    Invalid(#[from] CardsError),
}

impl SetupError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "invalid_setup",
        }
    }
}

fn validate_side(side: &PlayerSetup) -> Result<(), CardsError> {
    validate_hero(&side.hero)?;
    validate_deck_shape(&side.hero, side.cards.len())?;
    side.cards.iter().try_for_each(validate_creature)
}

/// Builds a player's pile in deck order; `next_id` runs on across both players so that ids
/// follow the deterministic order of the setup (One's deck, then Two's), before shuffling.
fn build_side(side: PlayerSetup, next_id: &mut u32) -> PlayerState {
    let deck = side
        .cards
        .into_iter()
        .map(|stats| {
            let id = CardInstanceId(*next_id);
            *next_id += 1;
            CardInstance { id, stats }
        })
        .collect();
    PlayerState {
        health: side.hero.health,
        hero: side.hero,
        deck,
        hand: Vec::new(),
        board: Vec::new(),
    }
}

/// Fisher-Yates through the game's generator (not `SliceRandom::shuffle`, whose use of the
/// generator is not ours to pin down).
fn shuffle(state: &mut GameState, player: PlayerId) {
    let mut deck = std::mem::take(&mut state.player_mut(player).deck);
    for i in (1..deck.len()).rev() {
        let bound = u32::try_from(i + 1).unwrap_or(u32::MAX);
        let j = usize::try_from(state.random_below(bound)).unwrap_or(0);
        deck.swap(i, j);
    }
    state.player_mut(player).deck = deck;
}

/// Moves the top `starting_hand` cards of the pile (the end of the `Vec`) to the hand.
fn deal(state: &mut GameState, player: PlayerId) {
    let side = state.player_mut(player);
    let wanted = usize::try_from(side.hero.starting_hand).unwrap_or(usize::MAX);
    let keep = side.deck.len().saturating_sub(wanted);
    let mut dealt = side.deck.split_off(keep);
    dealt.reverse(); // the top card first, as if drawn one by one
    side.hand.extend(dealt);
}

/// Starts a game. Re-validates `setup` (it may come from an untrusted client) and never
/// panics. The events hold `GameStarted` and, if the first player has nothing to do,
/// `GameWon`.
///
/// The generator is consumed in this order: One's shuffle, Two's shuffle, then
/// `random_below(2)` only if both heroes have the same initiative (0 means One).
/// Opening hands cost no time and no randomness. The second player's `start_budget` stays
/// unused until the token is handed over (the budget then comes from the time rule).
pub fn new_game(setup: GameSetup, seed: u64) -> Result<(GameState, Vec<Event>), SetupError> {
    validate_side(&setup.one)?;
    validate_side(&setup.two)?;
    let mut next_id = 0;
    let one = build_side(setup.one, &mut next_id);
    let two = build_side(setup.two, &mut next_id);
    let mut state = GameState::from_parts(one, two, setup.data_versions, seed);

    shuffle(&mut state, PlayerId::One);
    shuffle(&mut state, PlayerId::Two);
    deal(&mut state, PlayerId::One);
    deal(&mut state, PlayerId::Two);

    let (init_one, init_two) = (state.one.hero.initiative, state.two.hero.initiative);
    state.active = match init_one.cmp(&init_two) {
        std::cmp::Ordering::Greater => PlayerId::One,
        std::cmp::Ordering::Less => PlayerId::Two,
        std::cmp::Ordering::Equal if state.random_below(2) == 0 => PlayerId::One,
        std::cmp::Ordering::Equal => PlayerId::Two,
    };
    state.budget = state.player(state.active).hero.start_budget;

    let mut events = vec![Event::GameStarted {
        first: state.active,
        budget: state.budget,
    }];
    rules::check_exhaustion(&mut state, &mut events);
    Ok((state, events))
}
