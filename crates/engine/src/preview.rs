//! Dry-run preview of the legal actions: for each one, what it costs and what it causes,
//! computed by the engine itself and never by a client.
//!
//! The time rule stays in `time.rs` (reached through [`apply`]): the preview replays every
//! legal action on a clone of the state and reads the summary from the events produced. A
//! direct recomputation of the cost would duplicate the cost reading done inline by `apply`
//! and miss two real cases: a lethal attack on a hero spends no time and hands nothing over,
//! and a handover can end the game by exhaustion. The clone keeps the real state and its
//! generator untouched (`apply` never consumes the generator, so cloning it is enough).
//!
//! Only five fields come out. What a draw brings (the `drew` events) is hidden information and
//! is never copied: a draw of several cards changes only `cost` and the handover.

use crate::rules::{Action, Event, apply, legal_actions};
use crate::state::GameState;
use crate::view::GameResult;
use serde::Serialize;

/// What one legal action would cost and cause.
///
/// - `cost`: the `cost` its events report (`Played`, `EffectActivated`, `Attacked`), `None` for
///   `Concede`. A lethal attack on the hero shows its attack cost although no time is spent.
/// - `hands_over`: the token changes sides (an `ActivePlayerChanged` event).
/// - `budget_after`: the `budget` of the token move (of the player who keeps the token or of
///   the one who receives it), `None` when there is none (the game ends without one).
/// - `ends_game`: how the game ends, if this action ends it. Combine it with `hands_over` for
///   display: a lethal attack ends the game without handing over; a handover can end the game
///   by exhaustion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActionPreview {
    pub action: Action,
    pub cost: Option<u32>,
    pub hands_over: bool,
    pub budget_after: Option<u32>,
    pub ends_game: Option<GameResult>,
}

/// One preview per legal action, in the order of [`legal_actions`] (same length, `Concede`
/// last); empty once the game is over. One clone per legal action, so the work is bounded by
/// the size of `legal_actions` (931 at most in practice, 30 creatures against 30).
///
/// Should `apply` ever refuse a legal action on the clone (an internal inconsistency that must
/// not happen), the action is kept, with no cost and no consequence, instead of being silently
/// omitted: the differential tests then catch it.
pub fn preview_legal_actions(state: &GameState) -> Vec<ActionPreview> {
    legal_actions(state)
        .into_iter()
        .map(|action| {
            let mut copy = state.clone();
            let summary = apply(&mut copy, action)
                .map(|events| summarize(&events))
                .unwrap_or_default();
            ActionPreview {
                action,
                cost: summary.cost,
                hands_over: summary.hands_over,
                budget_after: summary.budget_after,
                ends_game: summary.ends_game,
            }
        })
        .collect()
}

#[derive(Default)]
struct Summary {
    cost: Option<u32>,
    hands_over: bool,
    budget_after: Option<u32>,
    ends_game: Option<GameResult>,
}

/// Reads the preview's facts from the events of one action. The `match` has no `_` arm on
/// purpose: a new `Event` variant must be reviewed here, not silently ignored.
fn summarize(events: &[Event]) -> Summary {
    let mut summary = Summary::default();
    for event in events {
        match event {
            Event::Played { cost, .. }
            | Event::EffectActivated { cost, .. }
            | Event::Attacked { cost, .. } => summary.cost = Some(*cost),
            Event::TokenMoved { budget, .. } => summary.budget_after = Some(*budget),
            Event::ActivePlayerChanged { .. } => summary.hands_over = true,
            Event::GameWon { winner, reason } => {
                summary.ends_game = Some(GameResult {
                    winner: *winner,
                    reason: *reason,
                });
            }
            // Hidden or irrelevant to the time rule: never copied into the preview.
            Event::GameStarted { .. }
            | Event::Drew { .. }
            | Event::Damaged { .. }
            | Event::CreatureDamaged { .. }
            | Event::Riposted { .. }
            | Event::Died { .. } => {}
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_game;
    use crate::rules::apply;
    use crate::testing::{hero, setup};

    /// 30 creatures against 30: the largest legal-action list (30 x 31 attacks + Concede).
    #[test]
    fn the_worst_case_of_931_legal_actions_is_previewed_in_full() {
        let (mut state, _) =
            new_game(setup(hero(2, 8, 8, vec![]), hero(1, 2, 8, vec![])), 3).unwrap();
        for side in [&mut state.one, &mut state.two] {
            side.board.append(&mut side.deck);
            side.board.append(&mut side.hand);
        }
        assert_eq!(state.one.board.len(), 30);
        assert_eq!(state.two.board.len(), 30);
        let before = state.clone();
        let legal = legal_actions(&state);
        assert_eq!(legal.len(), 931);

        let started = std::time::Instant::now();
        let previews = preview_legal_actions(&state);
        // Generous bound: measured at tens of milliseconds in debug; this only guards against
        // an accidental quadratic blow-up.
        assert!(started.elapsed().as_secs() < 10, "{:?}", started.elapsed());

        assert_eq!(state, before);
        assert_eq!(previews.len(), 931);
        for (preview, action) in previews.iter().zip(&legal) {
            assert_eq!(preview.action, *action);
            let mut copy = state.clone();
            let events = apply(&mut copy, *action).unwrap();
            let expected_cost = events.iter().find_map(|e| match e {
                crate::Event::Attacked { cost, .. } => Some(*cost),
                _ => None,
            });
            assert_eq!(preview.cost, expected_cost, "{preview:?}");
            assert_eq!(preview.hands_over, copy.active() != state.active());
        }
    }
}
