//! Resolution of an attack's damage: one isolated step, so a future rule that orders or
//! modifies damage (strike first, ...) changes [`plan_damage`] and [`apply_damage`] and
//! nothing else. The attack and the riposte are simultaneous: both amounts are planned from
//! the state as it is before the attack (a pure function), then applied, so a target killed
//! by the attack still ripostes and an attacker killed by the riposte still dealt its blow.
//!
//! # Termination
//!
//! Every game ends. Let `Φ` be the sum of the heroes' health plus, for every card, its
//! current health (`stats.health` of its instance, lowered by damage) and a bonus of 2 while
//! in a pile, 1 while in a hand, 0 on a board and, in a discard pile, nothing at all (a
//! dead creature has 0 health). Φ is a non-negative integer and every action strictly
//! decreases it:
//!
//! - a draw of `n` cards moves `n >= 1` cards from a pile to a hand: −`n`;
//! - a summoning moves one card from a hand to a board: −1;
//! - an attack removes `min(force, remaining health of the target) >= 1` from a hero or a
//!   creature (the excess damage is lost, so it removes nothing more);
//! - the riposte of the hero (`hero_riposte`, only against a non-lethal hit on a hero) removes
//!   in addition the whole remaining health of the attacker: a finite integer `>= 1` (a creature
//!   in play had `health >= 1` before the action), added to nothing, so Φ decreases strictly
//!   more than without the option; a lethal hit has no riposte and removes only its own
//!   `min(force, health) >= 1`;
//! - its riposte (S2b, only against a creature) removes `min(riposte, current health of the
//!   attacker) >= 0` more: finite health of a creature in play, never added to anything, so
//!   it can only lower Φ further, and is 0 at riposte 0 (the attack's `>= 1` alone is then
//!   enough);
//! - burying a dead creature, whichever side or role (target, attacker, both), removes
//!   nothing, its health being 0.
//!
//! Provocation (S2b) and the board limit (#171) only restrict the set of legal actions (the
//! attacks that may be aimed, the summonings that may be played): neither adds an action nor
//! changes an effect, so every action that remains decreases Φ exactly as above.
//!
//! (The first potential of the Epic, `2·|piles| + |hands| + Σ health`, broke once creatures
//! counted: a summoning added the creature's health, and "−force" ignored the lost excess.)
//! This relies on `force >= 1`, `health >= 1`, every cost `>= 1`, every draw `count >= 1` and
//! `riposte` a finite integer (bounded by `MAX_STAT`), all guaranteed by the validation
//! `new_game` replays (`duel-cards`), and on there being no healing, no riposte that heals,
//! and no card leaving a discard pile or returning to a pile, a hand or a board. Any rule
//! adding one of those (a heal, a return to hand, a recycling) must re-examine this.

use crate::effects::effective;
use crate::rules::{Event, Target};
use crate::state::{CardInstance, CardInstanceId, GameState, PlayerId};

/// The damage of one attack, computed before anything is applied: what the target takes and
/// what the attacker takes back (the target's riposte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttackDamage {
    /// The owner of the attacker.
    pub(crate) player: PlayerId,
    pub(crate) attacker: CardInstanceId,
    pub(crate) target: Target,
    /// The attacker's force: nominal, the excess over the target's health is lost.
    pub(crate) on_target: u32,
    /// The target's riposte: always 0 for a hero (the hero's own riposte is `attacker_slain`).
    pub(crate) on_attacker: u32,
    /// The hero's riposte: the attack is on a hero, the option is on and the hit does not kill
    /// it (`hero health > force`, read before the attack): the attacker is destroyed.
    pub(crate) attacker_slain: bool,
}

/// Step 1, pure: both damages of `attacker` (on `player`'s board) against `target`, read from
/// the state as it is before the attack. The caller has validated the attack.
pub(crate) fn plan_damage(
    state: &GameState,
    player: PlayerId,
    attacker: CardInstanceId,
    target: Target,
) -> AttackDamage {
    let on_target = state
        .player(player)
        .board
        .iter()
        .find(|c| c.id == attacker)
        .map_or(0, |c| effective(state, c).force);
    debug_assert!(on_target >= 1, "validated attacker with force >= 1");
    // A hero has no riposte; a creature hits back with its own, even if this attack kills it.
    let on_attacker = match target {
        Target::Hero => 0,
        Target::Creature(creature) => state
            .player(player.opponent())
            .board
            .iter()
            .find(|c| c.id == creature)
            .map_or(0, |c| effective(state, c).riposte),
    };
    // The one place that decides "lethal" (`health <= force`): a lethal hit has no riposte.
    let attacker_slain = matches!(target, Target::Hero)
        && state.rules().hero_riposte
        && state.player(player.opponent()).health > on_target;
    AttackDamage {
        player,
        attacker,
        target,
        on_target,
        on_attacker,
        attacker_slain,
    }
}

