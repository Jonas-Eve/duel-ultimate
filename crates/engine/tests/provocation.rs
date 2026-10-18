//! S2b-T2: provocation filters the targets of an attack, in `apply` and `legal_actions`
//! alike. Boards are built through `new_game` and `Play` only (public API).
//!
//! Card ids are assigned in setup order, One's deck first (0..30) then Two's (30..60), so a
//! test knows which card is which whatever the shuffle.

use duel_cards::{CreatureStats, Hero};
use duel_engine::{
    Action, CardInstanceId, Event, GameSetup, GameState, PlayerId, PlayerSetup, RuleError, RuleSet,
    Target, apply, legal_actions, new_game,
};

/// Force 1, summoning and attack cost 1, no riposte.
fn creature(health: u32, provocation: bool) -> CreatureStats {
    CreatureStats {
        cost: 1,
        force: 1,
        attack_cost: 1,
        health,
        provocation,
        riposte: 0,
    }
}

fn plain(health: u32) -> CreatureStats {
    creature(health, false)
}

fn taunt(health: u32) -> CreatureStats {
    creature(health, true)
}

fn id(n: u32) -> CardInstanceId {
    CardInstanceId(n)
}

fn hero(initiative: u32, start_budget: u32, starting_hand: u32) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time: 30,
        starting_hand,
        initiative,
        start_budget,
        health: 10,
        effects: vec![],
    }
}

/// A duel frozen right after both sides built their boards.
struct Arena {
    state: GameState,
    /// One's board, in order of summoning (ids 0..).
    one: Vec<CardInstanceId>,
    /// Two's board, in order of summoning (ids 30..).
    two: Vec<CardInstanceId>,
    /// A provocateur still in Two's hand, ready to be summoned.
    spare: CardInstanceId,
    /// A card in Two's pile (hidden), if `pile > 0`.
    piled: Option<CardInstanceId>,
}

/// One summons `one` (ids 0..), Two summons `two` (ids 30..), then One is active with `k`
/// time units (an attack costs 1: the first `k - 1` keep the token). Every
/// other card of One is a plain creature, every other card of Two a provocateur (the
/// `spare`). `pile` cards of each deck stay out of the starting hand; the seed is picked so
/// that every card to be summoned is in its owner's hand.
fn arena(one: &[CreatureStats], two: &[CreatureStats], k: u32, pile: u32) -> Arena {
    let (n, m) = (one.len() as u32, two.len() as u32);
    assert!(n >= 1 && m >= 1 && n < 30 && m < 29 && k >= 1);
    // One's last summoning costs m (Two then receives m), Two's last costs k.
    let mut one_cards: Vec<CreatureStats> = one.to_vec();
    one_cards[(n - 1) as usize].cost = m;
    one_cards.resize(30, plain(3));
    let mut two_cards: Vec<CreatureStats> = two.to_vec();
    two_cards[(m - 1) as usize].cost = k;
    two_cards.resize(30, taunt(3));
    let spare = id(30 + m);
    for seed in 0..500 {
        let (mut state, _) = new_game(
            GameSetup {
                one: PlayerSetup {
                    hero: hero(2, n, 30 - pile),
                    cards: one_cards.clone(),
                    card_names: Vec::new(),
                },
                two: PlayerSetup {
                    hero: hero(1, 1, 30 - pile),
                    cards: two_cards.clone(),
                    card_names: Vec::new(),
                },
                data_versions: vec!["fixture@1.0.0".into()],
                rules: RuleSet::default(),
            },
            seed,
        )
        .unwrap();
        let wanted_one: Vec<_> = (0..n).map(id).collect();
        let wanted_two: Vec<_> = (30..30 + m).map(id).chain([spare]).collect();
        let (hand_one, hand_two) = (state.hand(PlayerId::One), state.hand(PlayerId::Two));
        if !wanted_one.iter().all(|c| hand_one.contains(c))
            || !wanted_two.iter().all(|c| hand_two.contains(c))
        {
            continue;
        }
        for card in &wanted_one {
            apply(&mut state, Action::Play { card: *card }).unwrap();
        }
        for card in &wanted_two[..m as usize] {
            apply(&mut state, Action::Play { card: *card }).unwrap();
        }
        assert_eq!((state.active(), state.budget()), (PlayerId::One, k));
        let board_two = state.board(PlayerId::Two);
        let piled = (30..60)
            .map(id)
            .find(|c| !board_two.contains(c) && !state.hand(PlayerId::Two).contains(c));
        assert_eq!(piled.is_some(), pile > 0);
        return Arena {
            state,
            one: wanted_one,
            two: wanted_two[..m as usize].to_vec(),
            spare,
            piled,
        };
    }
    panic!("no seed puts every needed card in hand");
}

