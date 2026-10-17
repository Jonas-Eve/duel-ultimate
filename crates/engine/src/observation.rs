//! What a decision maker (an AI policy) is allowed to see of a running game: the active seat,
//! its legal actions other than `Concede`, and its own [`PlayerView`] on demand.
//!
//! Same principle as [`OwnSide`](crate::OwnSide) / [`OpponentSide`](crate::OpponentSide): the
//! signature is the guarantee. [`Observation`] has private fields, no `Deref`, no accessor to
//! the [`GameState`] and no way to clone it, so a function that only receives an observation
//! cannot reach the opposing hand, either pile's order or the generator. It does not
//! authenticate anything: the caller decides whose turn it observes (always the active seat).

use crate::rules::{Action, legal_actions};
use crate::state::{GameState, PlayerId};
use crate::view::{PlayerView, player_view};

/// The active seat's window on a running game. Built by [`Observation::of`], lazily: only the
/// legal action list is computed at construction (once); the view is computed when asked for.
#[derive(Debug)]
pub struct Observation<'a> {
    state: &'a GameState,
    actions: Vec<Action>,
}

impl<'a> Observation<'a> {
    /// The observation of the active seat of `state`, or `None` once the game is over. The
    /// actions are [`legal_actions`] without `Concede`, in the same order; they may be empty
    /// (nothing but conceding is playable), which is not a `None`.
    pub fn of(state: &'a GameState) -> Option<Observation<'a>> {
        if state.winner().is_some() {
            return None;
        }
        let actions = legal_actions(state)
            .into_iter()
            .filter(|action| *action != Action::Concede)
            .collect();
        Some(Observation { state, actions })
    }

    /// The seat that acts: the active player.
    pub fn seat(&self) -> PlayerId {
        self.state.active()
    }

    /// The legal actions of the seat other than `Concede`, in the engine's order.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// The seat's own view of the game (its hand in full, the opponent's hand and both piles
    /// as counts), computed on each call.
    pub fn view(&self) -> PlayerView {
        player_view(self.state, self.seat())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{draw, hero, setup};
    use crate::{WinReason, new_game};

    #[test]
    fn an_observation_holds_nothing_but_the_state_reference_and_the_action_list() {
        // No cached view, preview or copy of the state: the construction stays cheap.
        assert_eq!(
            size_of::<Observation<'static>>(),
            size_of::<&GameState>() + size_of::<Vec<Action>>()
        );
    }

    #[test]
    fn a_seat_with_nothing_playable_but_conceding_has_an_empty_list_not_a_none() {
        let (mut state, _) = new_game(setup(hero(2, 3, 8, vec![]), hero(1, 2, 6, vec![])), 1)
            .expect("the setup is valid");
        // Not reachable through `apply` (the engine ends the game by exhaustion first), so the
        // state is built by hand.
        let active = state.active();
        let side = match active {
            PlayerId::One => &mut state.one,
            PlayerId::Two => &mut state.two,
        };
        side.hand.clear();
        side.board.clear();
        assert_eq!(legal_actions(&state), vec![Action::Concede]);
        let obs = Observation::of(&state).expect("the game runs");
        assert!(obs.actions().is_empty());
    }

    #[test]
    fn a_finished_game_has_no_observation() {
        let (mut state, _) = new_game(
            setup(hero(2, 3, 8, vec![draw(1, 1)]), hero(1, 2, 6, vec![])),
            1,
        )
        .expect("the setup is valid");
        state.winner = Some((PlayerId::One, WinReason::Concede));
        assert!(Observation::of(&state).is_none());
    }
}
