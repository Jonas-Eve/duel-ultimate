//! Fixtures shared by the in-crate tests.

use crate::state::{CardInstance, GameState, PlayerState};
use crate::{GameSetup, PlayerSetup, RuleSet};
use duel_cards::{CardSpec, CreatureStats, Effect, Hero, HeroEffect};

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

/// Stats of a creature without `provocation` or `riposte` (what every S1/S2a card is).
pub(crate) fn stats(cost: u32, force: u32, attack_cost: u32, health: u32) -> CreatureStats {
    stats_with(cost, force, attack_cost, health, false, 0)
}

/// Stats of a creature, `provocation` and `riposte` included (provocation is read by the engine, riposte by S2b-T3).
pub(crate) fn stats_with(
    cost: u32,
    force: u32,
    attack_cost: u32,
    health: u32,
    provocation: bool,
    riposte: u32,
) -> CreatureStats {
    CreatureStats {
        cost,
        force,
        attack_cost,
        health,
        provocation,
        riposte,
    }
}

/// 30 distinguishable cards.
pub(crate) fn deck() -> Vec<CardSpec> {
    (1..=30)
        .map(|n| stats(n, 1 + n % 3, 1 + n % 2, 1 + n % 4).into())
        .collect()
}

pub(crate) fn setup(one: Hero, two: Hero) -> GameSetup {
    GameSetup {
        one: PlayerSetup {
            hero: one,
            cards: deck(),
            card_names: Vec::new(),
        },
        two: PlayerSetup {
            hero: two,
            cards: deck(),
            card_names: Vec::new(),
        },
        data_versions: vec!["fixture@1.0.0".into()],
        rules: RuleSet::default(),
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
