//! Game creation: validation, deterministic shuffle, opening hands, who starts.

use crate::rules::{self, Event};
use crate::ruleset::{RuleSet, RuleSetError};
use crate::state::{
    CardExtras, CardInstance, CardInstanceId, CardKind, GameState, PlayerId, PlayerState,
};
use duel_cards::{
    CardSpec, CardsError, CreatureStats, Hero, validate_card_names, validate_card_spec,
    validate_deck_shape, validate_hero,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
            // `validate_side` revalidated every spec: a spell carries its cost and zeros (the
            // shape of an instance, with no health: Φ), an immobile unit has a force and an attack
            // cost of 0. A plain unit has no extras, so a game of plain units clones what it did.
            let plain = spec.is_plain_unit();
            let (stats, extras) = match spec {
                CardSpec::Unit(unit) => (
                    unit.stats,
                    CardExtras {
                        kind: CardKind::Unit,
                        immobile: unit.immobile,
                        tags: unit.tags,
                        effects: unit.effects,
                    },
                ),
                CardSpec::Spell(spell) => (
                    CreatureStats {
                        cost: spell.cost,
                        force: 0,
                        attack_cost: 0,
                        health: 0,
                        provocation: false,
                        riposte: 0,
                    },
                    CardExtras {
                        kind: CardKind::Spell,
                        immobile: false,
                        tags: spell.tags,
                        effects: spell.effects,
                    },
                ),
            };
            let id = CardInstanceId(*next_id);
            *next_id += 1;
            CardInstance {
                id,
                stats,
                name: names.next(),
                extras: (!plain).then(|| Arc::new(extras)),
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
                .map(|s| format!("c{}", s.creature().unwrap().cost))
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
            .map(|c| format!("c{}", c.creature().unwrap().cost))
            .collect();
        let (state, _) = new_game(s, 5).unwrap();
        for card in &state.one.deck {
            let n = 31 - card.stats.cost;
            assert_eq!(card.name.as_deref(), Some(format!("c{n}").as_str()));
        }
    }

    #[test]
    fn a_plain_unit_carries_no_extras_and_the_other_cards_carry_theirs() {
        use crate::state::CardKind;
        use crate::testing::{immobile, on_play_draw, spell};
        let mut s = setup(hero(2, 3, 8, vec![]), hero(1, 1, 8, vec![]));
        let mut tagged = s.one.cards[2].clone();
        if let duel_cards::CardSpec::Unit(unit) = &mut tagged {
            unit.tags = vec!["bâtiment".into()];
        }
        s.one.cards[0] = spell(7);
        s.one.cards[1] = immobile(6, 5);
        s.one.cards[2] = tagged;
        let (state, _) = new_game(s, 3).unwrap();
        let all = all_cards(&state);
        let spells: Vec<_> = all.iter().filter(|c| c.is_spell()).collect();
        let immobiles: Vec<_> = all.iter().filter(|c| c.immobile()).collect();
        let tagged: Vec<_> = all.iter().filter(|c| !c.tags().is_empty()).collect();
        assert_eq!((spells.len(), immobiles.len(), tagged.len()), (1, 1, 1));
        assert_eq!(spells[0].kind(), CardKind::Spell);
        assert_eq!(immobiles[0].kind(), CardKind::Unit);
        assert_eq!(tagged[0].tags(), ["bâtiment".to_owned()]);
        // Only those three carry extras: the 57 other cards are plain units.
        assert_eq!(all.iter().filter(|c| c.extras.is_some()).count(), 3);
        // Every instance of a plain-unit deck has none: the clone per action costs what it did.
        let (plain_state, _) =
            new_game(setup(hero(2, 3, 8, vec![]), hero(1, 1, 8, vec![])), 3).unwrap();
        assert!(all_cards(&plain_state).iter().all(|c| c.extras.is_none()));
        // The accessors give the defaults of a plain unit.
        let card = all_cards(&plain_state)[0];
        assert_eq!(card.kind(), CardKind::Unit);
        assert!(!card.is_spell() && !card.immobile());
        assert!(card.tags().is_empty() && card.effects().is_empty());
        // A spell has the shape of a card with a cost and zeros, and a declared effect is kept.
        let mut with_effect = setup(hero(2, 3, 8, vec![]), hero(1, 1, 8, vec![]));
        with_effect.one.cards[0] = crate::testing::unit_with_effects(
            crate::testing::stats(4, 1, 1, 2),
            vec![on_play_draw()],
        );
        let (state, _) = new_game(with_effect, 3).unwrap();
        let carrier = all_cards(&state)
            .into_iter()
            .find(|c| !c.effects().is_empty())
            .unwrap();
        assert_eq!(carrier.effects(), &[on_play_draw()]);
        assert_eq!(carrier.stats.cost, 4);
    }

    #[test]
    fn a_spell_instance_has_its_cost_and_zeros() {
        let mut s = setup(hero(2, 3, 8, vec![]), hero(1, 1, 8, vec![]));
        s.one.cards[0] = crate::testing::spell(9);
        let (state, _) = new_game(s, 3).unwrap();
        let spell = all_cards(&state)
            .into_iter()
            .find(|c| c.is_spell())
            .unwrap();
        assert_eq!(
            spell.stats,
            duel_cards::CreatureStats {
                cost: 9,
                force: 0,
                attack_cost: 0,
                health: 0,
                provocation: false,
                riposte: 0
            }
        );
    }
}
