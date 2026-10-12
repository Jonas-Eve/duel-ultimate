//! Resolution of hero effects: one isolated step, so new effects are new variants.

use crate::rules::Event;
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