/// Step 2: applies both damages of `plan`, saturating at 0 health, and logs them.
pub(crate) fn apply_damage(state: &mut GameState, plan: AttackDamage, events: &mut Vec<Event>) {
    let victim = plan.player.opponent();
    match plan.target {
        Target::Hero => {
            let side = state.player_mut(victim);
            side.health = side.health.saturating_sub(plan.on_target);
            events.push(Event::Damaged {
                player: victim,
                amount: plan.on_target,
            });
            if plan.attacker_slain {
                // Infinite riposte: the attacker's health does not matter. `bury_dead` buries it.
                if let Some(attacker) = state
                    .player_mut(plan.player)
                    .board
                    .iter_mut()
                    .find(|c| c.id == plan.attacker)
                {
                    attacker.stats.health = 0;
                    events.push(Event::HeroRiposted {
                        player: plan.player,
                        creature: plan.attacker,
                    });
                } else {
                    debug_assert!(false, "validated attacker on its board");
                }
            }
        }
        Target::Creature(creature) => {
            let Some(target) = state
                .player_mut(victim)
                .board
                .iter_mut()
                .find(|c| c.id == creature)
            else {
                debug_assert!(false, "validated target on the opposing board");
                return;
            };
            target.stats.health = target.stats.health.saturating_sub(plan.on_target);
            events.push(Event::CreatureDamaged {
                player: victim,
                creature,
                amount: plan.on_target,
            });
            if plan.on_attacker >= 1 {
                // The attacker is on the active player's board, a different creature from
                // the target; a missing one (unreachable once validated) is simply skipped.
                if let Some(attacker) = state
                    .player_mut(plan.player)
                    .board
                    .iter_mut()
                    .find(|c| c.id == plan.attacker)
                {
                    attacker.stats.health = attacker.stats.health.saturating_sub(plan.on_attacker);
                    events.push(Event::Riposted {
                        player: plan.player,
                        creature: plan.attacker,
                        amount: plan.on_attacker,
                    });
                } else {
                    debug_assert!(false, "validated attacker on its board");
                }
            }
        }
    }
}

/// Resolves the damage of an attack: plan, then apply.
pub(crate) fn resolve_damage(
    state: &mut GameState,
    player: PlayerId,
    attacker: CardInstanceId,
    target: Target,
    events: &mut Vec<Event>,
) {
    let plan = plan_damage(state, player, attacker, target);
    apply_damage(state, plan, events);
}

