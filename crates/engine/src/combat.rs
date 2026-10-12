//! Resolution of an attack's damage: one isolated step, so a future rule that orders or
//! modifies damage (strike first, ...) changes this function and nothing else.
//!
//! # Termination
//!
//! Every game ends. With `Φ = 2·|piles| + |hands| + Σ health` (heroes and, later, living
//! creatures), a non-negative integer, every action strictly decreases Φ: a draw of `n`
//! cards removes `n >= 1`, a summoning removes 1, and an attack removes `force >= 1` from a
//! hero that has `health >= 1` while the game runs. That relies on `force >= 1`,
//! `health >= 1`, every cost `>= 1` and every draw `count >= 1`, all guaranteed by the
//! validation `new_game` replays (`duel-cards`), and on there being no healing and no card
//! returning to a pile or hand. Any rule adding one of those must re-examine this.

use crate::rules::{Event, Target};
use crate::state::{CardInstanceId, GameState, PlayerId};

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
    }
}
