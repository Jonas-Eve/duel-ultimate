use crate::hero::Hero;
use serde::{Deserialize, Serialize};

/// A preconstructed deck: a hero and the ids of its cards (ids may repeat).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deck {
    pub id: String,
    pub name: String,
    pub hero: Hero,
    pub cards: Vec<String>,
}
