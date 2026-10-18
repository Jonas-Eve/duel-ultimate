//! Actions, events and the rules that apply them.

use crate::effects::{self, effective};
use crate::ruleset::RuleSet;
use crate::state::{CardInstance, CardInstanceId, GameState, PlayerId, WinReason};
use crate::time::{self, Spend};
use duel_cards::HeroEffect;
use serde::{Deserialize, Serialize};

/// The most `picks` an [`Action`] may carry; the engine refuses more with
/// [`RuleError::TooManyPicks`], before reading the state. The bound lives in `validate`, not in
/// the deserializer, so that the refusal keeps a stable code (a bound in the deserializer would
/// surface as `invalid_json`). The input of the WASM binding is capped at 1 MiB before any
/// analysis, so a list is at most about 70 000 elements long when it reaches the engine: bounded,
/// harmless, never a reason to cap it earlier.
pub const MAX_PICKS: usize = 8;

/// A choice carried by an [`Action`]: the hero of a seat or a creature in play (untrusted, judged
/// by the engine). Distinct from [`Target`]: a spell may aim at the **own** hero, while
/// `Target::Hero` always means the opposing one. JSON: `{"hero":"One"}` or `{"creature":7}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pick {
    Hero(PlayerId),
    Creature(CardInstanceId),
}

/// What a player may do. The contract is additive: the JSON of an action without `picks` and
/// `source` is what it always was (both are omitted when empty). No legal action carries them at
/// this stage; only a hostile caller sends them, and `validate` refuses them with
/// `too_many_picks`, `invalid_picks` or `source_not_on_board`.
///
/// Architecture decision: the normal attack (`Attack { attacker, target }`, [`Target`]) is frozen.
/// A future special attack is written `Activate { source: Some(attacker), effect: k, picks:
/// [target] }`. `source` absent means the hero.
///
/// `Action` is not `Copy` (the list of picks is free): clone it when it must be kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Summons a creature of the active player's hand (untrusted id, validated by the engine).
    Play {
        card: CardInstanceId,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        picks: Vec<Pick>,
    },
    /// Activates an effect: the active hero's effect at this index of its list when `source` is
    /// absent, else an effect of the creature `source` of the active player's board (untrusted
    /// input).
    Activate {
        effect: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<CardInstanceId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        picks: Vec<Pick>,
    },
    /// Attacks with a creature of the active player's board (untrusted id, validated).
    Attack {
        attacker: CardInstanceId,
        target: Target,
    },
    Concede,
}

impl Action {
    /// `Play { card }`: summons a creature of the active player's hand.
    pub fn play(card: CardInstanceId) -> Action {
        Action::Play {
            card,
            picks: Vec::new(),
        }
    }

    /// `Activate { effect }`: activates the active hero's effect at this index.
    pub fn activate(effect: u32) -> Action {
        Action::Activate {
            effect,
            source: None,
            picks: Vec::new(),
        }
    }
}

