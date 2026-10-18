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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{draw, hero, potential, setup, stats_with};
    use crate::{
        GameSetup, RuleSet, apply, game_setup_from_data, legal_actions, new_game,
        preview_legal_actions,
    };
    use rand::{RngExt, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use std::time::Instant;

    const STARTER_CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
    const STARTER_DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");
    const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
    const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

    /// The hostile sweep runs at step 0 and at every `SWEEP_PERIOD`th step of each game (and on
    /// every artificial state). It costs about 500 state clones, so the period is what keeps the
    /// whole differential under 60 s in debug; the 250 seeds and the per-step comparison of
    /// `legal_actions` are never reduced.
    const SWEEP_PERIOD: usize = 2;
    const SEEDS: u64 = 250;
    const STEP_CAP: usize = 3000;

    /// Copied from `rules.rs` (with the rules as a parameter): about a third of the cards, by
    /// deck position, have provocation, on both sides.
    fn provocative_game(seed: u64, rules: RuleSet) -> GameState {
        let mut s = setup(
            hero(1 + (seed % 2) as u32, 3, 8, vec![draw(1, 1), draw(4, 3)]),
            hero(1, 2, 6, vec![draw(2, 2)]),
        );
        let cards = |offset: u32| {
            (0..30u32)
                .map(|n| {
                    let n = n + offset;
                    stats_with(
                        1 + n % 3,
                        1 + n % 3,
                        1 + n % 2,
                        1 + n % 4,
                        n.is_multiple_of(3),
                        0,
                    )
                })
                .collect::<Vec<_>>()
        };
        s.one.cards = cards(0).into_iter().map(Into::into).collect();
        s.two.cards = cards(1).into_iter().map(Into::into).collect();
        s.rules = rules;
        new_game(s, seed).unwrap().0
    }

    /// Copied from `rules.rs`: every attacker of the active player against the hero, every id of
    /// the game (60 cards, on any zone or none) and an unknown id.
    fn every_attack(state: &GameState) -> Vec<Action> {
        let ids = (0..61).chain([10_000]).map(CardInstanceId);
        state
            .player(state.active)
            .board
            .iter()
            .flat_map(|attacker| {
                std::iter::once(Target::Hero)
                    .chain(ids.clone().map(Target::Creature))
                    .map(|target| Action::Attack {
                        attacker: attacker.id,
                        target,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn data_game(
        cards: &str,
        decks: &str,
        one: &str,
        two: &str,
        rules: RuleSet,
        seed: u64,
    ) -> GameState {
        let mut s: GameSetup = game_setup_from_data(cards, decks, one, two, Vec::new()).unwrap();
        s.rules = rules;
        new_game(s, seed).unwrap().0
    }

    fn rules_of(seed: u64) -> RuleSet {
        match seed % 5 {
            0 => RuleSet::legacy(),
            1 => RuleSet::standard(),
            2 => RuleSet {
                hero_riposte: true,
                board_limit: Some(1),
            },
            3 => RuleSet {
                hero_riposte: true,
                board_limit: Some(3),
            },
            _ => RuleSet {
                hero_riposte: true,
                board_limit: None,
            },
        }
    }

    /// Game `seed % 3` among the provocative game, starter `rush` against `bulwark` (the two
    /// seatings, by the parity of the seed) and the fixture.
    fn game_of(seed: u64) -> GameState {
        let rules = rules_of(seed);
        match seed % 3 {
            0 => provocative_game(seed, rules),
            1 => {
                let (one, two) = if seed.is_multiple_of(2) {
                    ("rush", "bulwark")
                } else {
                    ("bulwark", "rush")
                };
                data_game(STARTER_CARDS, STARTER_DECKS, one, two, rules, seed)
            }
            _ => data_game(
                FIXTURE_CARDS,
                FIXTURE_DECKS,
                "aggro",
                "control",
                rules,
                seed,
            ),
        }
    }

    /// The actions a hostile caller may send, beyond the legal ones: every attack of
    /// `every_attack`, `Play` and `Activate { source }` on every id, `Activate` on indices past
    /// the hero's list, and actions carrying 1, 8 and 9 picks.
    fn hostile_actions(state: &GameState) -> Vec<Action> {
        let ids = || (0..61).chain([10_000]).map(CardInstanceId);
        let mut actions = every_attack(state);
        for id in ids() {
            actions.push(Action::play(id));
            actions.push(Action::Activate {
                effect: 0,
                source: Some(id),
                picks: Vec::new(),
            });
        }
        let effects = u32::try_from(state.player(state.active).hero.effects.len()).unwrap();
        actions.extend((0..effects + 4).map(Action::activate));
        let first_card = state
            .player(state.active)
            .hand
            .first()
            .map_or(CardInstanceId(10_000), |c| c.id);
        let first_creature = state
            .player(state.active)
            .board
            .first()
            .map_or(CardInstanceId(10_000), |c| c.id);
        for count in [1, 8, 9] {
            for pick in [Pick::Hero(PlayerId::One), Pick::Creature(first_creature)] {
                let picks = vec![pick; count];
                for card in [first_card, CardInstanceId(10_000)] {
                    actions.push(Action::Play {
                        card,
                        picks: picks.clone(),
                    });
                }
                for source in [None, Some(first_creature), Some(CardInstanceId(10_000))] {
                    for effect in [0, effects + 3] {
                        actions.push(Action::Activate {
                            effect,
                            source,
                            picks: picks.clone(),
                        });
                    }
                }
            }
        }
        actions.push(Action::Concede);
        actions
    }

    /// Every hostile action on two clones: same `Result`, same resulting state, and a refusal
    /// changes neither the state nor the generator.
    fn sweep(state: &GameState, context: &str) {
        for action in hostile_actions(state) {
            if is_force_zero_attack(state, &action) {
                // `combat::plan_damage` (shared, unchanged here) debug-asserts a force of 1 or
                // more, so applying panics in debug builds until #131 makes `validate` refuse
                // it: the two judgments are compared instead.
                assert_eq!(
                    crate::rules::validate(state, &action),
                    validate(state, &action),
                    "{context}: {action:?}"
                );
                continue;
            }
            let (mut new, mut old) = (state.clone(), state.clone());
            let got = apply(&mut new, action.clone());
            let expected = apply_reference(&mut old, action.clone());
            assert_eq!(got, expected, "{context}: {action:?}");
            assert_eq!(new, old, "{context}: {action:?} left different states");
            if got.is_err() {
                assert_eq!(
                    new, *state,
                    "{context}: a refusal of {action:?} changed the state"
                );
            }
        }
    }

    /// An attack by a creature of the active board whose force is 0 (only built by hand).
    fn is_force_zero_attack(state: &GameState, action: &Action) -> bool {
        matches!(action, Action::Attack { attacker, .. }
            if state.player(state.active).board.iter().any(|c| c.id == *attacker && c.stats.force == 0))
    }

    fn apply_reference(state: &mut GameState, action: Action) -> Result<Vec<Event>, RuleError> {
        super::apply(state, action)
    }

    /// The listing, its preview and the exhaustion check, new against reference.
    fn compare_listing(state: &GameState, context: &str) {
        let legal = legal_actions(state);
        assert_eq!(
            legal,
            super::legal_actions(state),
            "{context}: legal_actions"
        );
        let previews: Vec<Action> = preview_legal_actions(state)
            .into_iter()
            .map(|p| p.action)
            .collect();
        assert_eq!(previews, legal, "{context}: preview_legal_actions");
        let (mut new, mut old) = (state.clone(), state.clone());
        let (mut new_events, mut old_events) = (Vec::new(), Vec::new());
        crate::rules::check_exhaustion(&mut new, &mut new_events);
        super::check_exhaustion(&mut old, &mut old_events);
        assert_eq!(new_events, old_events, "{context}: check_exhaustion events");
        assert_eq!(new, old, "{context}: check_exhaustion state");
    }

    #[test]
    fn legal_actions_match_the_reference_in_order_on_random_games() {
        let started = Instant::now();
        let mut sweeps = 0;
        for seed in 0..SEEDS {
            let mut state = game_of(seed);
            let mut policy = ChaCha8Rng::seed_from_u64(seed ^ 0xD1FF);
            let mut steps = 0;
            loop {
                let context = format!("seed {seed} step {steps}");
                compare_listing(&state, &context);
                if steps % SWEEP_PERIOD == 0 {
                    sweep(&state, &context);
                    sweeps += 1;
                }
                if state.winner.is_some() {
                    break;
                }
                let playable: Vec<Action> = legal_actions(&state)
                    .into_iter()
                    .filter(|a| *a != Action::Concede)
                    .collect();
                assert!(!playable.is_empty(), "{context}: exhausted yet not over");
                let action = playable[policy.random_range(0..playable.len())].clone();
                let before = potential(&state);
                apply(&mut state, action.clone()).unwrap();
                assert!(
                    potential(&state) < before,
                    "{context}: Φ did not fall on {action:?}"
                );
                steps += 1;
                assert!(steps < STEP_CAP, "seed {seed}: the game does not end");
            }
        }
        println!(
            "differential: {SEEDS} games, {sweeps} hostile sweeps (period {SWEEP_PERIOD}), {:?}",
            started.elapsed()
        );
    }

    /// A provocative game with `own` creatures on the active board and `opposing` on the other,
    /// moved from the piles (so ids stay unique).
    fn with_boards(seed: u64, rules: RuleSet, own: usize, opposing: usize) -> GameState {
        let mut state = provocative_game(seed, rules);
        for (seat, count) in [(state.active, own), (state.active.opponent(), opposing)] {
            let side = state.player_mut(seat);
            for _ in 0..count {
                let card = side.deck.pop().unwrap();
                side.board.push(card);
            }
        }
        state
    }

    fn artificial_states() -> Vec<(&'static str, GameState)> {
        let mut states = Vec::new();
        for seed in 0..5u64 {
            let rules = rules_of(seed);
            let base = || with_boards(seed, rules, 3, 3);
            let mut over_limit = with_boards(seed, rules, 7, 2);
            over_limit.rules.board_limit = Some(5);
            states.push(("board above the limit", over_limit));
            let mut s = base();
            let active = s.active;
            s.player_mut(active).hand.clear();
            states.push(("empty hand", s));
            let mut s = with_boards(seed, rules, 0, 2);
            let active = s.active;
            states.push(("empty own board", s.clone()));
            s.player_mut(active.opponent()).board.clear();
            states.push(("both boards empty", s));
            states.push(("empty opposing board", with_boards(seed, rules, 3, 0)));
            let mut s = base();
            let active = s.active;
            s.player_mut(active).deck.clear();
            states.push(("empty pile", s));
            let mut s = base();
            let active = s.active;
            s.player_mut(active).hero.effects.clear();
            states.push(("hero without effects", s));
            let mut s = base();
            let active = s.active;
            s.player_mut(active).hero.effects =
                (0..8).map(|n| draw(1 + n % 3, 1 + n % 2)).collect();
            states.push(("hero with eight effects", s));
            let mut s = base();
            s.winner = Some((PlayerId::One, WinReason::Concede));
            states.push(("finished game", s));
            let mut s = with_boards(seed, rules, 0, 3);
            let active = s.active;
            let side = s.player_mut(active);
            side.hand.clear();
            side.hero.effects.clear();
            states.push(("exhausted active player", s));
            let mut s = base();
            let active = s.active;
            let dead = s.player_mut(active).board.remove(0);
            s.player_mut(active).discard.push(dead);
            states.push(("dead attacker still referenced by id", s));
        }
        states
    }

    #[test]
    fn artificial_states_agree_with_the_reference() {
        for (index, (label, state)) in artificial_states().into_iter().enumerate() {
            let context = format!("{label} #{index}");
            compare_listing(&state, &context);
            sweep(&state, &context);
        }
    }

    #[test]
    fn a_force_zero_attacker_is_still_legal_as_before() {
        for seed in 0..5u64 {
            let mut state = with_boards(seed, rules_of(seed), 2, 1);
            let active = state.active;
            let attacker = {
                let side = state.player_mut(active);
                side.board[0].stats.force = 0;
                side.board[0].id
            };
            // Whatever the provocation, one target is legal: the hero, or a provocateur.
            let legal = legal_actions(&state);
            let zero_attacks: Vec<&Action> = legal
                .iter()
                .filter(|a| matches!(a, Action::Attack { attacker: id, .. } if *id == attacker))
                .collect();
            assert!(
                !zero_attacks.is_empty(),
                "seed {seed}: the force-0 attacker is not listed"
            );
            assert_eq!(legal, super::legal_actions(&state), "seed {seed}");
            // Accepted by `validate` (listed), as before; applying one is left to the sweep's
            // `validate` comparison (see `sweep`).
            for action in zero_attacks {
                assert_eq!(
                    crate::rules::validate(&state, action),
                    Ok(()),
                    "seed {seed}"
                );
                assert_eq!(validate(&state, action), Ok(()), "seed {seed}");
            }
            sweep(&state, &format!("force zero, seed {seed}"));
        }
    }

    /// FNV-1a, 64 bits.
    fn fnv1a(text: &str) -> u64 {
        text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
    }

    /// Updated by task 7 only, which deletes the module.
    const ORACLE_CHECKSUM: u64 = 0x662d_bf98_23e4_5b5f;

    #[test]
    fn the_reference_is_frozen_by_a_checksum() {
        let text = include_str!("reference.rs");
        let marker = ["\n#[cfg(test)]\n", "mod tests {"].concat();
        let frozen = text.split(marker.as_str()).next().unwrap();
        assert_eq!(
            fnv1a(frozen),
            ORACLE_CHECKSUM,
            "reference.rs (outside its tests) changed: it is frozen until task 7 of #128 removes it"
        );
    }
}