fn strike(attacker: CardInstanceId, target: Target) -> Action {
    Action::Attack { attacker, target }
}

fn hit(attacker: CardInstanceId, victim: CardInstanceId) -> Action {
    strike(attacker, Target::Creature(victim))
}

/// The targets offered to each attacker by `legal_actions`, in listing order.
fn offered(state: &GameState) -> Vec<(CardInstanceId, Vec<Target>)> {
    let mut out: Vec<(CardInstanceId, Vec<Target>)> = Vec::new();
    for action in legal_actions(state) {
        if let Action::Attack { attacker, target } = action {
            match out.last_mut() {
                Some((a, targets)) if *a == attacker => targets.push(target),
                _ => out.push((attacker, vec![target])),
            }
        }
    }
    out
}

fn creatures(ids: &[CardInstanceId]) -> Vec<Target> {
    ids.iter().copied().map(Target::Creature).collect()
}

/// Refused with `error`, nothing changed (the generator included, `GameState: PartialEq`).
fn assert_refused(state: &GameState, action: Action, error: RuleError) {
    let mut clone = state.clone();
    assert_eq!(apply(&mut clone, action), Err(error), "{action:?}");
    assert_eq!(&clone, state, "{action:?} changed the state");
}

// ---- legal_actions ---------------------------------------------------------------------------

#[test]
fn a_single_provocateur_is_the_only_target_of_every_attacker_in_summoning_order() {
    let a = arena(
        &[plain(3), plain(3), plain(3)],
        &[plain(3), taunt(3), plain(3)],
        2,
        0,
    );
    let provocateur = a.two[1];
    assert_eq!(
        offered(&a.state),
        vec![
            (a.one[0], creatures(&[provocateur])),
            (a.one[1], creatures(&[provocateur])),
            (a.one[2], creatures(&[provocateur])),
        ],
        "neither the hero nor the other creatures, one list per attacker in summoning order"
    );
}

#[test]
fn several_provocateurs_are_all_offered_in_board_order() {
    // Layouts of Two's board (true = provocateur), at most five creatures (the standard board
    // limit): the provocateurs are not contiguous and not always at the ends.
    for layout in [
        &[false, true, false, true][..],
        &[true, false, true, false, true][..],
    ] {
        let two: Vec<_> = layout
            .iter()
            .map(|&taunting| if taunting { taunt(3) } else { plain(3) })
            .collect();
        let a = arena(&[plain(3), plain(3)], &two, 2, 0);
        let provocateurs: Vec<_> = (0..layout.len())
            .filter(|&i| layout[i])
            .map(|i| a.two[i])
            .collect();
        assert_eq!(
            offered(&a.state),
            vec![
                (a.one[0], creatures(&provocateurs)),
                (a.one[1], creatures(&provocateurs)),
            ],
            "{layout:?}"
        );
    }
}

#[test]
fn without_a_provocateur_every_target_is_offered_as_in_s2a() {
    let a = arena(&[plain(3)], &[plain(3), plain(3)], 2, 0);
    let mut expected = vec![Target::Hero];
    expected.extend(creatures(&a.two));
    assert_eq!(offered(&a.state), vec![(a.one[0], expected)]);
}