/// What an attack aims at: the opposing hero or a creature of the opposing board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Hero,
    /// An opposing creature in play (untrusted id, validated by the engine).
    Creature(CardInstanceId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    /// The game began. `data_versions` is the opaque list of the setup, copied as is; it is
    /// absent from the JSON when empty. `rules` is last: the complete option object of the
    /// game, absent when it is the engine default. A journal with neither versions nor
    /// non-default options keeps the exact form it had before these fields existed.
    GameStarted {
        first: PlayerId,
        budget: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        data_versions: Vec<String>,
        #[serde(default, skip_serializing_if = "RuleSet::is_default")]
        rules: RuleSet,
    },
    EffectActivated {
        player: PlayerId,
        effect: u32,
        cost: u32,
    },
    Drew {
        player: PlayerId,
        card: CardInstanceId,
    },
    Played {
        player: PlayerId,
        card: CardInstanceId,
        cost: u32,
    },
    /// A creature attacks: `cost` is the time it spent.
    Attacked {
        player: PlayerId,
        attacker: CardInstanceId,
        cost: u32,
    },
    /// The hero of `player` (the one attacked) took `amount` damage.
    Damaged {
        player: PlayerId,
        amount: u32,
    },
    /// The creature `creature` of `player` (its owner) took `amount` damage (the attacker's
    /// nominal force: the excess over its remaining health is lost).
    CreatureDamaged {
        player: PlayerId,
        creature: CardInstanceId,
        amount: u32,
    },
    /// The creature `creature` of `player` (the attacker, the one that suffers) took the
    /// riposte of its target: `amount` is the target's nominal riposte (the excess over the
    /// attacker's remaining health is lost). Only for an attack on a creature with `riposte >= 1`,
    /// even if that target dies of the attack.
    Riposted {
        player: PlayerId,
        creature: CardInstanceId,
        amount: u32,
    },
    /// The hero of the defender's side hit back (`hero_riposte`): the creature `creature` of
    /// `player` (the attacker, as in [`Event::Riposted`]) is destroyed, whatever its health.
    /// No amount: `Riposted::amount` names the nominal riposte of a creature target, and this
    /// one is not a number. Only for a non-lethal attack on the hero with the option on,
    /// after `Damaged` and before the `Died` that buries the attacker.
    HeroRiposted {
        player: PlayerId,
        creature: CardInstanceId,
    },
    /// The creature left `player`'s board for their discard pile.
    Died {
        player: PlayerId,
        creature: CardInstanceId,
    },
    /// After every non-final action: `player` holds the token with `budget`.
    TokenMoved {
        player: PlayerId,
        budget: u32,
    },
    /// Only when the token crossed the boundary.
    ActivePlayerChanged {
        player: PlayerId,
    },
    GameWon {
        winner: PlayerId,
        reason: WinReason,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("the game is already over")]
    GameOver,
    #[error("that card is not in the active player's hand")]
    CardNotInHand,
    #[error("the active hero has no such effect")]
    UnknownEffect,
    #[error("this effect cannot be activated right now")]
    EffectNotApplicable,
    #[error("that creature is not on the active player's board")]
    AttackerNotOnBoard,
    #[error("that target is not a creature of the opposing board")]
    TargetNotOnBoard,
    #[error("an opposing creature with provocation must be attacked first")]
    TargetForbiddenByProvocation,
    #[error("the active player's board is full")]
    BoardFull,
    #[error("an action carries too many picks")]
    TooManyPicks,
    #[error("this action does not take these picks")]
    InvalidPicks,
    #[error("the source is not a creature of the active player's board")]
    SourceNotOnBoard,
    #[error("this unit cannot attack: its force is zero")]
    AttackForceZero,
    /// **Temporary** (#208): a card whose effect the game cannot resolve yet, a spell to play or
    /// a declared `activated` effect of a unit. Removed by #129 (effects of units), #130 (spells)
    /// and #131 when they deliver the behavior.
    #[error("this card uses an effect the game cannot resolve yet")]
    UnsupportedEffect,
}

impl RuleError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::GameOver => "game_over",
            Self::CardNotInHand => "card_not_in_hand",
            Self::UnknownEffect => "unknown_effect",
            Self::EffectNotApplicable => "effect_not_applicable",
            Self::AttackerNotOnBoard => "attacker_not_on_board",
            Self::TargetNotOnBoard => "target_not_on_board",
            Self::TargetForbiddenByProvocation => "target_forbidden_by_provocation",
            Self::BoardFull => "board_full",
            Self::TooManyPicks => "too_many_picks",
            Self::InvalidPicks => "invalid_picks",
            Self::SourceNotOnBoard => "source_not_on_board",
            Self::AttackForceZero => "attack_force_zero",
            Self::UnsupportedEffect => "unsupported_effect",
        }
    }
}

/// The effect of the active hero at the untrusted index `effect`, if there is one.
pub(crate) fn hero_effect(state: &GameState, effect: u32) -> Option<&HeroEffect> {
    let index = usize::try_from(effect).ok()?;
    state.player(state.active).hero.effects.get(index)
}

/// Position in the active player's hand of the card `card`, if it is there.
pub(crate) fn hand_position(state: &GameState, card: CardInstanceId) -> Option<usize> {
    state
        .player(state.active)
        .hand
        .iter()
        .position(|c| c.id == card)
}

/// Position in the active player's board of the creature `card`, if it is there.
pub(crate) fn board_position(state: &GameState, card: CardInstanceId) -> Option<usize> {
    state
        .player(state.active)
        .board
        .iter()
        .position(|c| c.id == card)
}

/// Position in the opposing player's board of the creature `card`, if it is there.
pub(crate) fn opposing_board_position(state: &GameState, card: CardInstanceId) -> Option<usize> {
    state
        .player(state.active.opponent())
        .board
        .iter()
        .position(|c| c.id == card)
}

