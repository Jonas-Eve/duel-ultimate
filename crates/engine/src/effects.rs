//! The effects of the game: what a source carries, the single legality predicate [`usable`], the
//! listing of the legal actions derived from the effects present, and the resolution of hero
//! effects (one isolated step, so new effects are new variants).
//!
//! The read side (#183): [`effects_of`] is the one accessor of the effects a source carries,
//! derived on read, so a later change of their storage touches nothing else. [`sources`] lists
//! every candidate [`Use`] in the order of the legal actions, [`usable`] is the only judge of
//! legality (called by `validate`, `legal_actions` and `check_exhaustion`), and [`decode`] /
//! [`encode`] turn the wire [`Action`] into a [`Use`] and back. The attack is an intrinsic
//! effect, synthesized for every creature of the board (never a line of data); a unit may declare
//! effects (#208), carried by its instance but never resolved yet (`unsupported_effect`).

use crate::combat;
use crate::rules::{
    Action, Event, Pick, RuleError, Target, attack_blocker, board_position, hand_position,
    hero_effect, opposing_board_position, play_blocker_of, provocation_allows,
};
use crate::state::{CardInstance, CardInstanceId, GameState, PlayerId, WinReason};
use duel_cards::{Effect, Trigger};

/// The values of a card the rules read: cost, force, attack cost, riposte and provocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EffectiveStats {
    pub(crate) cost: u32,
    pub(crate) force: u32,
    pub(crate) attack_cost: u32,
    pub(crate) riposte: u32,
    pub(crate) provocation: bool,
}

/// The only reader of the effective values of `card` (guarded by the source scan of
/// `tests/effective_is_the_only_reader.rs`). The identity at this slice: the five printed
/// values; #131 folds modifiers (from `state` and the card) in here, without touching a caller.
/// The current health is not an effective value and stays read as `card.stats.health`.
#[inline]
pub(crate) fn effective(_state: &GameState, card: &CardInstance) -> EffectiveStats {
    EffectiveStats {
        cost: card.stats.cost,
        force: card.stats.force,
        attack_cost: card.stats.attack_cost,
        riposte: card.stats.riposte,
        provocation: card.stats.provocation,
    }
}

/// Where an effect comes from: a card of the active hand, a creature of the active board, or the
/// active hero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Hand(CardInstanceId),
    Board(CardInstanceId),
    Hero,
}

/// Which effect of a source: summoning a card, the intrinsic attack of a creature, or the effect
/// of the hero's list at this index (the effects a unit declares are not listed here yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectRef {
    Play,
    Attack,
    Declared(u32),
}

/// What the player chose for the effect: an attack target or a list of picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice<'a> {
    Target(Target),
    Picks(&'a [Pick]),
}

/// One thing the active player may try: conceding, or using an effect of a source with a choice.
/// Larger than [`Action`] (a hand card cannot attack): [`encode`] gives `None` for the
/// combinations no action expresses, and [`usable`] refuses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Use<'a> {
    Concede,
    Effect {
        source: Source,
        effect: EffectRef,
        choice: Choice<'a>,
    },
}

/// The effects a source of this kind carries, without looking for it in the state: `Play` for a
/// card of the hand, `Attack` for a creature of the board, `Declared(i)` for the `declared`
/// effects of the hero's list. The effects a unit declares are not listed here yet (#129).
fn carried(source: Source, declared: usize) -> impl Iterator<Item = EffectRef> {
    let intrinsic = match source {
        Source::Hand(_) => Some(EffectRef::Play),
        Source::Board(_) => Some(EffectRef::Attack),
        Source::Hero => None,
    };
    let declared = if source == Source::Hero { declared } else { 0 };
    intrinsic.into_iter().chain(
        (0..declared)
            .filter_map(|i| u32::try_from(i).ok())
            .map(EffectRef::Declared),
    )
}

/// The effects `source` carries for the active player: `Play` for a card of the hand, `Attack`
/// for a creature of the board, `Declared(i)` for each effect of the hero's list, in order.
/// Empty for a source that is not where it claims to be. The single accessor of this list.
///
/// Since #208 `usable` no longer asks it for the declared effects of a board unit (it reads the
/// instance it already holds); #129 gives the units their listed effects back.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn effects_of(
    state: &GameState,
    source: Source,
) -> impl Iterator<Item = EffectRef> + '_ {
    let present = match source {
        Source::Hand(card) => hand_position(state, card).is_some(),
        Source::Board(creature) => board_position(state, creature).is_some(),
        Source::Hero => true,
    };
    let declared = state.player(state.active).hero.effects.len();
    carried(source, declared).filter(move |_| present)
}