// ---- apply: the hero and the dying provocateur -----------------------------------------------

#[test]
fn the_hero_is_untargetable_until_the_last_provocateur_dies() {
    let mut a = arena(&[plain(3), plain(3), plain(3)], &[taunt(1), taunt(1)], 6, 0);
    let (p, q) = (a.two[0], a.two[1]);
    let s = &mut a.state;
    assert_refused(
        s,
        strike(a.one[0], Target::Hero),
        RuleError::TargetForbiddenByProvocation,
    );
    apply(s, hit(a.one[0], p)).unwrap();
    assert_eq!(
        s.board(PlayerId::Two),
        vec![q],
        "the first provocateur died"
    );
    assert_refused(
        s,
        strike(a.one[1], Target::Hero),
        RuleError::TargetForbiddenByProvocation,
    );
    assert!(
        !legal_actions(s).contains(&strike(a.one[1], Target::Hero)),
        "legal_actions agrees"
    );
    apply(s, hit(a.one[1], q)).unwrap();
    assert!(s.board(PlayerId::Two).is_empty());
    assert!(legal_actions(s).contains(&strike(a.one[2], Target::Hero)));
    let events = apply(s, strike(a.one[2], Target::Hero)).unwrap();
    assert!(events.contains(&Event::Damaged {
        player: PlayerId::Two,
        amount: 1
    }));
}

#[test]
fn a_creature_refused_behind_a_provocateur_is_attackable_once_it_dies() {
    let mut a = arena(&[plain(3), plain(3)], &[taunt(1), plain(3)], 4, 0);
    let (p, x) = (a.two[0], a.two[1]);
    let s = &mut a.state;
    assert_refused(s, hit(a.one[0], x), RuleError::TargetForbiddenByProvocation);
    apply(s, hit(a.one[0], p)).unwrap();
    assert_eq!(s.discard(PlayerId::Two), vec![p]);
    assert!(legal_actions(s).contains(&hit(a.one[1], x)));
    apply(s, hit(a.one[1], x)).unwrap();
    assert_eq!(s.creature_health(x), Some(2));
}

#[test]
fn a_wounded_provocateur_still_provokes() {
    let mut a = arena(&[plain(3), plain(3)], &[taunt(3), plain(3)], 4, 0);
    let p = a.two[0];
    let s = &mut a.state;
    apply(s, hit(a.one[0], p)).unwrap();
    assert_eq!(s.creature_health(p), Some(2));
    assert_eq!(offered(s)[0].1, creatures(&[p]));
    assert_refused(
        s,
        strike(a.one[1], Target::Hero),
        RuleError::TargetForbiddenByProvocation,
    );
}

#[test]
fn a_provocateur_summoned_between_two_attacks_restricts_at_once() {
    let mut a = arena(&[plain(3)], &[plain(3)], 1, 0);
    let (mine, theirs, spare) = (a.one[0], a.two[0], a.spare);
    let s = &mut a.state;
    assert!(legal_actions(s).contains(&strike(mine, Target::Hero)));
    apply(s, hit(mine, theirs)).unwrap(); // hands the token over (cost 1 = budget 1)
    assert_eq!(s.active(), PlayerId::Two);
    apply(s, Action::Play { card: spare }).unwrap(); // a provocateur, no summoning sickness
    assert_eq!(s.active(), PlayerId::One);
    assert_eq!(offered(s), vec![(mine, creatures(&[spare]))]);
    assert_refused(
        s,
        strike(mine, Target::Hero),
        RuleError::TargetForbiddenByProvocation,
    );
    assert_refused(
        s,
        hit(mine, theirs),
        RuleError::TargetForbiddenByProvocation,
    );
}

// ---- both sides ------------------------------------------------------------------------------