/// Whether provocation lets the active player aim at `target`. Computed from the current
/// board every time (no cache: a remembered set would be a second state to invalidate): with
/// no opposing creature with provocation anything goes (the S2a behavior), otherwise only one
/// of those creatures. The single implementation of the filter, called by `effects::usable` alone.
pub(crate) fn provocation_allows(state: &GameState, target: Target) -> bool {
    let opposing = &state.player(state.active.opponent()).board;
    let mut provocateurs = opposing
        .iter()
        .filter(|c| effective(state, c).provocation)
        .peekable();
    if provocateurs.peek().is_none() {
        return true;
    }
    match target {
        Target::Hero => false,
        Target::Creature(id) => provocateurs.any(|c| c.id == id),
    }
}

/// Why `seat` cannot summon a creature right now, if it cannot: `None` once the game is over
/// (nothing is blocked by a rule of play then), `Some(BoardFull)` when the board limit is
/// reached. Read from the seat's public board only, never from a hand. The single answer,
/// called by `effects::usable` (the active seat) and by `player_view` (the hand of the seat asked).
pub(crate) fn play_blocker(state: &GameState, seat: PlayerId) -> Option<RuleError> {
    if state.winner.is_some() {
        return None;
    }
    // `>=`, not `==`: a state built by hand above the limit is still blocked.
    match state.rules.board_limit {
        Some(limit)
            if state.player(seat).board.len() >= usize::try_from(limit).unwrap_or(usize::MAX) =>
        {
            Some(RuleError::BoardFull)
        }
        _ => None,
    }
}

/// Why `card`, a card of `seat`'s hand, cannot be played right now, if it cannot: `None` once the
/// game is over, `Some(UnsupportedEffect)` for a spell (temporary, until #130), otherwise the
/// answer of [`play_blocker`] (`board_full`) for a unit. A spell takes no place on a board, so
/// `board_full` is never its reason. The single answer for a card of a hand, called by
/// `effects::usable`.
pub(crate) fn play_blocker_of(
    state: &GameState,
    seat: PlayerId,
    card: &CardInstance,
) -> Option<RuleError> {
    if state.winner.is_some() {
        return None;
    }
    if card.is_spell() {
        return Some(RuleError::UnsupportedEffect);
    }
    play_blocker(state, seat)
}

/// Why `card`, a creature of the active board, cannot attack right now, if it cannot: `None` once
/// the game is over (as [`play_blocker`]), `Some(AttackForceZero)` when its effective force is 0
/// (an immobile unit has none), read through `effective` alone. The single predicate of this
/// reason, called by `effects::usable` and, later, by the view; the caller already holds the
/// instance, so there is no second walk of the board.
pub(crate) fn attack_blocker(state: &GameState, card: &CardInstance) -> Option<RuleError> {
    if state.winner.is_some() {
        return None;
    }
    (effective(state, card).force == 0).then_some(RuleError::AttackForceZero)
}

/// The single legality check, shared by [`apply`] and [`legal_actions`]: the game must be
/// running, the picks bounded, then `effects::usable` (the one predicate) judges the use the
/// action expresses.
pub(crate) fn validate(state: &GameState, action: &Action) -> Result<(), RuleError> {
    if state.winner.is_some() {
        return Err(RuleError::GameOver);
    }
    // The size of the list first, before any read of the state: the same refusal whatever
    // card or source it names, so nothing about a hidden zone leaks.
    if picks_of(action).len() > MAX_PICKS {
        return Err(RuleError::TooManyPicks);
    }
    effects::usable(state, &effects::decode(action))
}

/// The picks an action carries: none for `Attack` and `Concede`.
fn picks_of(action: &Action) -> &[Pick] {
    match action {
        Action::Play { picks, .. } | Action::Activate { picks, .. } => picks,
        Action::Attack { .. } | Action::Concede => &[],
    }
}

/// Ends the game if the active player has no usable action other than conceding. A creature that
/// carries the attack always has one (an opposing provocateur if there is one, else the opposing
/// hero), so it keeps its owner in the game. Lazy: stops at the first usable action.
pub(crate) fn check_exhaustion(state: &mut GameState, events: &mut Vec<Event>) {
    let can_act = effects::sources(state).any(|used| effects::usable(state, &used).is_ok());
    if state.winner.is_none() && !can_act {
        let winner = state.active.opponent();
        state.winner = Some((winner, WinReason::Exhaustion));
        events.push(Event::GameWon {
            winner,
            reason: WinReason::Exhaustion,
        });
    }
}

