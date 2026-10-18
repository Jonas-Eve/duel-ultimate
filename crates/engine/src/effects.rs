//! The effects of the game: what a source carries, the single legality predicate [`usable`], the
//! listing of the legal actions derived from the effects present, and the resolution of hero
//! effects (one isolated step, so new effects are new variants).
//!
//! The read side (#183): [`effects_of`] is the one accessor of the effects a source carries,
//! derived on read, so a later change of their storage touches nothing else. [`sources`] lists
//! every candidate [`Use`] in the order of the legal actions, [`usable`] is the only judge of
//! legality (called by `validate`, `legal_actions` and `check_exhaustion`), and [`decode`] /
//! [`encode`] turn the wire [`Action`] into a [`Use`] and back. The attack is an intrinsic
//! effect, synthesized for every creature of the board (never a line of data); a creature
//! declares no effect at this stage.

use crate::rules::{
    Action, Event, Pick, RuleError, Target, board_position, hand_position, hero_effect,
    opposing_board_position, play_blocker, provocation_allows,
};
use crate::state::{CardInstanceId, GameState, PlayerId};
use duel_cards::Effect;

/// Where an effect comes from: a card of the active hand, a creature of the active board, or the
/// active hero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Hand(CardInstanceId),
    Board(CardInstanceId),
    Hero,
}

/// Which effect of a source: summoning a card, the intrinsic attack of a creature, or the effect
/// of the hero's list at this index (a creature declares none).
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
/// effects of the hero's list. A creature declares none.
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
/// are judged before, by `validate`). An attack adds no check of the force of the attacker.
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
            hand_position(state, card).ok_or(RuleError::CardNotInHand)?;
            // No summoning takes a choice at this stage.
            if !picks.is_empty() {
                return Err(RuleError::InvalidPicks);
            }
            play_blocker(state, state.active).map_or(Ok(()), Err)
        }
        (Source::Board(attacker), EffectRef::Attack, Choice::Target(target)) => {
            // Order of the checks: the attacker, then the target on the opposing board, then
            // the provocation. Every id that is not an opposing creature gets `TargetNotOnBoard`
            // whatever zone (hidden or not) it is in, even when a provocateur exists: the
            // provocation error only ever speaks of the public board.
            board_position(state, attacker).ok_or(RuleError::AttackerNotOnBoard)?;
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
        (Source::Board(creature), EffectRef::Declared(_), _) => {
            // One code for every id that is not a creature of my board (hand, pile, discard,
            // opposing board, unknown): no hidden zone is told apart.
            board_position(state, creature).ok_or(RuleError::SourceNotOnBoard)?;
            // A creature declares no effect at this stage: the board carries none to find.
            if effects_of(state, source).any(|carried| carried == effect) {
                Ok(())
            } else {
                Err(RuleError::UnknownEffect)
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

    /// What `apply` and the frozen reference each give on a clone of `state`: the result, then the
    /// state, which must agree to the byte (generator included).
    fn both(state: &GameState, action: &Action) -> Result<Vec<Event>, RuleError> {
        let (mut new, mut old) = (state.clone(), state.clone());
        let got = apply(&mut new, action.clone());
        let expected = crate::reference::apply(&mut old, action.clone());
        assert_eq!(got, expected, "{action:?}");
        assert_eq!(new, old, "{action:?}");
        got
    }

    /// An attack by a creature of force 0 is only judged by `validate` (`plan_damage`
    /// debug-asserts a force of 1).
    fn is_force_zero_attack(state: &GameState, action: &Action) -> bool {
        matches!(action, Action::Attack { attacker, .. }
            if state.player(state.active).board.iter().any(|c| c.id == *attacker && c.stats.force == 0))
    }

    #[test]
    fn the_journal_order_of_every_operation_is_unchanged() {
        #[derive(Default)]
        struct Seen {
            play: bool,
            multi_draw: bool,
            lethal: bool,
            hero_hit: bool,
            dead_attacker: bool,
            hero_riposte: bool,
            concede: bool,
            hand_over_exhaustion: bool,
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
                    let events = both(&state, action).unwrap();
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
                    seen.hand_over_exhaustion |= won(WinReason::Exhaustion)
                        && events
                            .iter()
                            .any(|e| matches!(e, Event::ActivePlayerChanged { .. }));
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
        let _ = seen.hand_over_exhaustion;
    }

    #[test]
    fn a_lethal_attack_spends_no_time_and_does_not_check_exhaustion() {
        let mut state = with_boards(1, RuleSet::standard(), 1, 0);
        let active = state.active;
        state.player_mut(active.opponent()).health = 1;
        let attacker = state.player(active).board[0].id;
        let (budget, token) = (state.budget, state.active);
        let events = both(
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
        let events = both(&state, &Action::Concede).unwrap();
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
        let events = both(&state, &action).unwrap();
        assert!(events.contains(&Event::Died {
            player: active,
            creature: attacker
        }));
        assert!(events.iter().any(|e| matches!(e, Event::TokenMoved { .. })));
        let mut after = state.clone();
        apply(&mut after, action).unwrap();
        let mut reference = state.clone();
        crate::reference::apply(
            &mut reference,
            Action::Attack {
                attacker,
                target: Target::Creature(target),
            },
        )
        .unwrap();
        assert_eq!(after.budget, reference.budget);
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
