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
        #[serde(default)]
        provocation: bool,
        #[serde(default)]
        riposte: u32,
    },
}

impl Card {
    pub fn id(&self) -> &str {
        match self {
            Card::Creature { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Card::Creature { name, .. } => name,
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
                provocation,
                riposte,
                ..
            } => CreatureStats {
                cost: *cost,
                force: *force,
                attack_cost: *attack_cost,
                health: *health,
                provocation: *provocation,
                riposte: *riposte,
            },
        }
    }
}

/// Resolved numeric stats of a creature card (what the engine copies into a game).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatureStats {
    pub cost: u32,
    pub force: u32,
    pub attack_cost: u32,
    pub health: u32,
    #[serde(default)]
    pub provocation: bool,
    #[serde(default)]
    pub riposte: u32,
}

/// A card as the engine setup consumes it: validated by `duel-cards`
/// ([`validate_card_spec`](crate::validate_card_spec)), so that new kinds of card (buildings,
/// spells) join as variants without touching the contract between crates.
///
/// A single variant for now. Its JSON is exactly the JSON of [`CreatureStats`]: `Serialize` and
/// `Deserialize` are written by hand and delegate to it, so `deny_unknown_fields`, the optional
/// `provocation` and `riposte` (four or six keys read, six always written) and the text of every
/// serde error stay those of `CreatureStats` (`#[serde(untagged)]` would change the messages, and
/// `transparent` does not apply to enums).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardSpec {
    Creature(CreatureStats),
}

impl CardSpec {
    /// The creature stats of the card. Temporary: it exists because there is one variant; the
    /// day another kind joins (#137), callers `match` on the kind instead.
    pub fn creature(&self) -> &CreatureStats {
        match self {
            CardSpec::Creature(stats) => stats,
        }
    }
}

impl From<CreatureStats> for CardSpec {
    fn from(stats: CreatureStats) -> Self {
        CardSpec::Creature(stats)
    }
}

impl Serialize for CardSpec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            CardSpec::Creature(stats) => stats.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for CardSpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        CreatureStats::deserialize(deserializer).map(CardSpec::Creature)
    }
}
