//! Fixtures shared by the in-crate tests.

use crate::state::{CardInstance, GameState, PlayerState};
use crate::{GameSetup, PlayerSetup};
use duel_cards::{CreatureStats, Effect, Hero, HeroEffect};

pub(crate) fn draw(cost: u32, count: u32) -> HeroEffect {
    HeroEffect {
        name: format!("draw {count} for {cost}"),
        cost,
        effect: Effect::Draw { count },
    }
}

pub(crate) fn hero(
    initiative: u32,
    start_budget: u32,
    max_time: u32,
    effects: Vec<HeroEffect>,
) -> Hero {
    Hero {
        name: format!("hero-{initiative}"),
        max_time,
        starting_hand: 4,
        initiative,
        start_budget,
        health: 10,
        effects,
    }
}

/// 30 distinguishable cards.
pub(crate) fn deck() -> Vec<CreatureStats> {
    (1..=30)
        .map(|n| CreatureStats {
            cost: n,
            force: 1 + n % 3,
            attack_cost: 1 + n % 2,
            health: 1 + n % 4,
        })
        .collect()
}

pub(crate) fn setup(one: Hero, two: Hero) -> GameSetup {
    GameSetup {
        one: PlayerSetup {
            hero: one,
            cards: deck(),
        },
        two: PlayerSetup {
            hero: two,
            cards: deck(),
        },
        data_versions: vec!["fixture@1.0.0".into()],
    }
}

/// The termination potential Φ of `combat.rs`: the heroes' health plus, per card, its
/// current health and 2 in a pile, 1 in a hand, 0 more on a board, nothing in a discard.
pub(crate) fn potential(state: &GameState) -> u64 {
    let side = |p: &PlayerState| {
        let health = |cards: &[CardInstance], extra: u64| {
            cards
                .iter()
                .map(|c| u64::from(c.stats.health) + extra)
                .sum::<u64>()
        };
        u64::from(p.health) + health(&p.deck, 2) + health(&p.hand, 1) + health(&p.board, 0)
    };
    side(&state.one) + side(&state.two)
}
