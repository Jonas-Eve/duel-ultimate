//! Game creation: validation, deterministic shuffle, opening hands, who starts.

use crate::rules::{self, Event};
use crate::ruleset::{RuleSet, RuleSetError};
use crate::state::{CardInstance, CardInstanceId, GameState, PlayerId, PlayerState};
use duel_cards::{
    CardSpec, CardsError, Hero, validate_card_names, validate_card_spec, validate_deck_shape,
    validate_hero,
};
use serde::{Deserialize, Serialize};

/// One side of a game: a hero and the validated cards (`CardSpec`) of its 30 slots, in deck order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerSetup {
    pub hero: Hero,
    pub cards: Vec<CardSpec>,
    /// Optional display names, parallel to `cards` (same order). Absent or empty: no names.
    /// Presentation only: no rule, formula or draw reads them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_names: Vec<String>,
}

/// Everything a game needs. `new_game` reads neither disk nor identifiers: callers resolve
/// decks with `duel-cards` first (the `data` module does it for the callers that start from
/// `cards.json` / `decks.json` strings, by deck identifier, still without any I/O). `data_versions` is opaque and untrusted: recorded as is in
/// the state and copied into `Event::GameStarted` (replays, balance), never interpreted,
/// validated or sorted. The key is mandatory in JSON (an empty list is valid). `rules` is
/// optional: absent reads as the engine default and is written only when it differs from it;
/// `new_game` validates it (the options may come from an untrusted client). To replay a
/// historical game, pass its rules explicitly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameSetup {
    pub one: PlayerSetup,
    pub two: PlayerSetup,
    pub data_versions: Vec<String>,
    #[serde(default, skip_serializing_if = "RuleSet::is_default")]
    pub rules: RuleSet,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SetupError {
    #[error("invalid game data: {0}")]
    Invalid(#[from] CardsError),
    #[error("invalid rules: {0}")]
    InvalidRules(#[from] RuleSetError),
}

impl SetupError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) | Self::InvalidRules(_) => "invalid_setup",
        }
    }
}

fn validate_side(side: &PlayerSetup) -> Result<(), CardsError> {
    validate_hero(&side.hero)?;
    validate_deck_shape(&side.hero, side.cards.len())?;
    validate_card_names(&side.card_names, side.cards.len())?;
    side.cards.iter().try_for_each(validate_card_spec)
}