/// Applies `action` for the active player. A refusal leaves the state (and its generator)
/// untouched: everything is validated before anything is mutated. Never consumes randomness.
pub fn apply(state: &mut GameState, action: Action) -> Result<Vec<Event>, RuleError> {
    validate(state, &action)?;
    let used = effects::decode(&action);
    let mut events = Vec::new();
    if let effects::Settled::Spent(cost) = effects::resolve(state, &used, &mut events) {
        pay(state, cost, &mut events);
        check_exhaustion(state, &mut events);
    }
    Ok(events)
}

/// Charges `cost` to the active player and moves the token accordingly.
fn pay(state: &mut GameState, cost: u32, events: &mut Vec<Event>) {
    let player = state.active;
    let my_max = state.player(player).hero.max_time;
    let opp_max = state.player(player.opponent()).hero.max_time;
    match time::spend(state.budget, cost, my_max, opp_max) {
        Spend::Stay(budget) => state.budget = budget,
        Spend::HandOver(budget) => {
            state.active = player.opponent();
            state.budget = budget;
        }
    }
    events.push(Event::TokenMoved {
        player: state.active,
        budget: state.budget,
    });
    if state.active != player {
        events.push(Event::ActivePlayerChanged {
            player: state.active,
        });
    }
}

/// Every action the active player may take: one `Play` per card in hand (none while the board is
/// full, `play_blocker`), hero effects in the order of the hero's list, then one
/// `Attack` per (attacker, valid target) pair, attackers in order of summoning and, for each, the opposing hero first and then every creature of the
/// opposing board in order (a dead creature is on no board, so it is neither listed as an
/// attacker nor as a target; own creatures are never targets), then `Concede`. While the
/// opponent has a creature with provocation, only those creatures are listed as targets (in
/// board order) and the hero is not: a provocation filters, it never adds an action. Empty once
/// the game is over. [`apply`] accepts exactly these actions, and `Concede`.
pub fn legal_actions(state: &GameState) -> Vec<Action> {
    if state.winner.is_some() {
        return Vec::new();
    }
    let mut actions: Vec<Action> = effects::sources(state)
        .filter(|used| effects::usable(state, used).is_ok())
        .filter_map(|used| effects::encode(&used))
        .collect();
    actions.push(Action::Concede);
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{draw, hero, potential, setup, stats_with};
    use crate::{CardInstanceId, new_game};
    use rand::{RngExt, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    /// A random game where about a third of the cards (by deck position, shuffled at setup)
    /// have provocation, on both sides.
    fn provocative_game(seed: u64) -> GameState {
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
                        0,
                    )
                })
                .collect::<Vec<_>>()
        };
        s.one.cards = cards(0).into_iter().map(Into::into).collect();
        s.two.cards = cards(1).into_iter().map(Into::into).collect();
        new_game(s, seed).unwrap().0
    }

    /// Every attacker of the active player against the hero, every id of the game (60 cards,
    /// on any zone or none) and an unknown id.
    fn every_attack(state: &GameState) -> Vec<Action> {
        let ids = (0..61).chain([10_000]).map(CardInstanceId);
        state
            .player(state.active)
            .board
            .iter()
            .flat_map(|attacker| {
                std::iter::once(Target::Hero)
                    .chain(ids.clone().map(Target::Creature))
                    .map(|target| Action::Attack {
                        attacker: attacker.id,
                        target,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn legal_actions_and_apply_agree_on_provocative_games_and_phi_always_falls() {
        let (mut attacks_on_provocateurs, mut forbidden_hero_attacks) = (0, 0);
        for seed in 0..120 {
            let mut state = provocative_game(seed);
            let mut policy = ChaCha8Rng::seed_from_u64(seed ^ 0xFACE);
            let mut steps = 0;
            while state.winner.is_none() {
                let legal = legal_actions(&state);
                for action in every_attack(&state) {
                    let mut clone = state.clone();
                    match apply(&mut clone, action.clone()) {
                        Ok(_) => assert!(
                            legal.contains(&action),
                            "seed {seed}: {action:?} applies but is not listed"
                        ),
                        Err(error) => {
                            assert!(
                                !legal.contains(&action),
                                "seed {seed}: {action:?} is listed but refused with {error:?}"
                            );
                            assert_eq!(clone, state, "seed {seed}: a refusal changed the state");
                            if error == RuleError::TargetForbiddenByProvocation
                                && matches!(
                                    action,
                                    Action::Attack {
                                        target: Target::Hero,
                                        ..
                                    }
                                )
                            {
                                forbidden_hero_attacks += 1;
                            }
                        }
                    }
                }
                let playable: Vec<Action> = legal
                    .iter()
                    .filter(|a| **a != Action::Concede)
                    .cloned()
                    .collect();
                // A creature that carries the attack always has a legal attack, even when only
                // the opposing provocateurs may be targeted: provocation never exhausts a player.
                for creature in &state.player(state.active).board {
                    assert!(
                        playable.iter().any(|a| matches!(
                            a,
                            Action::Attack { attacker, .. } if *attacker == creature.id
                        )),
                        "seed {seed}: {:?} carries the attack but has none legal",
                        creature.id
                    );
                }
                assert!(!playable.is_empty(), "seed {seed}: exhausted yet not over");
                let action = playable[policy.random_range(0..playable.len())].clone();
                if let Action::Attack {
                    target: Target::Creature(victim),
                    ..
                } = action
                    && state
                        .player(state.active.opponent())
                        .board
                        .iter()
                        .any(|c| c.id == victim && c.stats.provocation)
                {
                    attacks_on_provocateurs += 1;
                }
                let before = potential(&state);
                apply(&mut state, action.clone()).unwrap();
                assert!(
                    potential(&state) < before,
                    "seed {seed}: Φ did not fall on {action:?}"
                );
                steps += 1;
                assert!(steps < 2000, "seed {seed}: the game does not end");
            }
            let (_, reason) = state.winner.unwrap();
            assert_ne!(reason, WinReason::Exhaustion, "seed {seed}");
        }
        assert!(attacks_on_provocateurs > 100, "{attacks_on_provocateurs}");
        assert!(forbidden_hero_attacks > 100, "{forbidden_hero_attacks}");
    }
}

/// The two predicates `usable` reads for the new kinds (#208): `attack_blocker` and
/// `play_blocker_of`, on a mixed game of spells, immobile units and plain units.
#[cfg(test)]
mod predicate_tests {
    use super::*;
    use crate::effects::{Choice, EffectRef, Source, Use, usable};
    use crate::new_game;
    use crate::state::CardInstance;
    use crate::testing::{
        draw, hero, immobile, potential, setup, spell, stats_with, unit_with_effects,
    };
    use rand::{RngExt, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    /// A third spells, a third immobile units, a third units (a few with a declared effect),
    /// both sides, a board limit that sometimes binds.
    fn mixed_game(seed: u64) -> GameState {
        let mut s = setup(
            hero(1 + (seed % 2) as u32, 3, 8, vec![draw(1, 1), draw(4, 3)]),
            hero(1, 2, 6, vec![draw(2, 2)]),
        );
        s.rules.board_limit = Some(1 + (seed % 4) as u32);
        let cards = |offset: u32| {
            (0..30u32)
                .map(|n| {
                    let n = n + offset;
                    match n % 3 {
                        0 => spell(1 + n % 3),
                        1 => immobile(1 + n % 2, 1 + n % 4),
                        _ => {
                            let stats = stats_with(
                                1 + n % 3,
                                1 + n % 3,
                                1 + n % 2,
                                1 + n % 4,
                                n.is_multiple_of(4),
                                0,
                            );
                            if n.is_multiple_of(5) {
                                unit_with_effects(stats, vec![crate::testing::activated_draw()])
                            } else {
                                stats.into()
                            }
                        }
                    }
                })
                .collect::<Vec<_>>()
        };
        s.one.cards = cards(0);
        s.two.cards = cards(1);
        new_game(s, seed).unwrap().0
    }

    fn card_of(state: &GameState, id: CardInstanceId) -> &CardInstance {
        let side = state.player(state.active);
        side.hand
            .iter()
            .chain(&side.board)
            .find(|c| c.id == id)
            .unwrap()
    }

    #[test]
    fn attack_blocker_and_play_blocker_of_are_the_predicates_usable_uses() {
        let (mut plays, mut refused_plays, mut attacks, mut refused_attacks) = (0, 0, 0, 0);
        for seed in 0..60u64 {
            let mut state = mixed_game(seed);
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            for _ in 0..300 {
                if state.winner.is_some() {
                    break;
                }
                let active = state.active;
                for card in state.player(active).hand.clone() {
                    let used = Use::Effect {
                        source: Source::Hand(card.id),
                        effect: EffectRef::Play,
                        choice: Choice::Picks(&[]),
                    };
                    let expected = play_blocker_of(&state, active, &card).map_or(Ok(()), Err);
                    assert_eq!(usable(&state, &used), expected, "seed {seed}");
                    plays += 1;
                    refused_plays += usize::from(expected.is_err());
                }
                for attacker in state.player(active).board.clone() {
                    let targets = std::iter::once(Target::Hero).chain(
                        state
                            .player(active.opponent())
                            .board
                            .iter()
                            .map(|c| Target::Creature(c.id)),
                    );
                    for target in targets.collect::<Vec<_>>() {
                        let used = Use::Effect {
                            source: Source::Board(attacker.id),
                            effect: EffectRef::Attack,
                            choice: Choice::Target(target),
                        };
                        let expected = match attack_blocker(&state, card_of(&state, attacker.id)) {
                            Some(error) => Err(error),
                            None if provocation_allows(&state, target) => Ok(()),
                            None => Err(RuleError::TargetForbiddenByProvocation),
                        };
                        assert_eq!(usable(&state, &used), expected, "seed {seed}");
                        attacks += 1;
                        refused_attacks += usize::from(expected.is_err());
                    }
                }
                let actions = legal_actions(&state);
                let action = actions[rng.random_range(0..actions.len())].clone();
                if action == Action::Concede {
                    break;
                }
                apply(&mut state, action).unwrap();
            }
        }
        // The mixed game exercises both answers of both predicates.
        assert!(plays > refused_plays && refused_plays > 0);
        assert!(attacks > refused_attacks && refused_attacks > 0);
    }

    #[test]
    fn the_predicates_say_nothing_once_the_game_is_over() {
        let mut state = mixed_game(1);
        let spell_in_hand = state
            .player(state.active)
            .hand
            .iter()
            .find(|c| c.is_spell())
            .cloned()
            .expect("a spell in the opening hand");
        assert_eq!(
            play_blocker_of(&state, state.active, &spell_in_hand),
            Some(RuleError::UnsupportedEffect)
        );
        state.winner = Some((state.active, WinReason::Concede));
        assert_eq!(play_blocker_of(&state, state.active, &spell_in_hand), None);
    }

    #[test]
    fn attack_blocker_is_the_zero_force_boundary() {
        let mut state = mixed_game(2);
        let mut card = state.player(state.active).hand[0].clone();
        card.extras = None;
        card.stats.force = 1;
        assert_eq!(attack_blocker(&state, &card), None);
        card.stats.force = 0;
        assert_eq!(
            attack_blocker(&state, &card),
            Some(RuleError::AttackForceZero)
        );
        state.winner = Some((state.active, WinReason::Concede));
        assert_eq!(attack_blocker(&state, &card), None);
    }

    #[test]
    fn a_spell_costs_a_hand_slot_but_no_board_slot() {
        // `board_limit` 1 with a full board: a unit is `board_full`, a spell is not blocked by
        // the board (it is blocked as a spell, `unsupported_effect`, never `board_full`).
        let mut state = mixed_game(4);
        state.rules.board_limit = Some(1);
        let active = state.active;
        let taken = state
            .player_mut(active)
            .hand
            .iter()
            .position(|c| !c.is_spell())
            .unwrap();
        let unit = state.player_mut(active).hand.remove(taken);
        state.player_mut(active).board.push(unit);
        let hand = state.player(active).hand.clone();
        let spell_card = hand.iter().find(|c| c.is_spell()).unwrap();
        let unit_card = hand.iter().find(|c| !c.is_spell()).unwrap();
        assert_eq!(
            play_blocker_of(&state, active, spell_card),
            Some(RuleError::UnsupportedEffect)
        );
        assert_eq!(
            play_blocker_of(&state, active, unit_card),
            Some(RuleError::BoardFull)
        );
    }

    #[test]
    fn phi_still_falls_with_the_new_kinds() {
        let mut spells_seen = false;
        for seed in 0..250u64 {
            let mut state = mixed_game(seed);
            let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xABCD);
            let mut phi = potential(&state);
            for _ in 0..5_000 {
                if state.winner.is_some() {
                    break;
                }
                let actions = legal_actions(&state);
                let playable: Vec<_> = actions.iter().filter(|a| **a != Action::Concede).collect();
                if playable.is_empty() {
                    break;
                }
                let action = playable[rng.random_range(0..playable.len())].clone();
                apply(&mut state, action).unwrap();
                let now = potential(&state);
                assert!(now < phi, "seed {seed}: phi {phi} -> {now}");
                phi = now;
            }
            spells_seen |= state
                .player(PlayerId::One)
                .hand
                .iter()
                .any(|c| c.is_spell());
            assert!(state.winner.is_some(), "seed {seed}: the game terminates");
        }
        assert!(spells_seen, "spells were part of the games");
    }
}
