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
            // Order of the checks: the attacker, then the target. Every refused target
            // gets the same error, whatever zone (hidden or not) the id is in.
            board_position(state, *attacker).ok_or(RuleError::AttackerNotOnBoard)?;
            match target {
                Target::Hero => Ok(()),
                Target::Creature(creature) => opposing_board_position(state, *creature)
                    .map(|_| ())
                    .ok_or(RuleError::TargetNotOnBoard),
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
/// [`legal_actions`]: the hand, then hero effects. Later tasks extend it (attacks), and
/// exhaustion follows automatically.
fn candidates(state: &GameState) -> Vec<Action> {
    let side = state.player(state.active);
    let plays = side.hand.iter().map(|c| Action::Play { card: c.id });
    let effects = (0..side.hero.effects.len())
        .filter_map(|i| u32::try_from(i).ok())
        .map(|effect| Action::Activate { effect });
    let attacks = side.board.iter().map(|c| Action::Attack {
        attacker: c.id,
        target: Target::Hero,
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

/// Ends the game if the active player has no legal action other than conceding.
pub(crate) fn check_exhaustion(state: &mut GameState, events: &mut Vec<Event>) {
    if state.winner.is_none() && playable(state).is_empty() {
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
/// order of the hero's list, one `Attack` (on the hero) per creature of the board in order of
/// summoning, then `Concede`. Empty once the game is over.
///
/// Transitional: [`apply`] already accepts an attack on a creature of the opposing board,
/// which this list does not offer yet (a later task of the same story adds those attacks).
pub fn legal_actions(state: &GameState) -> Vec<Action> {
    if state.winner.is_some() {
        return Vec::new();
    }
    let mut actions = playable(state);
    actions.push(Action::Concede);
    actions
}
