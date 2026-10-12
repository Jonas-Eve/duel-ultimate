use serde::{Deserialize, Serialize};

/// A card definition. The `kind` field discriminates the variant in JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Card {
    Creature {
        id: String,
        name: String,
        cost: u32,
        force: u32,
        attack_cost: u32,
        health: u32,
    },
}

impl Card {
    pub fn id(&self) -> &str {
        match self {
            Card::Creature { id, .. } => id,
        }
    }

    /// The numeric stats of the card, as the engine consumes them.
    pub fn stats(&self) -> CreatureStats {
        match self {
            Card::Creature {
                cost,
                force,
                attack_cost,
                health,
                ..
            } => CreatureStats {
                cost: *cost,
                force: *force,
                attack_cost: *attack_cost,
                health: *health,
            },
        }
    }
}

/// Resolved numeric stats of a creature card (what the engine copies into a game).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatureStats {
    pub cost: u32,
    pub force: u32,
    pub attack_cost: u32,
    pub health: u32,
}