/// Lazy iterator of [`sources`]: a hand-written cursor, because a chain of adapters costs a
/// fifth of the throughput of the simulator on this hot path. `effect` is the position in
/// `carried(source, ..)` and `target` the position among the attack targets (0 = the opposing
/// hero, k = the k-th opposing creature).
struct Sources<'s> {
    state: &'s GameState,
    /// Which list of the active side is walked: 0 = hand, 1 = hero, 2 = board.
    list: u8,
    /// Position of the source in its list (always 0 for the hero).
    index: usize,
    effect: usize,
    target: usize,
}

impl Sources<'_> {
    /// The source at the cursor, or `None` past the end of its list.
    fn source(&self) -> Option<Source> {
        let side = self.state.player(self.state.active);
        match self.list {
            0 => side.hand.get(self.index).map(|c| Source::Hand(c.id)),
            1 => (self.index == 0).then_some(Source::Hero),
            _ => side.board.get(self.index).map(|c| Source::Board(c.id)),
        }
    }
}

impl Iterator for Sources<'_> {
    type Item = Use<'static>;

    fn next(&mut self) -> Option<Use<'static>> {
        let side = self.state.player(self.state.active);
        let opposing = &self.state.player(self.state.active.opponent()).board;
        loop {
            let Some(source) = self.source() else {
                if self.list >= 2 {
                    return None;
                }
                (self.list, self.index, self.effect, self.target) = (self.list + 1, 0, 0, 0);
                continue;
            };
            let Some(effect) = carried(source, side.hero.effects.len()).nth(self.effect) else {
                (self.index, self.effect, self.target) = (self.index + 1, 0, 0);
                continue;
            };
            let choice = if effect == EffectRef::Attack {
                let target = match self.target {
                    0 => Some(Target::Hero),
                    k => opposing.get(k - 1).map(|c| Target::Creature(c.id)),
                };
                self.target += 1;
                match target {
                    Some(target) => Choice::Target(target),
                    None => {
                        (self.effect, self.target) = (self.effect + 1, 0);
                        continue;
                    }
                }
            } else {
                self.effect += 1;
                Choice::Picks(&[])
            };
            return Some(Use::Effect {
                source,
                effect,
                choice,
            });
        }
    }
}

/// Every candidate use of the active player, `Concede` excluded, lazily and in the order of the
/// legal actions: the hand, the hero's declared effects, then per creature of the board (board
/// order) its effects, an attack once per target (the opposing hero first, then each opposing
/// creature in board order, never an own creature). The sources are read from the active side
/// itself, so each is where it claims to be (`effects_of` is the checked accessor for a source
/// named from outside). [`usable`] stays the single judge of which are legal, provocation
/// included.
pub(crate) fn sources(state: &GameState) -> impl Iterator<Item = Use<'static>> + '_ {
    Sources {
        state,
        list: 0,
        index: 0,
        effect: 0,
        target: 0,
    }
}

/// The use an action expresses. Total: whatever the action names, untrusted, [`usable`] judges.
pub(crate) fn decode(action: &Action) -> Use<'_> {
    match action {
        Action::Play { card, picks } => Use::Effect {
            source: Source::Hand(*card),
            effect: EffectRef::Play,
            choice: Choice::Picks(picks),
        },
        Action::Activate {
            effect,
            source,
            picks,
        } => Use::Effect {
            source: source.map_or(Source::Hero, Source::Board),
            effect: EffectRef::Declared(*effect),
            choice: Choice::Picks(picks),
        },
        Action::Attack { attacker, target } => Use::Effect {
            source: Source::Board(*attacker),
            effect: EffectRef::Attack,
            choice: Choice::Target(*target),
        },
        Action::Concede => Use::Concede,
    }
}

/// The action that expresses a use, the inverse of [`decode`]; `None` for a combination no
/// action expresses (never produced by [`sources`]).
pub(crate) fn encode(used: &Use<'_>) -> Option<Action> {
    match *used {
        Use::Concede => Some(Action::Concede),
        Use::Effect {
            source: Source::Hand(card),
            effect: EffectRef::Play,
            choice: Choice::Picks(picks),
        } => Some(Action::Play {
            card,
            picks: picks.to_vec(),
        }),
        Use::Effect {
            source,
            effect: EffectRef::Declared(effect),
            choice: Choice::Picks(picks),
        } => Some(Action::Activate {
            effect,
            source: match source {
                Source::Board(id) => Some(id),
                Source::Hero => None,
                Source::Hand(_) => return None,
            },
            picks: picks.to_vec(),
        }),
        Use::Effect {
            source: Source::Board(attacker),
            effect: EffectRef::Attack,
            choice: Choice::Target(target),
        } => Some(Action::Attack { attacker, target }),
        Use::Effect { .. } => None,
    }
}

