//! Resolution of an attack's damage: one isolated step, so a future rule that orders or
//! modifies damage (strike first, ...) changes this function and nothing else.
//!
//! # Termination
//!
//! Every game ends. Let `Φ` be the sum of the heroes' health plus, for every card, its
//! current health (`stats.health` of its instance, lowered by damage) and a bonus of 2 while
//! in a pile, 1 while in a hand, 0 on a board and, in a discard pile, nothing at all (a
//! dead creature has 0 health). Φ is a non-negative integer and every action strictly
//! decreases it:
//!
//! - a draw of `n` cards moves `n >= 1` cards from a pile to a hand: −`n`;
//! - a summoning moves one card from a hand to a board: −1;
//! - an attack removes `min(force, remaining health of the target) >= 1` from a hero or a
//!   creature (the excess damage is lost, so it removes nothing more), and burying a dead
//!   creature removes nothing, its health being 0.
//!
//! (The first potential of the Epic, `2·|piles| + |hands| + Σ health`, broke once creatures
//! counted: a summoning added the creature's health, and "−force" ignored the lost excess.)
//! This relies on `force >= 1`, `health >= 1`, every cost `>= 1` and every draw `count >= 1`,
//! all guaranteed by the validation `new_game` replays (`duel-cards`), and on there being no
//! healing and no card leaving a discard pile or returning to a pile, a hand or a board. Any
//! rule adding one of those must re-examine this.

use crate::rules::{Event, Target};
use crate::state::{CardInstance, CardInstanceId, GameState, PlayerId};

/// Applies the damage of `attacker` (on `player`'s board) to `target`, saturating at 0
/// health, and logs it. The caller has validated that the attacker is on that board.
pub(crate) fn resolve_damage(
    state: &mut GameState,
    player: PlayerId,
    attacker: CardInstanceId,
    target: Target,
    events: &mut Vec<Event>,
) {
    let force = state
        .player(player)
        .board
        .iter()
        .find(|c| c.id == attacker)
        .map_or(0, |c| c.stats.force);
    debug_assert!(force >= 1, "validated attacker with force >= 1");
    match target {
        Target::Hero => {
            let victim = player.opponent();
            let side = state.player_mut(victim);
            side.health = side.health.saturating_sub(force);
            events.push(Event::Damaged {
                player: victim,
                amount: force,
            });
        }
        Target::Creature(creature) => {
            let victim = player.opponent();
            let Some(target) = state
                .player_mut(victim)
                .board
                .iter_mut()
                .find(|c| c.id == creature)
            else {
                debug_assert!(false, "validated target on the opposing board");
                return;
            };
            target.stats.health = target.stats.health.saturating_sub(force);
            events.push(Event::CreatureDamaged {
                player: victim,
                creature,
                amount: force,
            });
        }
    }
}

/// Moves every creature at 0 health from its board to its owner's discard pile and logs a
/// `Died` for each, in ascending instance id order whatever the board or the side (the
/// order of death when several die at once). Surviving creatures keep their board order.
pub(crate) fn bury_dead(state: &mut GameState, events: &mut Vec<Event>) {
    let mut dead: Vec<(CardInstance, PlayerId)> = Vec::new();
    for player in [PlayerId::One, PlayerId::Two] {
        let board = std::mem::take(&mut state.player_mut(player).board);
        let (fallen, alive): (Vec<_>, Vec<_>) =
            board.into_iter().partition(|c| c.stats.health == 0);
        state.player_mut(player).board = alive;
        dead.extend(fallen.into_iter().map(|c| (c, player)));
    }
    dead.sort_by_key(|(card, _)| card.id);
    for (card, player) in dead {
        events.push(Event::Died {
            player,
            creature: card.id,
        });
        state.player_mut(player).discard.push(card);
    }
}