/// Builds a player's pile in deck order; `next_id` runs on across both players so that ids
/// follow the deterministic order of the setup (One's deck, then Two's), before shuffling.
fn build_side(side: PlayerSetup, next_id: &mut u32) -> PlayerState {
    // The names are paired with their cards here, before the shuffle, so a name follows its
    // card into every zone. `validate_side` guarantees none, or exactly one per card.
    let mut names = side.card_names.into_iter();
    let deck = side
        .cards
        .into_iter()
        .map(|spec| {
            let CardSpec::Creature(stats) = spec;
            let id = CardInstanceId(*next_id);
            *next_id += 1;
            CardInstance {
                id,
                stats,
                name: names.next(),
            }
        })
        .collect();
    PlayerState {
        health: side.hero.health,
        hero: side.hero,
        deck,
        hand: Vec::new(),
        board: Vec::new(),
        discard: Vec::new(),
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
/// panics. The decks are validated first, then `setup.rules`. The events hold `GameStarted`
/// (with a copy of `setup.data_versions` and, when not the default, `setup.rules`, which
/// influence neither the generator nor, yet, any rule) and, if the first player has nothing to
/// do, `GameWon`.
///
/// The generator is consumed in this order: One's shuffle, Two's shuffle, then
/// `random_below(2)` only if both heroes have the same initiative (0 means One).
/// Opening hands cost no time and no randomness. The second player's `start_budget` stays
/// unused until the token is handed over (the budget then comes from the time rule).
pub fn new_game(setup: GameSetup, seed: u64) -> Result<(GameState, Vec<Event>), SetupError> {
    validate_side(&setup.one)?;
    validate_side(&setup.two)?;
    setup.rules.validate()?;
    let mut next_id = 0;
    let one = build_side(setup.one, &mut next_id);
    let two = build_side(setup.two, &mut next_id);
    let mut state = GameState::from_parts(one, two, setup.data_versions, setup.rules, seed);

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
        data_versions: state.data_versions().to_vec(),
        rules: state.rules(),
    }];
    rules::check_exhaustion(&mut state, &mut events);
    Ok((state, events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Action, apply, legal_actions};
    use crate::state::CardInstance;
    use crate::testing::{hero, setup};

    /// Names `c{n}`, where `n` is the cost of the card: `testing::deck()` gives its 30 cards
    /// the distinct costs 1..=30, so a name says which stats it must sit next to.
    fn named(mut setup: GameSetup) -> GameSetup {
        for side in [&mut setup.one, &mut setup.two] {
            side.card_names = side
                .cards
                .iter()
                .map(|s| format!("c{}", s.creature().cost))
                .collect();
        }
        setup
    }

    /// Every card of every zone of both players.
    fn all_cards(state: &GameState) -> Vec<&CardInstance> {
        [&state.one, &state.two]
            .into_iter()
            .flat_map(|p| {
                p.deck
                    .iter()
                    .chain(&p.hand)
                    .chain(&p.board)
                    .chain(&p.discard)
            })
            .collect()
    }

    fn assert_names_follow_stats(state: &GameState, context: &str) {
        let cards = all_cards(state);
        assert_eq!(cards.len(), 60, "{context}: no card appears or vanishes");
        for card in cards {
            assert_eq!(
                card.name.as_deref(),
                Some(format!("c{}", card.stats.cost).as_str()),
                "{context}: card {:?}",
                card.id
            );
        }
    }

    #[test]
    fn a_game_without_names_gives_every_instance_no_name() {
        let (state, _) = new_game(setup(hero(2, 3, 8, vec![]), hero(1, 1, 8, vec![])), 3).unwrap();
        assert!(all_cards(&state).iter().all(|c| c.name.is_none()));
    }

    #[test]
    fn the_name_follows_its_card_through_the_shuffle_and_the_deal() {
        for seed in 0..30 {
            let (state, _) = new_game(
                named(setup(hero(2, 3, 8, vec![]), hero(1, 1, 8, vec![]))),
                seed,
            )
            .unwrap();
            assert_names_follow_stats(&state, &format!("seed {seed}"));
            // The shuffle really moved cards: the pile is not in deck order.
            let costs: Vec<u32> = state.one.deck.iter().map(|c| c.stats.cost).collect();
            assert!(costs.windows(2).any(|w| w[0] > w[1]), "seed {seed}");
        }
    }

    #[test]
    fn the_name_follows_its_card_to_the_hand_the_board_and_the_discard() {
        let mut seen_board = false;
        let mut seen_discard = false;
        for seed in 0..12 {
            let draw = crate::testing::draw(1, 2);
            let setup = named(setup(
                hero(2, 3, 8, vec![draw.clone()]),
                hero(1, 1, 8, vec![draw]),
            ));
            let (mut state, _) = new_game(setup, seed).unwrap();
            for step in 0..3_000_usize {
                let actions = legal_actions(&state);
                let playable: Vec<&Action> =
                    actions.iter().filter(|a| **a != Action::Concede).collect();
                let Some(action) = playable.get(step.wrapping_mul(7) % playable.len().max(1))
                else {
                    break;
                };
                apply(&mut state, (*action).clone()).unwrap();
                assert_names_follow_stats(&state, &format!("seed {seed} step {step}"));
                seen_board |= !state.one.board.is_empty() || !state.two.board.is_empty();
                seen_discard |= !state.one.discard.is_empty() || !state.two.discard.is_empty();
            }
        }
        assert!(seen_board, "a card reached a board");
        assert!(seen_discard, "a card reached a discard");
    }

    #[test]
    fn a_name_is_per_instance_not_per_position() {
        // Names listed in reverse order against the stats: each instance keeps the pairing the
        // setup gave it (by position in the deck, before the shuffle), whatever the shuffle does.
        let mut s = setup(hero(2, 3, 8, vec![]), hero(1, 1, 8, vec![]));
        s.one.card_names = s
            .one
            .cards
            .iter()
            .rev()
            .map(|c| format!("c{}", c.creature().cost))
            .collect();
        let (state, _) = new_game(s, 5).unwrap();
        for card in &state.one.deck {
            let n = 31 - card.stats.cost;
            assert_eq!(card.name.as_deref(), Some(format!("c{n}").as_str()));
        }
    }
}
