use serde::{Deserialize, Serialize};

/// The hero embedded in a deck: the player's avatar, time budget and starting parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hero {
    pub name: String,
    pub max_time: u32,
    pub starting_hand: u32,
    pub initiative: u32,
    pub start_budget: u32,
    pub health: u32,
    /// What the hero can activate, in a stable order (drawing is one of them).
    pub effects: Vec<HeroEffect>,
}

/// An effect a hero can activate for `cost` time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeroEffect {
    pub name: String,
    pub cost: u32,
    pub effect: Effect,
}

/// What an activated effect does. The `kind` field discriminates the variant in JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    /// Moves up to `count` cards from the top of the draw pile to the hand.
    Draw { count: u32 },
}