/// The single legality predicate: whether the active player may use this now. The checks and
/// their order are those `validate` always had (the game being over and the size of the picks
/// are judged before, by `validate`). An attack checks the force of its attacker (`attack_blocker`) right after the attacker.
pub(crate) fn usable(state: &GameState, used: &Use<'_>) -> Result<(), RuleError> {
    let Use::Effect {
        source,
        effect,
        choice,
    } = *used
    else {
        return Ok(());
    };
    match (source, effect, choice) {
        (Source::Hand(card), EffectRef::Play, Choice::Picks(picks)) => {
            // The hand first: an id that is not in it gets `CardNotInHand` whatever the board.
            let card = hand_position(state, card)
                .and_then(|i| state.player(state.active).hand.get(i))
                .ok_or(RuleError::CardNotInHand)?;
            // No summoning takes a choice at this stage.
            if !picks.is_empty() {
                return Err(RuleError::InvalidPicks);
            }
            play_blocker_of(state, state.active, card).map_or(Ok(()), Err)
        }
        (Source::Board(attacker), EffectRef::Attack, Choice::Target(target)) => {
            // Order of the checks: the attacker, its force, then the target on the opposing
            // board, then the provocation. Every id that is not an opposing creature gets `TargetNotOnBoard`
            // whatever zone (hidden or not) it is in, even when a provocateur exists: the
            // provocation error only ever speaks of the public board.
            let attacker = board_position(state, attacker)
                .and_then(|i| state.player(state.active).board.get(i))
                .ok_or(RuleError::AttackerNotOnBoard)?;
            if let Some(blocked) = attack_blocker(state, attacker) {
                return Err(blocked);
            }
            if let Target::Creature(creature) = target {
                opposing_board_position(state, creature).ok_or(RuleError::TargetNotOnBoard)?;
            }
            if provocation_allows(state, target) {
                Ok(())
            } else {
                Err(RuleError::TargetForbiddenByProvocation)
            }
        }
        (Source::Hero, EffectRef::Declared(index), Choice::Picks(picks)) => {
            let declared = hero_effect(state, index).ok_or(RuleError::UnknownEffect)?;
            // No hero effect takes a choice at this stage.
            if !picks.is_empty() {
                return Err(RuleError::InvalidPicks);
            }
            if applicable(state, state.active, &declared.effect) {
                Ok(())
            } else {
                Err(RuleError::EffectNotApplicable)
            }
        }
        (Source::Board(creature), EffectRef::Declared(index), _) => {
            // One code for every id that is not a creature of my board (hand, pile, discard,
            // opposing board, unknown): no hidden zone is told apart.
            let unit = board_position(state, creature)
                .and_then(|i| state.player(state.active).board.get(i))
                .ok_or(RuleError::SourceNotOnBoard)?;
            // A declared `activated` effect exists but is not resolved yet (temporary code, until
            // #129); any other index, or an effect that is not `activated`, is no effect to
            // activate.
            let declared = usize::try_from(index)
                .ok()
                .and_then(|i| unit.effects().get(i));
            match declared {
                Some(declared) if declared.trigger == Trigger::Activated => {
                    Err(RuleError::UnsupportedEffect)
                }
                _ => Err(RuleError::UnknownEffect),
            }
        }
        // A combination `decode` never builds (a card of the hand that attacks, a hero that
        // plays...): no such effect.
        _ => Err(RuleError::UnknownEffect),
    }
}

/// Whether `effect` can currently be activated by `player`.
pub(crate) fn applicable(state: &GameState, player: PlayerId, effect: &Effect) -> bool {
    match effect {
        Effect::Draw { .. } => !state.player(player).deck.is_empty(),
    }
}

/// What an effect does when resolved: the operations of the write side. Built on the fly from an
/// [`EffectRef`] (never stored); [`Effect`] stays the type of the data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Op {
    /// Moves up to `count` cards from the pile to the hand.
    Draw { count: u32 },
    /// The attack of a creature: damage planned then applied by `combat`.
    Attack,
    /// Summons a card of the hand onto the board.
    Play,
}

/// The operation of a declared effect: the one place the sugar of the hero's list is normalized.
pub(crate) fn op_of(effect: &Effect) -> Op {
    match effect {
        Effect::Draw { count } => Op::Draw { count: *count },
    }
}

/// How a resolved use ends: it costs `Spent(cost)` of time (the caller pays, then checks
/// exhaustion), or it `Ended` the game by itself (no time, the token stays).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Settled {
    Spent(u32),
    Ended,
}

