//! The evaluation behind the greedy policy: integers only (so native and WASM take the same
//! decisions), one score per action outcome, one place that reads the fields of a card.
//!
//! The score of an action is computed on what the seat is allowed to know after it
//! ([`ActionOutcome`]): its own view with a drawn card only counted, the opponent's public side,
//! the preview. Nothing here knows a rule of the game, a card by its name or a deck.

use duel_engine::{ActionOutcome, CardView, Observation, PlayerId};
use std::cmp::Reverse;

/// Value of a won game, and its opposite for a lost one. Large enough to dominate any sum of the
/// other terms, small enough that adding them to it cannot overflow an `i64`.
pub(crate) const WIN: i64 = 1 << 40;

/// The weights of the evaluation, all integers, all provisional (a card value of its own is
/// #133's job, see `docs/ai/greedy.md`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Weights {
    /// Value of one hero health point (mine counts for, the opponent's against).
    pub hero_hp: i64,
    /// Value of one point of `force` of a creature in play.
    pub force: i64,
    /// Value of one point of `riposte` of a creature in play.
    pub riposte: i64,
    /// Value of one point of current `health` of a creature in play.
    pub health: i64,
    /// Bonus for a creature with `provocation`.
    pub provocation: i64,
    /// Share, in percent, of its board value that a known card in hand keeps.
    pub hand_percent: i64,
    /// Value of a card drawn (never shown, so a constant) and of a card in the opposing hand.
    pub draw_value: i64,
    /// Value of one unit of time left to the seat when it keeps the token, or granted to the
    /// opponent when it hands the token over.
    pub time: i64,
    /// Penalty for handing the token over.
    pub handover: i64,
}

pub(crate) const DEFAULT_WEIGHTS: Weights = Weights {
    hero_hp: 2,
    force: 3,
    riposte: 2,
    health: 2,
    provocation: 4,
    hand_percent: 50,
    draw_value: 6,
    time: 3,
    handover: 4,
};

/// What a card is worth in play, from the statistics of its view only: never its name, id or
/// cost. A card with no health left is worth nothing.
pub(crate) fn card_value(card: &CardView, w: &Weights) -> i64 {
    if card.health == 0 {
        return 0;
    }
    let provocation = if card.provocation { w.provocation } else { 0 };
    i64::from(card.force)
        .saturating_mul(w.force)
        .saturating_add(i64::from(card.riposte).saturating_mul(w.riposte))
        .saturating_add(i64::from(card.health).saturating_mul(w.health))
        .saturating_add(provocation)
}

fn board_value(cards: &[CardView], w: &Weights) -> i64 {
    cards
        .iter()
        .fold(0, |sum, card| sum.saturating_add(card_value(card, w)))
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// The score of an outcome for `seat`, the seat that acted: higher is better.
pub(crate) fn score(outcome: &ActionOutcome, seat: PlayerId, w: &Weights) -> i64 {
    let view = &outcome.view_after;
    let terminal = match outcome.preview.ends_game.or(view.result) {
        Some(result) if result.winner == seat => WIN,
        Some(_) => -WIN,
        None => 0,
    };
    let heroes = i64::from(view.you.hero.health)
        .saturating_sub(i64::from(view.opponent.hero.health))
        .saturating_mul(w.hero_hp);
    let boards =
        board_value(&view.you.board, w).saturating_sub(board_value(&view.opponent.board, w));
    let hand = board_value(&view.you.hand_known, w)
        .saturating_mul(w.hand_percent)
        .saturating_div(100)
        .saturating_add(count(view.you.hand_unseen).saturating_mul(w.draw_value))
        .saturating_sub(count(view.opponent.hand_count).saturating_mul(w.draw_value));
    let time = i64::from(view.budget).saturating_mul(w.time);
    let time = if view.active == seat {
        time
    } else {
        time.saturating_neg().saturating_sub(w.handover)
    };
    [terminal, heroes, boards, hand, time]
        .into_iter()
        .fold(0, i64::saturating_add)
}

/// The index of the best action of `obs`: highest score, then lowest cost (an action with no
/// cost last), then lowest index (the engine's order). An empty list gives 0 (the harness never
/// asks a policy in that case).
pub(crate) fn choose_best(obs: &Observation<'_>, w: &Weights) -> usize {
    let seat = obs.seat();
    (0..obs.actions().len())
        .filter_map(|i| {
            let outcome = obs.outcome(i)?;
            let cost = outcome.preview.cost.unwrap_or(u32::MAX);
            Some((Reverse(score(&outcome, seat, w)), cost, i))
        })
        .min()
        .map_or(0, |(_, _, i)| i)
}