#[test]
fn only_the_opponents_provocation_constrains_a_side() {
    let mut a = arena(&[plain(3), taunt(3)], &[taunt(3), plain(3)], 1, 0);
    let (a0, a1) = (a.one[0], a.one[1]);
    let (p, x) = (a.two[0], a.two[1]);
    let s = &mut a.state;
    // One's creatures, provocateur included, may only target Two's provocateur.
    assert_eq!(
        offered(s),
        vec![(a0, creatures(&[p])), (a1, creatures(&[p]))]
    );
    apply(s, hit(a1, p)).unwrap(); // the provocateur attacks freely (a provocateur target)
    assert_eq!(s.active(), PlayerId::Two);
    // Two's creatures may only target One's provocateur.
    assert_eq!(
        offered(s),
        vec![(p, creatures(&[a1])), (x, creatures(&[a1]))]
    );
    assert_refused(
        s,
        strike(p, Target::Hero),
        RuleError::TargetForbiddenByProvocation,
    );
    assert_refused(s, hit(x, a0), RuleError::TargetForbiddenByProvocation);
}

#[test]
fn a_provocateur_attacks_the_hero_when_the_opponent_has_none() {
    let mut a = arena(&[plain(3)], &[taunt(3)], 1, 0);
    let (mine, p) = (a.one[0], a.two[0]);
    let s = &mut a.state;
    apply(s, hit(mine, p)).unwrap();
    assert_eq!(s.active(), PlayerId::Two);
    assert!(legal_actions(s).contains(&strike(p, Target::Hero)));
    let events = apply(s, strike(p, Target::Hero)).unwrap();
    assert!(events.contains(&Event::Damaged {
        player: PlayerId::One,
        amount: 1
    }));
}

// ---- refusals --------------------------------------------------------------------------------

#[test]
fn the_error_has_its_own_stable_code() {
    let e = RuleError::TargetForbiddenByProvocation;
    assert_eq!(e.code(), "target_forbidden_by_provocation");
    assert_ne!(e.code(), RuleError::TargetNotOnBoard.code());
}

#[test]
fn the_hero_and_a_plain_creature_are_refused_atomically() {
    let a = arena(&[plain(3)], &[taunt(3), plain(3)], 2, 0);
    assert_refused(
        &a.state,
        strike(a.one[0], Target::Hero),
        RuleError::TargetForbiddenByProvocation,
    );
    assert_refused(
        &a.state,
        hit(a.one[0], a.two[1]),
        RuleError::TargetForbiddenByProvocation,
    );
}

#[test]
fn the_attacker_is_checked_before_the_provocation() {
    let a = arena(&[plain(3)], &[taunt(3), plain(3)], 2, 0);
    for attacker in [a.two[0], a.spare, id(9999), id(1)] {
        for target in [Target::Hero, Target::Creature(a.two[1])] {
            assert_refused(
                &a.state,
                strike(attacker, target),
                RuleError::AttackerNotOnBoard,
            );
        }
    }
}

#[test]
fn a_target_off_the_opposing_board_is_not_on_board_even_with_a_provocateur() {
    // One's own creature, the opponent's hand, pile and discard, an unknown id: all the same
    // error, so the provocation is no oracle on hidden zones.
    let mut a = arena(&[plain(3), plain(3)], &[taunt(1), taunt(3), plain(3)], 3, 1);
    let piled = a.piled.unwrap();
    apply(&mut a.state, hit(a.one[0], a.two[0])).unwrap();
    let dead = a.two[0];
    assert_eq!(a.state.discard(PlayerId::Two), vec![dead]);
    assert!(
        a.state.board(PlayerId::Two).contains(&a.two[1]),
        "a provocateur remains"
    );
    let in_hand = a.spare;
    assert!(a.state.hand(PlayerId::Two).contains(&in_hand));
    for target in [a.one[1], in_hand, piled, dead, id(9999)] {
        assert_refused(&a.state, hit(a.one[1], target), RuleError::TargetNotOnBoard);
    }
    assert!(a.state.winner().is_none());
}