/// Resolves a use `validate` has accepted, by operation, appending what happened to `events`.
/// Revalidates nothing: an inconsistency (unreachable once validated) is a `debug_assert!`
/// then a clean exit that changes nothing.
pub(crate) fn resolve(state: &mut GameState, used: &Use<'_>, events: &mut Vec<Event>) -> Settled {
    let player = state.active;
    let Use::Effect {
        source,
        effect,
        choice,
    } = *used
    else {
        let winner = player.opponent();
        state.winner = Some((winner, WinReason::Concede));
        events.push(Event::GameWon {
            winner,
            reason: WinReason::Concede,
        });
        return Settled::Ended;
    };
    // The declared effect of the hero, with its operation: (index, cost, op).
    let declared = match effect {
        EffectRef::Declared(index) => {
            hero_effect(state, index).map(|e| (index, e.cost, op_of(&e.effect)))
        }
        EffectRef::Play | EffectRef::Attack => None,
    };
    let op = match effect {
        EffectRef::Play => Some(Op::Play),
        EffectRef::Attack => Some(Op::Attack),
        EffectRef::Declared(_) => declared.map(|(_, _, op)| op),
    };
    match (op, source, choice) {
        (Some(Op::Play), Source::Hand(card), _) => {
            let Some(index) = hand_position(state, card) else {
                debug_assert!(false, "validated card in the hand");
                return Settled::Ended;
            };
            let creature = state.player_mut(player).hand.remove(index);
            let cost = effective(state, &creature).cost;
            state.player_mut(player).board.push(creature);
            events.push(Event::Played { player, card, cost });
            Settled::Spent(cost)
        }
        (Some(Op::Draw { count }), Source::Hero, _) => {
            let (index, cost) = declared.map_or((0, 0), |(index, cost, _)| (index, cost));
            events.push(Event::EffectActivated {
                player,
                effect: index,
                cost,
            });
            draw(state, player, count, events);
            Settled::Spent(cost)
        }
        (Some(Op::Attack), Source::Board(attacker), Choice::Target(target)) => {
            let Some(cost) = state
                .player(player)
                .board
                .iter()
                .find(|c| c.id == attacker)
                .map(|c| effective(state, c).attack_cost)
            else {
                debug_assert!(false, "validated attacker on the board");
                return Settled::Ended;
            };
            events.push(Event::Attacked {
                player,
                attacker,
                cost,
            });
            combat::resolve_damage(state, player, attacker, target, events);
            combat::bury_dead(state, events);
            if state.player(player.opponent()).health == 0 {
                // The game stops here: no time is spent, the token does not move and the
                // budget stays within 1..=max_time of the (unchanged) active player.
                state.winner = Some((player, WinReason::HeroDown));
                events.push(Event::GameWon {
                    winner: player,
                    reason: WinReason::HeroDown,
                });
                return Settled::Ended;
            }
            Settled::Spent(cost)
        }
        _ => {
            debug_assert!(false, "a use `validate` accepted always resolves");
            Settled::Ended
        }
    }
}