/// Moves every creature at 0 health from its board to its owner's discard pile and logs a
/// `Died` for each, in ascending instance id order whatever the board or the side (the
/// order of death when several die at once). Surviving creatures keep their board order.
pub(crate) fn bury_dead(state: &mut GameState, events: &mut Vec<Event>) {
    let mut dead: Vec<(CardInstance, PlayerId)> = Vec::new();
    for player in [PlayerId::One, PlayerId::Two] {
        let board = std::mem::take(&mut state.player_mut(player).board);
        let (fallen, alive): (Vec<_>, Vec<_>) =
            board.into_iter().partition(|c| c.stats.health == 0);
        state.player_mut(player).board = alive;
        dead.extend(fallen.into_iter().map(|c| (c, player)));
    }
    dead.sort_by_key(|(card, _)| card.id);
    for (card, player) in dead {
        events.push(Event::Died {
            player,
            creature: card.id,
        });
        state.player_mut(player).discard.push(card);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_game;
    use crate::state::CardInstance;
    use crate::testing::{draw, hero, setup, stats_with};

    /// A running game with `attacker` (id 100, on One's board) and `target` (id 101, on Two's).
    fn facing(attacker: (u32, u32, u32), target: (u32, u32, u32)) -> GameState {
        let (mut state, _) = new_game(
            setup(
                hero(2, 8, 8, vec![draw(1, 1)]),
                hero(1, 2, 5, vec![draw(1, 1)]),
            ),
            3,
        )
        .unwrap();
        let card = |id, (force, health, riposte)| CardInstance {
            id: CardInstanceId(id),
            stats: stats_with(1, force, 1, health, false, riposte),
            name: None,
        };
        state.one.board = vec![card(100, attacker)];
        state.two.board = vec![card(101, target)];
        state
    }

    #[test]
    fn plan_damage_reads_force_and_riposte_nominally_and_never_touches_the_state() {
        // (attacker force, attacker health, target health, target riposte) -> (on_target, on_attacker)
        let table = [
            (1, 1, 1, 0, 1, 0),
            (2, 5, 5, 3, 2, 3),
            (3, 2, 1, 4, 3, 4), // excess on both sides stays nominal
            (4, 1, 9, 0, 4, 0),
            (1, 3, 3, 9, 1, 9),
        ];
        for (force, own, theirs, riposte, on_target, on_attacker) in table {
            let state = facing((force, own, 0), (1, theirs, riposte));
            let before = state.clone();
            let plan = plan_damage(
                &state,
                PlayerId::One,
                CardInstanceId(100),
                Target::Creature(CardInstanceId(101)),
            );
            assert_eq!(state, before, "planning must not mutate the state");
            assert_eq!(
                plan,
                AttackDamage {
                    player: PlayerId::One,
                    attacker: CardInstanceId(100),
                    target: Target::Creature(CardInstanceId(101)),
                    on_target,
                    on_attacker,
                    attacker_slain: false,
                },
                "force {force}, riposte {riposte}"
            );
        }
    }

    #[test]
    fn plan_damage_against_a_hero_has_no_riposte_even_with_ripostants_on_the_board() {
        let state = facing((2, 5, 0), (1, 5, 7));
        let before = state.clone();
        let plan = plan_damage(&state, PlayerId::One, CardInstanceId(100), Target::Hero);
        assert_eq!(state, before);
        assert_eq!((plan.on_target, plan.on_attacker), (2, 0));
    }

    #[test]
    fn apply_damage_applies_the_plan_it_is_given_not_a_recomputed_one() {
        // A hand-made plan: the damage comes from the plan alone.
        let mut state = facing((2, 5, 0), (1, 5, 7));
        let plan = AttackDamage {
            player: PlayerId::One,
            attacker: CardInstanceId(100),
            target: Target::Creature(CardInstanceId(101)),
            on_target: 1,
            on_attacker: 2,
            attacker_slain: false,
        };
        let mut events = Vec::new();
        apply_damage(&mut state, plan, &mut events);
        assert_eq!(state.creature_health(CardInstanceId(101)), Some(4));
        assert_eq!(state.creature_health(CardInstanceId(100)), Some(3));
        assert_eq!(events.len(), 2);
    }

    fn with_hero_riposte(mut state: GameState, hero_riposte: bool) -> GameState {
        state.rules = crate::RuleSet {
            hero_riposte,
            ..crate::RuleSet::legacy()
        };
        state
    }

    #[test]
    fn plan_damage_slays_the_attacker_of_a_non_lethal_hit_on_the_hero_only_with_the_option() {
        // The hero (One's opponent: Two) has 8 health in `facing`; the attacker has force 2.
        for (option, health, slain) in [
            (true, 8, true),
            (true, 3, true),
            (false, 8, false),
            // Lethal (health <= force, read before the attack): never a riposte.
            (true, 2, false),
            (true, 1, false),
        ] {
            let mut state = with_hero_riposte(facing((2, 5, 0), (1, 5, 0)), option);
            state.two.health = health;
            let before = state.clone();
            let plan = plan_damage(&state, PlayerId::One, CardInstanceId(100), Target::Hero);
            assert_eq!(state, before, "planning must not mutate the state");
            assert_eq!(
                plan.attacker_slain, slain,
                "option {option}, health {health}"
            );
            assert_eq!((plan.on_target, plan.on_attacker), (2, 0));
        }
    }

    #[test]
    fn plan_damage_never_slays_the_attacker_of_an_attack_on_a_creature() {
        let state = with_hero_riposte(facing((2, 5, 0), (1, 5, 0)), true);
        let plan = plan_damage(
            &state,
            PlayerId::One,
            CardInstanceId(100),
            Target::Creature(CardInstanceId(101)),
        );
        assert!(!plan.attacker_slain);
    }

    #[test]
    fn apply_damage_slays_the_attacker_the_plan_names_and_logs_it_after_damaged() {
        let mut state = with_hero_riposte(facing((2, 9, 0), (1, 5, 0)), true);
        let plan = plan_damage(&state, PlayerId::One, CardInstanceId(100), Target::Hero);
        assert!(plan.attacker_slain);
        let mut events = Vec::new();
        apply_damage(&mut state, plan, &mut events);
        assert_eq!(
            events,
            vec![
                Event::Damaged {
                    player: PlayerId::Two,
                    amount: 2
                },
                Event::HeroRiposted {
                    player: PlayerId::One,
                    creature: CardInstanceId(100)
                },
            ]
        );
        assert_eq!(state.creature_health(CardInstanceId(100)), Some(0));
        assert_eq!(state.two.health, 8);
    }
}
