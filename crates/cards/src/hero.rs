use serde::{Deserialize, Serialize};

/// The hero embedded in a deck: the player's avatar, time budget and starting parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hero {
    pub name: String,
    pub max_time: u32,
    pub draw_cost: u32,
    pub starting_hand: u32,
    pub initiative: u32,
    pub start_budget: u32,
    pub health: u32,
}
