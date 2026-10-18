//! Frozen oracle of #128: removed by task 7, modified only to compile.
//!
//! A verbatim copy (moved by copy-paste, not rewritten) of the rules of `rules.rs` as of
//! `main` at 6baf8ba (the merge of #181, after #180's `picks`/`source` checks): `validate`,
//! `candidates`, `playable`, `legal_actions`, `check_exhaustion`, `apply` and the helpers they
//! depend on (`hero_effect`, `hand_position`, `board_position`, `opposing_board_position`,
//! `provocation_allows`, `play_blocker`, `picks_of`, `pay`, and its own `effects::applicable`
//! / `effects::resolve`). Only `combat::{plan_damage, apply_damage, resolve_damage,
//! bury_dead}`, `time::spend` and the state, `Action`, `Event` and `RuleError` types stay
//! shared with production. Nothing in production calls this module; it exists for the
//! differential tests below, and goes away with them.

use crate::combat;
use crate::rules::{Action, Event, MAX_PICKS, Pick, RuleError, Target};
use crate::state::{CardInstanceId, GameState, PlayerId, WinReason};
use crate::time::{self, Spend};
use duel_cards::HeroEffect;

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

/// Why `seat` cannot summon a creature right now, if it cannot: `None` once the game is over
/// (nothing is blocked by a rule of play then), `Some(BoardFull)` when the board limit is
/// reached. Read from the seat's public board only, never from a hand. The single answer,
/// called by [`validate`] (the active seat) and by `player_view` (the hand of the seat asked).
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

/// The single legality check, shared by [`apply`] and [`legal_actions`].
pub(crate) fn validate(state: &GameState, action: &Action) -> Result<(), RuleError> {
    if state.winner.is_some() {
        return Err(RuleError::GameOver);
    }
    // The size of the list first, before any read of the state: the same refusal whatever
    // card or source it names, so nothing about a hidden zone leaks.
    if picks_of(action).len() > MAX_PICKS {
        return Err(RuleError::TooManyPicks);
    }
    match action {
        Action::Concede => Ok(()),
        Action::Play { card, picks } => {
            // The hand first: an id that is not in it gets `CardNotInHand` whatever the board.
            hand_position(state, *card).ok_or(RuleError::CardNotInHand)?;
            // No summoning takes a choice at this stage.
            if !picks.is_empty() {
                return Err(RuleError::InvalidPicks);
            }
            play_blocker(state, state.active).map_or(Ok(()), Err)
        }
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
        Action::Activate {
            effect,
            source: None,
            picks,
        } => {
            let hero_effect = hero_effect(state, *effect).ok_or(RuleError::UnknownEffect)?;
            // No hero effect takes a choice at this stage.
            if !picks.is_empty() {
                return Err(RuleError::InvalidPicks);
            }
            if effects::applicable(state, state.active, &hero_effect.effect) {
                Ok(())
            } else {
                Err(RuleError::EffectNotApplicable)
            }
        }
        Action::Activate {
            source: Some(source),
            ..
        } => {
            // One code for every id that is not a creature of my board (hand, pile, discard,
            // opposing board, unknown): no hidden zone is told apart.
            board_position(state, *source).ok_or(RuleError::SourceNotOnBoard)?;
            // A creature declares no effect at this stage.
            Err(RuleError::UnknownEffect)
        }
    }
}

/// The picks an action carries: none for `Attack` and `Concede`.
fn picks_of(action: &Action) -> &[Pick] {
    match action {
        Action::Play { picks, .. } | Action::Activate { picks, .. } => picks,
        Action::Attack { .. } | Action::Concede => &[],
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
    let plays = side.hand.iter().map(|c| Action::play(c.id));
    let effects = (0..side.hero.effects.len())
        .filter_map(|i| u32::try_from(i).ok())
        .map(Action::activate);
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
        Action::Play { card, .. } => {
            let index = hand_position(state, card).ok_or(RuleError::CardNotInHand)?;
            let creature = state.player_mut(player).hand.remove(index);
            let cost = creature.stats.cost;
            state.player_mut(player).board.push(creature);
            let mut events = vec![Event::Played { player, card, cost }];
            pay(state, cost, &mut events);
            check_exhaustion(state, &mut events);
            Ok(events)
        }
        Action::Activate { effect, .. } => {
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
    let mut actions = playable(state);
    actions.push(Action::Concede);
    actions
}

/// The frozen copy of `effects.rs` (hero-effect resolution), verbatim apart from its place.
mod effects {
    use super::Event;
    use crate::state::{GameState, PlayerId};
    use duel_cards::Effect;

    /// Whether `effect` can currently be activated by `player`.
    pub(crate) fn applicable(state: &GameState, player: PlayerId, effect: &Effect) -> bool {
        match effect {
            Effect::Draw { .. } => !state.player(player).deck.is_empty(),
        }
    }

    /// Applies `effect` for `player`, appending what happened to `events`. The caller has
    /// already checked [`applicable`].
    pub(crate) fn resolve(
        state: &mut GameState,
        player: PlayerId,
        effect: &Effect,
        events: &mut Vec<Event>,
    ) {
        match effect {
            Effect::Draw { count } => {
                // Draws what is left when the pile is shorter than `count`.
                let wanted = usize::try_from(*count).unwrap_or(usize::MAX);
                let side = state.player_mut(player);
                for _ in 0..wanted {
                    let Some(card) = side.deck.pop() else { break };
                    events.push(Event::Drew {
                        player,
                        card: card.id,
                    });
                    side.hand.push(card);
                }
            }
        }
    }
}
