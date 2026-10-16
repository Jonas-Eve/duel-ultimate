//! Dry-run preview of the legal actions (stub: filled in by the implementation commit).

use crate::rules::{Action, legal_actions};
use crate::state::GameState;
use crate::view::GameResult;
use serde::Serialize;

/// What one legal action would cost and cause, computed by the engine.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActionPreview {
    pub action: Action,
    pub cost: Option<u32>,
    pub hands_over: bool,
    pub budget_after: Option<u32>,
    pub ends_game: Option<GameResult>,
}

/// One preview per legal action, in the order of [`legal_actions`].
pub fn preview_legal_actions(state: &GameState) -> Vec<ActionPreview> {
    legal_actions(state)
        .into_iter()
        .map(|action| ActionPreview {
            action,
            cost: None,
            hands_over: false,
            budget_after: None,
            ends_game: None,
        })
        .collect()
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
