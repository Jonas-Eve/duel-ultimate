//! Actions, events and the rules that apply them.

use crate::combat;
use crate::effects;
use crate::state::{CardInstanceId, GameState, PlayerId, WinReason};
use crate::time::{self, Spend};
use duel_cards::HeroEffect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Summons a creature of the active player's hand (untrusted id, validated by the engine).
    Play {
        card: CardInstanceId,
    },
    /// Activates the active hero's effect at this index of its list (untrusted input).
    Activate {
        effect: u32,
    },
    /// Attacks with a creature of the active player's board (untrusted id, validated).
    Attack {
        attacker: CardInstanceId,
        target: Target,
    },
    Concede,
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
    GameStarted {
        first: PlayerId,
        budget: u32,
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
        }
    }
}

/// The effect of the active hero at the untrusted index `effect`, if there is one.
fn hero_effect(state: &GameState, effect: u32) -> Option<&HeroEffect> {
    let index = usize::try_from(effect).ok()?;
    state.player(state.active).hero.effects.get(index)
}

/// Position in the active player's hand of the card `card`, if it is there.
fn hand_position(state: &GameState, card: CardInstanceId) -> Option<usize> {
    state
        .player(state.active)
        .hand
        .iter()
        .position(|c| c.id == card)
}

/// Position in the active player's board of the creature `card`, if it is there.
fn board_position(state: &GameState, card: CardInstanceId) -> Option<usize> {
    state
        .player(state.active)
        .board
        .iter()
        .position(|c| c.id == card)
}

/// Position in the opposing player's board of the creature `card`, if it is there.
fn opposing_board_position(state: &GameState, card: CardInstanceId) -> Option<usize> {
    state
        .player(state.active.opponent())
        .board
        .iter()
        .position(|c| c.id == card)
}

/// Whether provocation lets the active player aim at `target`. Computed from the current
/// board every time (no cache: a remembered set would be a second state to invalidate): with
/// no opposing creature with provocation anything goes (the S2a behavior), otherwise only one
/// of those creatures. The single implementation of the filter, called by [`validate`] alone.
fn provocation_allows(state: &GameState, target: Target) -> bool {
    let opposing = &state.player(state.active.opponent()).board;
    let mut provocateurs = opposing.iter().filter(|c| c.stats.provocation).peekable();
    if provocateurs.peek().is_none() {
        return true;
    }
    match target {
        Target::Hero => false,
        Target::Creature(id) => provocateurs.any(|c| c.id == id),
    }
}

/// The single legality check, shared by [`apply`] and [`legal_actions`].
pub(crate) fn validate(state: &GameState, action: &Action) -> Result<(), RuleError> {
    if state.winner.is_some() {
        return Err(RuleError::GameOver);
    }
    match action {
        Action::Concede => Ok(()),
        Action::Play { card } => hand_position(state, *card)
            .map(|_| ())
            .ok_or(RuleError::CardNotInHand),
        Action::Attack { attacker, target } => {
            // Order of the checks: the attacker, then the target on the opposing board,
            // then the provocation. Every id that is not an opposing creature gets
            // `TargetNotOnBoard` whatever zone (hidden or not) it is in, even when a
            // provocateur exists: the provocation error only ever speaks of the public board.
            board_position(state, *attacker).ok_or(RuleError::AttackerNotOnBoard)?;
            if let Target::Creature(creature) = target {
                opposing_board_position(state, *creature).ok_or(RuleError::TargetNotOnBoard)?;
            }
            if provocation_allows(state, *target) {
                Ok(())
            } else {
                Err(RuleError::TargetForbiddenByProvocation)
            }
        }
        Action::Activate { effect } => {
            let hero_effect = hero_effect(state, *effect).ok_or(RuleError::UnknownEffect)?;
            if effects::applicable(state, state.active, &hero_effect.effect) {
                Ok(())
            } else {
                Err(RuleError::EffectNotApplicable)
            }
        }
    }
}

/// Every action worth testing for legality, `Concede` excluded, in the order of
/// [`legal_actions`]: the hand, hero effects, then per attacker (board order) the opposing
/// hero first and each creature of the opposing board in order. At most
/// `board x (1 + opposing board)` attacks. Own creatures, hand, pile and discard are never
/// candidates; `validate` stays the single judge of what is legal, provocation included (a
/// provocation hides the hero and the other creatures by refusing them there, not by a
/// second filter here).
fn candidates(state: &GameState) -> Vec<Action> {
    let side = state.player(state.active);
    let opposing = state.player(state.active.opponent());
    let plays = side.hand.iter().map(|c| Action::Play { card: c.id });
    let effects = (0..side.hero.effects.len())
        .filter_map(|i| u32::try_from(i).ok())
        .map(|effect| Action::Activate { effect });
    let attacks = side.board.iter().flat_map(|attacker| {
        std::iter::once(Target::Hero)
            .chain(opposing.board.iter().map(|c| Target::Creature(c.id)))
            .map(|target| Action::Attack {
                attacker: attacker.id,
                target,
            })
    });
    plays.chain(effects).chain(attacks).collect()
}