/// Draws what is left when the pile is shorter than `count`.
fn draw(state: &mut GameState, player: PlayerId, count: u32, events: &mut Vec<Event>) {
    let wanted = usize::try_from(count).unwrap_or(usize::MAX);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WinReason;
    use crate::testing::{draw, hero, setup, stats_with};
    use crate::{GameSetup, RuleSet, apply, game_setup_from_data, legal_actions, new_game};
    use rand::{RngExt, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    const STARTER_CARDS: &str = include_str!("../../../data/starter/1.1.0/cards.json");
    const STARTER_DECKS: &str = include_str!("../../../data/starter/1.1.0/decks.json");

    fn rules_of(seed: u64) -> RuleSet {
        match seed % 4 {
            0 => RuleSet::legacy(),
            1 => RuleSet::standard(),
            2 => RuleSet {
                hero_riposte: true,
                board_limit: Some(2),
            },
            _ => RuleSet {
                hero_riposte: true,
                board_limit: None,
            },
        }
    }

    /// About a third of the cards have provocation, on both sides.
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
                    .into()
                })
                .collect::<Vec<_>>()
        };
        s.one.cards = cards(0);
        s.two.cards = cards(1);
        s.rules = rules;
        new_game(s, seed).unwrap().0
    }

    fn starter_game(seed: u64, rules: RuleSet) -> GameState {
        let mut s: GameSetup =
            game_setup_from_data(STARTER_CARDS, STARTER_DECKS, "rush", "bulwark", Vec::new())
                .unwrap();
        s.rules = rules;
        new_game(s, seed).unwrap().0
    }

    fn game_of(seed: u64) -> GameState {
        if seed.is_multiple_of(2) {
            provocative_game(seed, rules_of(seed))
        } else {
            starter_game(seed, rules_of(seed))
        }
    }

    /// Every state of `seeds` random games, the initial one included.
    fn states(seeds: u64) -> Vec<GameState> {
        let mut all = Vec::new();
        for seed in 0..seeds {
            let mut state = game_of(seed);
            let mut policy = ChaCha8Rng::seed_from_u64(seed ^ 0x183);
            while state.winner.is_none() {
                all.push(state.clone());
                let playable: Vec<Action> = legal_actions(&state)
                    .into_iter()
                    .filter(|a| *a != Action::Concede)
                    .collect();
                let action = playable[policy.random_range(0..playable.len())].clone();
                apply(&mut state, action).unwrap();
            }
            all.push(state);
        }
        all
    }

    /// Every id of the game and an unknown one, with picks, with and without a source.
    fn hostile_actions(state: &GameState) -> Vec<Action> {
        let ids = (0..61).chain([10_000]).map(CardInstanceId);
        let mut actions = vec![Action::Concede, Action::activate(0), Action::activate(99)];
        for id in ids {
            actions.push(Action::play(id));
            actions.push(Action::Play {
                card: id,
                picks: vec![Pick::Hero(PlayerId::Two), Pick::Creature(id)],
            });
            actions.push(Action::Activate {
                effect: 1,
                source: Some(id),
                picks: vec![Pick::Creature(id)],
            });
            actions.push(Action::Activate {
                effect: 0,
                source: None,
                picks: vec![Pick::Hero(PlayerId::One)],
            });
            for target in [Target::Hero, Target::Creature(id)] {
                actions.push(Action::Attack {
                    attacker: id,
                    target,
                });
            }
        }
        actions.extend(legal_actions(state));
        actions
    }

    const FIXTURE_CARDS: &str = include_str!("../../../data/fixture/1.0.0/cards.json");
    const FIXTURE_DECKS: &str = include_str!("../../../data/fixture/1.0.0/decks.json");

    fn fixture_game(seed: u64, rules: RuleSet) -> GameState {
        let mut s: GameSetup =
            game_setup_from_data(FIXTURE_CARDS, FIXTURE_DECKS, "aggro", "control", Vec::new())
                .unwrap();
        s.rules = rules;
        new_game(s, seed).unwrap().0
    }

    /// Every state of one random game of `state`, the initial one included.
    fn play_out(mut state: GameState, seed: u64, into: &mut Vec<GameState>) {
        let mut policy = ChaCha8Rng::seed_from_u64(seed ^ 0x185);
        while state.winner.is_none() {
            into.push(state.clone());
            let playable: Vec<Action> = legal_actions(&state)
                .into_iter()
                .filter(|a| *a != Action::Concede)
                .collect();
            let action = playable[policy.random_range(0..playable.len())].clone();
            apply(&mut state, action).unwrap();
        }
        into.push(state);
    }

    #[test]
    fn effective_is_the_identity_in_this_slice() {
        let mut all = states(250);
        for seed in 0..250u64 {
            play_out(fixture_game(seed, rules_of(seed)), seed, &mut all);
        }
        let mut checked = 0;
        for state in &all {
            for side in [&state.one, &state.two] {
                let zones = [&side.deck, &side.hand, &side.board, &side.discard];
                for card in zones.into_iter().flatten() {
                    let got = effective(state, card);
                    assert_eq!(
                        got,
                        EffectiveStats {
                            cost: card.stats.cost,
                            force: card.stats.force,
                            attack_cost: card.stats.attack_cost,
                            riposte: card.stats.riposte,
                            provocation: card.stats.provocation,
                        }
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 100_000, "{checked} cards checked");
    }

    #[test]
    fn encode_and_decode_are_inverse_on_every_legal_action_and_every_hostile_action() {
        for (index, state) in states(40).iter().enumerate() {
            for action in hostile_actions(state) {
                assert_eq!(
                    encode(&decode(&action)),
                    Some(action.clone()),
                    "state {index}: {action:?}"
                );
            }
        }
    }

    #[test]
    fn every_legal_action_is_usable_and_nothing_else_is() {
        for (index, state) in states(40).iter().enumerate() {
            let derived: Vec<Action> = sources(state)
                .filter(|used| usable(state, used).is_ok())
                .filter_map(|used| encode(&used))
                .collect();
            let mut legal = legal_actions(state);
            if state.winner.is_some() {
                assert!(legal.is_empty(), "state {index}");
                continue;
            }
            assert_eq!(legal.pop(), Some(Action::Concede), "state {index}");
            assert_eq!(derived, legal, "state {index}");
            // `apply` accepts exactly these actions plus `Concede`: the hostile ones that are
            // not listed are refused, and the state is untouched.
            for action in hostile_actions(state) {
                let mut clone = state.clone();
                let accepted = apply_checked(&mut clone, &action);
                let listed = legal.contains(&action) || action == Action::Concede;
                assert_eq!(accepted, listed, "state {index}: {action:?}");
            }
        }
    }

    /// Whether `apply` accepts `action`; a refusal must leave the state untouched. A force-0
    /// attack is judged by `validate` only (`combat::plan_damage` debug-asserts a force of 1).
    fn apply_checked(state: &mut GameState, action: &Action) -> bool {
        let before = state.clone();
        if crate::rules::validate(state, action).is_ok() {
            if !matches!(action, Action::Attack { attacker, .. }
                if state.player(state.active).board.iter().any(|c| c.id == *attacker && c.stats.force == 0))
            {
                assert!(apply(state, action.clone()).is_ok(), "{action:?}");
            }
            true
        } else {
            assert!(apply(state, action.clone()).is_err(), "{action:?}");
            assert_eq!(*state, before);
            false
        }
    }

    #[test]
    fn a_creature_that_carries_attack_has_a_legal_attack_per_attacker() {
        for state in states(60) {
            if state.winner.is_some() {
                continue;
            }
            let legal = legal_actions(&state);
            for creature in &state.player(state.active).board {
                assert!(
                    legal.iter().any(
                        |a| matches!(a, Action::Attack { attacker, .. } if *attacker == creature.id)
                    ),
                    "{:?} has no legal attack",
                    creature.id
                );
            }
        }
    }

    #[test]
    fn exhaustion_is_no_usable_action() {
        let mut cases = Vec::new();
        for seed in 0..8u64 {
            let rules = rules_of(seed);
            let base = || provocative_game(seed, rules);
            let mut s = base();
            let a = s.active;
            let side = s.player_mut(a);
            side.hand.clear();
            side.hero.effects.clear();
            cases.push(s);
            let mut s = base();
            let a = s.active;
            let side = s.player_mut(a);
            side.hand.clear();
            side.deck.clear();
            cases.push(s);
            // A creature on the board keeps its owner in the game.
            let mut s = base();
            let a = s.active;
            let side = s.player_mut(a);
            side.hero.effects.clear();
            let card = side.hand.remove(0);
            side.board.push(card);
            cases.push(s);
            let mut s = base();
            let a = s.active;
            s.player_mut(a).hero.effects.clear();
            s.rules.board_limit = Some(1);
            let side = s.player_mut(a);
            let card = side.hand.remove(0);
            side.board.push(card);
            cases.push(s);
        }
        let (mut ended, mut continued) = (0, 0);
        for (index, state) in cases.into_iter().enumerate() {
            let none_usable = sources(&state).all(|u| usable(&state, &u).is_err());
            let mut after = state.clone();
            let mut events = Vec::new();
            crate::rules::check_exhaustion(&mut after, &mut events);
            if none_usable {
                ended += 1;
                let winner = state.active.opponent();
                assert_eq!(
                    after.winner,
                    Some((winner, WinReason::Exhaustion)),
                    "case {index}"
                );
                assert_eq!(
                    events,
                    vec![Event::GameWon {
                        winner,
                        reason: WinReason::Exhaustion
                    }]
                );
            } else {
                continued += 1;
                assert_eq!(after, state, "case {index}");
                assert!(events.is_empty());
            }
        }
        assert!(ended > 0 && continued > 0, "{ended} {continued}");
    }

    #[test]
    fn the_declared_effect_of_a_creature_is_always_unknown() {
        for state in states(20) {
            if state.winner.is_some() {
                continue;
            }
            let own: Vec<CardInstanceId> = state
                .player(state.active)
                .board
                .iter()
                .map(|c| c.id)
                .collect();
            for id in (0..61).chain([10_000]).map(CardInstanceId) {
                for effect in [0, 1, 99] {
                    let used = Use::Effect {
                        source: Source::Board(id),
                        effect: EffectRef::Declared(effect),
                        choice: Choice::Picks(&[]),
                    };
                    let expected = if own.contains(&id) {
                        RuleError::UnknownEffect
                    } else {
                        RuleError::SourceNotOnBoard
                    };
                    assert_eq!(usable(&state, &used), Err(expected), "{id:?}");
                }
            }
        }
    }

    #[test]
    fn effects_of_lists_play_for_a_hand_attack_for_a_board_and_the_hero_list_in_order() {
        let mut state = provocative_game(1, RuleSet::standard());
        let a = state.active;
        let side = state.player_mut(a);
        let card = side.hand.remove(0);
        let on_board = card.id;
        side.board.push(card);
        let in_hand = state.player(a).hand[0].id;
        let hero_list = state.player(a).hero.effects.len() as u32;
        assert!(hero_list >= 2);
        let listed = |source| effects_of(&state, source).collect::<Vec<_>>();
        assert_eq!(listed(Source::Hand(in_hand)), vec![EffectRef::Play]);
        assert_eq!(listed(Source::Board(on_board)), vec![EffectRef::Attack]);
        assert_eq!(
            listed(Source::Hero),
            (0..hero_list).map(EffectRef::Declared).collect::<Vec<_>>()
        );
        // A source that is not where it claims to be carries nothing.
        assert!(listed(Source::Hand(on_board)).is_empty());
        assert!(listed(Source::Board(in_hand)).is_empty());
        assert!(listed(Source::Hand(CardInstanceId(10_000))).is_empty());
    }

    /// What `apply` gives on a clone of `state`.
    fn applied(state: &GameState, action: &Action) -> Result<Vec<Event>, RuleError> {
        apply(&mut state.clone(), action.clone())
    }

    /// An attack by a creature of force 0 is only judged by `validate` (`plan_damage`
    /// debug-asserts a force of 1).
    fn is_force_zero_attack(state: &GameState, action: &Action) -> bool {
        matches!(action, Action::Attack { attacker, .. }
            if state.player(state.active).board.iter().any(|c| c.id == *attacker && c.stats.force == 0))
    }

    #[test]
    fn random_play_exercises_every_operation_and_every_journal_shape() {
        #[derive(Default)]
        struct Seen {
            play: bool,
            multi_draw: bool,
            lethal: bool,
            hero_hit: bool,
            dead_attacker: bool,
            hero_riposte: bool,
            concede: bool,
        }
        let mut seen = Seen::default();
        for seed in 0..60u64 {
            let mut state = game_of(seed);
            let mut policy = ChaCha8Rng::seed_from_u64(seed ^ 0x184);
            while state.winner.is_none() {
                let legal = legal_actions(&state);
                for action in &legal {
                    if is_force_zero_attack(&state, action) {
                        continue;
                    }
                    let events = applied(&state, action).unwrap();
                    let attacker = match action {
                        Action::Attack { attacker, .. } => Some(*attacker),
                        _ => None,
                    };
                    let won = |why| {
                        events
                            .iter()
                            .any(|e| matches!(e, Event::GameWon { reason, .. } if *reason == why))
                    };
                    match action {
                        Action::Play { .. } => seen.play = true,
                        Action::Concede => seen.concede = true,
                        Action::Activate { .. } => {
                            let drawn = events
                                .iter()
                                .filter(|e| matches!(e, Event::Drew { .. }))
                                .count();
                            seen.multi_draw |= drawn >= 2;
                        }
                        Action::Attack {
                            target: Target::Hero,
                            ..
                        } => {
                            seen.lethal |= won(WinReason::HeroDown);
                            seen.hero_hit |= !won(WinReason::HeroDown);
                        }
                        Action::Attack { .. } => {}
                    }
                    seen.hero_riposte |= events
                        .iter()
                        .any(|e| matches!(e, Event::HeroRiposted { .. }));
                    seen.dead_attacker |= events.iter().any(|e| matches!(e, Event::Riposted { .. }))
                        && events.iter().any(|e| matches!(e, Event::Died { creature, .. } if Some(*creature) == attacker));
                }
                let playable: Vec<Action> = legal
                    .into_iter()
                    .filter(|a| *a != Action::Concede)
                    .collect();
                let action = playable[policy.random_range(0..playable.len())].clone();
                apply(&mut state, action).unwrap();
            }
        }
        assert!(seen.play, "no summoning");
        assert!(seen.multi_draw, "no multi-card draw");
        assert!(seen.lethal, "no lethal hero attack");
        assert!(seen.hero_hit, "no non-lethal hero attack");
        assert!(seen.dead_attacker, "no attacker dead of a riposte");
        assert!(seen.hero_riposte, "no hero riposte");
        assert!(seen.concede, "no concession");
    }

    #[test]
    fn a_lethal_attack_spends_no_time_and_does_not_check_exhaustion() {
        let mut state = with_boards(1, RuleSet::standard(), 1, 0);
        let active = state.active;
        state.player_mut(active.opponent()).health = 1;
        let attacker = state.player(active).board[0].id;
        let (budget, token) = (state.budget, state.active);
        let events = applied(
            &state,
            &Action::Attack {
                attacker,
                target: Target::Hero,
            },
        )
        .unwrap();
        assert_eq!(
            events.last(),
            Some(&Event::GameWon {
                winner: active,
                reason: WinReason::HeroDown
            })
        );
        assert!(!events.iter().any(|e| matches!(
            e,
            Event::TokenMoved { .. } | Event::ActivePlayerChanged { .. }
        )));
        let mut after = state.clone();
        apply(
            &mut after,
            Action::Attack {
                attacker,
                target: Target::Hero,
            },
        )
        .unwrap();
        assert_eq!((after.budget, after.active), (budget, token));
    }

    #[test]
    fn concede_does_not_check_exhaustion() {
        let mut state = with_boards(2, RuleSet::standard(), 0, 2);
        let active = state.active;
        let side = state.player_mut(active);
        side.hand.clear();
        side.hero.effects.clear();
        let events = applied(&state, &Action::Concede).unwrap();
        assert_eq!(
            events,
            vec![Event::GameWon {
                winner: active.opponent(),
                reason: WinReason::Concede
            }]
        );
    }

    #[test]
    fn a_dead_attacker_still_pays_its_attack_cost() {
        let mut state = with_boards(3, RuleSet::standard(), 1, 1);
        let active = state.active;
        let opp = active.opponent();
        state.player_mut(active).board[0].stats.health = 1;
        state.player_mut(active).board[0].stats.attack_cost = 2;
        state.player_mut(opp).board[0].stats.riposte = 3;
        state.player_mut(opp).board[0].stats.provocation = false;
        state.player_mut(opp).board[0].stats.health = 9;
        let attacker = state.player(active).board[0].id;
        let target = state.player(opp).board[0].id;
        let action = Action::Attack {
            attacker,
            target: Target::Creature(target),
        };
        let events = applied(&state, &action).unwrap();
        assert!(events.contains(&Event::Died {
            player: active,
            creature: attacker
        }));
        // The cost (2) is paid although the attacker died of the riposte: the time rule (a cost
        // below the budget keeps the token with the difference, otherwise it passes) decides.
        let budget = state.budget;
        let expected = if 2 < budget {
            Event::TokenMoved {
                player: active,
                budget: budget - 2,
            }
        } else {
            Event::TokenMoved {
                player: opp,
                budget: (2 - budget + 1).min(state.player(opp).hero.max_time),
            }
        };
        assert!(events.contains(&expected), "{events:?}");
    }

    #[test]
    fn a_force_zero_attacker_is_refused_whatever_the_target() {
        // #208: an effective force of 0 cannot attack (`attack_force_zero`, before the target
        // checks). `validate` refuses it and `legal_actions` never lists it.
        for seed in 0..5u64 {
            let mut state = with_boards(seed, rules_of(seed), 2, 1);
            let active = state.active;
            let attacker = {
                let side = state.player_mut(active);
                side.board[0].stats.force = 0;
                side.board[0].id
            };
            assert!(
                !legal_actions(&state)
                    .iter()
                    .any(|a| matches!(a, Action::Attack { attacker: id, .. } if *id == attacker)),
                "seed {seed}: the force-0 attacker is listed"
            );
            let unknown = Target::Creature(crate::CardInstanceId(9_999));
            for target in [Target::Hero, unknown] {
                assert_eq!(
                    crate::rules::validate(&state, &Action::Attack { attacker, target }),
                    Err(RuleError::AttackForceZero),
                    "seed {seed}"
                );
            }
            // The other creature of the board keeps attacking.
            let other = state.player(active).board[1].id;
            assert!(
                legal_actions(&state)
                    .iter()
                    .any(|a| matches!(a, Action::Attack { attacker: id, .. } if *id == other))
            );
        }
    }

    #[test]
    fn the_declared_effects_of_a_board_unit() {
        use crate::state::CardExtras;
        let mut state = with_boards(3, rules_of(3), 1, 0);
        let active = state.active;
        let id = state.player(active).board[0].id;
        let make = |trigger: &str| -> duel_cards::CardEffect {
            let cost = if trigger == "activated" {
                r#""cost":2,"#
            } else {
                ""
            };
            serde_json::from_str(&format!(
                r#"{{"trigger":"{trigger}",{cost}"ops":[{{"op":"draw","target":{{"select":"hero","side":"own"}},"count":1}}]}}"#
            ))
            .unwrap()
        };
        let continuous: duel_cards::CardEffect = serde_json::from_str(
            r#"{"trigger":"continuous","ops":[{"op":"grant_keyword","target":{"select":"self"},"keyword":"ranged"}]}"#,
        )
        .unwrap();
        // Index 0 is `on_play`, 2 `continuous` and index 1 `activated`: the trigger is what makes it activable.
        state.player_mut(active).board[0].extras = Some(std::sync::Arc::new(CardExtras {
            kind: crate::state::CardKind::Unit,
            immobile: false,
            tags: Vec::new(),
            effects: vec![make("on_play"), make("activated"), continuous],
        }));
        let activate = |index: u32| Action::Activate {
            effect: index,
            source: Some(id),
            picks: Vec::new(),
        };
        let judged = |action: &Action| crate::rules::validate(&state, action);
        // An `activated` effect is declared, not yet resolvable: the temporary code.
        assert_eq!(judged(&activate(1)), Err(RuleError::UnsupportedEffect));
        // An `on_play` or `continuous` effect is not activable, an index past the list is no
        // effect at all.
        assert_eq!(judged(&activate(0)), Err(RuleError::UnknownEffect));
        assert_eq!(judged(&activate(2)), Err(RuleError::UnknownEffect));
        assert_eq!(judged(&activate(3)), Err(RuleError::UnknownEffect));
        assert_eq!(judged(&activate(u32::MAX)), Err(RuleError::UnknownEffect));
        // A source that is not on the board: the same code whatever the zone, before the index.
        let elsewhere = state.player(active).hand[0].id;
        for source in [elsewhere, crate::CardInstanceId(9_999)] {
            let action = Action::Activate {
                effect: 1,
                source: Some(source),
                picks: Vec::new(),
            };
            assert_eq!(judged(&action), Err(RuleError::SourceNotOnBoard));
        }
        // None of it is a legal action.
        assert!(!legal_actions(&state).iter().any(|a| matches!(
            a,
            Action::Activate {
                source: Some(_),
                ..
            }
        )));
    }

    #[test]
    fn op_of_draw_carries_the_count_and_is_the_only_hero_sugar() {
        assert_eq!(op_of(&Effect::Draw { count: 3 }), Op::Draw { count: 3 });
        assert_eq!(op_of(&Effect::Draw { count: 1 }), Op::Draw { count: 1 });
    }

    /// A provocative game with `own` creatures on the active board and `opposing` on the other.
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
}
