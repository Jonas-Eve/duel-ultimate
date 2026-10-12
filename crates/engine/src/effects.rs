//! Resolution of hero effects: one isolated step, so new effects are new variants.

use crate::rules::Event;
use crate::state::{GameState, PlayerId};
use duel_cards::Effect;

/// Whether `effect` can currently be activated by `player`.
pub(crate) fn applicable(_state: &GameState, _player: PlayerId, _effect: &Effect) -> bool {
    todo!()
}

/// Applies `effect` for `player`, appending what happened to `events`. The caller has
/// already checked [`applicable`].
pub(crate) fn resolve(
    _state: &mut GameState,
    _player: PlayerId,
    _effect: &Effect,
    _events: &mut Vec<Event>,
) {
    todo!()
}