/// The legal actions other than `Concede`.
fn playable(state: &GameState) -> Vec<Action> {
    candidates(state)
        .into_iter()
        .filter(|action| validate(state, action).is_ok())
        .collect()
}

/// Ends the game if the active player has no legal action other than conceding. A living
/// creature always has one (an opposing provocateur if there is one, else the opposing hero),
/// so it keeps its owner in the game.
pub(crate) fn check_exhaustion(state: &mut GameState, events: &mut Vec<Event>) {
    let can_act = candidates(state)
        .iter()
        .any(|action| validate(state, action).is_ok());
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
    let player = state.active;
    match action {
        Action::Concede => {
            let winner = player.opponent();
            state.winner = Some((winner, WinReason::Concede));
            Ok(vec![Event::GameWon {
                winner,
                reason: WinReason::Concede,
            }])
        }
        Action::Attack { attacker, target } => {
            let index = board_position(state, attacker).ok_or(RuleError::AttackerNotOnBoard)?;
            let cost = state
                .player(player)
                .board
                .get(index)
                .map(|c| c.stats.attack_cost)
                .ok_or(RuleError::AttackerNotOnBoard)?;
            let mut events = vec![Event::Attacked {
                player,
                attacker,
                cost,
            }];
            combat::resolve_damage(state, player, attacker, target, &mut events);
            combat::bury_dead(state, &mut events);
            let victim = player.opponent();
            if state.player(victim).health == 0 {
                // The game stops here: no time is spent, the token does not move and the
                // budget stays within 1..=max_time of the (unchanged) active player.
                state.winner = Some((player, WinReason::HeroDown));
                events.push(Event::GameWon {
                    winner: player,
                    reason: WinReason::HeroDown,
                });
                return Ok(events);
            }
            pay(state, cost, &mut events);
            check_exhaustion(state, &mut events);
            Ok(events)
        }
        Action::Play { card } => {
            let index = hand_position(state, card).ok_or(RuleError::CardNotInHand)?;
            let creature = state.player_mut(player).hand.remove(index);
            let cost = creature.stats.cost;
            state.player_mut(player).board.push(creature);
            let mut events = vec![Event::Played { player, card, cost }];
            pay(state, cost, &mut events);
            check_exhaustion(state, &mut events);
            Ok(events)
        }
        Action::Activate { effect } => {
            let hero_effect = hero_effect(state, effect)
                .cloned()
                .ok_or(RuleError::UnknownEffect)?;
            let mut events = vec![Event::EffectActivated {
                player,
                effect,
                cost: hero_effect.cost,
            }];
            effects::resolve(state, player, &hero_effect.effect, &mut events);
            pay(state, hero_effect.cost, &mut events);
            check_exhaustion(state, &mut events);
            Ok(events)
        }
    }
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

/// Every action the active player may take: one `Play` per card in hand, hero effects in the
/// order of the hero's list, then one `Attack` per (attacker, valid target) pair, attackers in
/// order of summoning and, for each, the opposing hero first and then every creature of the
/// opposing board in order (a dead creature is on no board, so it is neither listed as an
/// attacker nor as a target; own creatures are never targets), then `Concede`. While the
/// opponent has a creature with provocation, only those creatures are listed as targets (in
/// board order) and the hero is not: a provocation filters, it never adds an action. Empty once
/// the game is over. [`apply`] accepts exactly these actions, and `Concede`.
pub fn legal_actions(state: &GameState) -> Vec<Action> {
    if state.winner.is_some() {
        return Vec::new();
    }
    let mut actions = playable(state);
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
        s.one.cards = cards(0);
        s.two.cards = cards(1);
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
                    match apply(&mut clone, action) {
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
                    .copied()
                    .filter(|a| *a != Action::Concede)
                    .collect();
                // A living creature always has a legal attack, even when only the opposing
                // provocateurs may be targeted: provocation never exhausts a player.
                if !state.player(state.active).board.is_empty() {
                    assert!(
                        playable.iter().any(|a| matches!(a, Action::Attack { .. })),
                        "seed {seed}: a creature but no attack is legal"
                    );
                }
                assert!(!playable.is_empty(), "seed {seed}: exhausted yet not over");
                let action = playable[policy.random_range(0..playable.len())];
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
                apply(&mut state, action).unwrap();
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
