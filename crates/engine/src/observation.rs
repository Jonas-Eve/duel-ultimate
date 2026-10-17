//! What a decision maker (an AI policy) is allowed to see of a running game: the active seat,
//! its legal actions other than `Concede`, and its own [`PlayerView`] on demand.
//!
//! Same principle as [`OwnSide`](crate::OwnSide) / [`OpponentSide`](crate::OpponentSide): the
//! signature is the guarantee. [`Observation`] has private fields, no `Deref`, no accessor to
//! the [`GameState`] and no way to clone it, so a function that only receives an observation
//! cannot reach the opposing hand, either pile's order or the generator. It does not
//! authenticate anything: the caller decides whose turn it observes (always the active seat).

use crate::preview::{ActionPreview, dry_run, preview_of};
use crate::rules::{Action, Event, legal_actions};
use crate::state::{CardInstanceId, GameState, PlayerId};
use crate::view::{OutcomeView, PlayerView, outcome_view, player_view};

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

    /// The preview of the legal action at index `i` of [`actions`](Self::actions) (what it costs
    /// and causes, hidden draws excluded); `None` out of range. One clone per call.
    pub fn preview(&self, i: usize) -> Option<ActionPreview> {
        let action = *self.actions.get(i)?;
        let run = dry_run(self.state, action);
        Some(preview_of(
            action,
            run.as_ref().map(|(_, events)| events.as_slice()),
        ))
    }

    /// The outcome of the legal action at index `i` of [`actions`](Self::actions): its preview and
    /// the view of the acting seat after it, in which a drawn card is only counted
    /// ([`OutcomeView`]); `None` out of range. One clone per call, nothing is evaluated before.
    /// Should `apply` refuse the action on the clone (an internal inconsistency), the outcome has
    /// no consequence: an empty preview and the unchanged view.
    pub fn outcome(&self, i: usize) -> Option<ActionOutcome> {
        let action = *self.actions.get(i)?;
        let seat = self.seat();
        let outcome = match dry_run(self.state, action) {
            Some((after, events)) => ActionOutcome {
                preview: preview_of(action, Some(&events)),
                view_after: outcome_view(&after, seat, &drawn_ids(&events, seat)),
            },
            None => ActionOutcome {
                preview: preview_of(action, None),
                view_after: outcome_view(self.state, seat, &[]),
            },
        };
        Some(outcome)
    }
}

/// The cards `seat` drew from its pile during an action: provenance, read from the `Drew`
/// events (the only way a card reaches a hand from the pile).
fn drawn_ids(events: &[Event], seat: PlayerId) -> Vec<CardInstanceId> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Drew { player, card } if *player == seat => Some(*card),
            _ => None,
        })
        .collect()
}

/// What one legal action would do, as the acting seat may know it: its [`ActionPreview`] and
/// its own view right after it, a draw only counted (never shown). `Serialize` is deliberately
/// absent: nothing exposes it to a client, only the Rust AI reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionOutcome {
    pub preview: ActionPreview,
    pub view_after: OutcomeView,
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
