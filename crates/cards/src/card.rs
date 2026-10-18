use serde::{Deserialize, Serialize};

/// A card definition. The `kind` field discriminates the variant in JSON.
///
/// The variant is written `unit` and still read as `creature` (alias, #206): the frozen data
/// sets use the old name and are never rewritten. The Rust name stays `Creature` (#205).
/// `force` and `attack_cost` are absent from the data of an `immobile` unit (never a `0`
/// sentinel); [`parse_cards`](crate::parse_cards) checks which of the two forms a card has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Card {
    #[serde(rename = "unit", alias = "creature")]
    Creature {
        id: String,
        name: String,
        cost: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        force: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attack_cost: Option<u32>,
        health: u32,
        #[serde(default)]
        provocation: bool,
        #[serde(default)]
        riposte: u32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        immobile: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
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

    /// The numeric stats of the card, as the engine consumes them. An immobile unit has no
    /// `force` nor `attack_cost` in its data: internally they read 0 (never written back).
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
                force: force.unwrap_or(0),
                attack_cost: attack_cost.unwrap_or(0),
                health: *health,
                provocation: *provocation,
                riposte: *riposte,
            },
        }
    }

    /// The card as the engine setup consumes it.
    pub fn spec(&self) -> CardSpec {
        match self {
            Card::Creature { immobile, tags, .. } => CardSpec::Unit(UnitSpec {
                stats: self.stats(),
                immobile: *immobile,
                tags: tags.clone(),
            }),
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

/// A unit as the engine setup consumes it: its stats, the keyword `immobile` and its `tags`.
///
/// For an immobile unit `stats.force` and `stats.attack_cost` are 0: that is the internal state,
/// the wire form refuses their presence and 0 is never a sentinel of the data; the only reader on
/// the engine side stays `effective()`. `tags` are untrusted display text, never read by the
/// engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitSpec {
    pub stats: CreatureStats,
    pub immobile: bool,
    pub tags: Vec<String>,
}

/// A card as the engine setup consumes it: validated by `duel-cards`
/// ([`validate_card_spec`](crate::validate_card_spec)), so that new kinds of card (spells)
/// join as variants without touching the contract between crates.
///
/// A single variant for now. Its JSON is written without `kind`: the six keys of
/// [`CreatureStats`] in their order (`cost, force, attack_cost, health, provocation, riposte`),
/// then `immobile` (only when true) and `tags` (only when not empty); an immobile unit omits
/// `force` and `attack_cost`. Read, `kind` is optional (`unit` or `creature`), `provocation` and
/// `riposte` default, and unknown keys are refused. `Serialize` and `Deserialize` are written by
/// hand around a flat struct (no `serde_json::Value` buffer, so serde errors keep their line and
/// column).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardSpec {
    Unit(UnitSpec),
}

impl CardSpec {
    /// The stats of the unit. Temporary: it exists because there is one variant; the day
    /// another kind joins (task 2 of #137, which makes it an `Option`), callers `match`.
    pub fn creature(&self) -> &CreatureStats {
        match self {
            CardSpec::Unit(unit) => &unit.stats,
        }
    }

    /// True for a unit with neither `immobile` nor `tags`: the only thing the engine plays today.
    pub fn is_plain_unit(&self) -> bool {
        match self {
            CardSpec::Unit(unit) => !unit.immobile && unit.tags.is_empty(),
        }
    }
}

impl From<CreatureStats> for CardSpec {
    fn from(stats: CreatureStats) -> Self {
        CardSpec::Unit(UnitSpec {
            stats,
            immobile: false,
            tags: Vec::new(),
        })
    }
}

/// Write side of the wire form: borrows the spec, skips what the format omits.
#[derive(Serialize)]
struct SpecOut<'a> {
    cost: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    force: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attack_cost: Option<u32>,
    health: u32,
    provocation: bool,
    riposte: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    immobile: bool,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    tags: &'a [String],
}

/// The kinds a spec may name; both read as a unit.
#[derive(Deserialize)]
enum SpecKind {
    #[serde(rename = "unit", alias = "creature")]
    Unit,
}

/// Read side of the wire form (flat, so that serde's errors keep their position).
#[derive(Deserialize)]
#[serde(rename = "CardSpec", deny_unknown_fields)]
struct SpecIn {
    #[serde(default)]
    #[allow(dead_code)]
    kind: Option<SpecKind>,
    cost: u32,
    #[serde(default)]
    force: Option<u32>,
    #[serde(default)]
    attack_cost: Option<u32>,
    health: u32,
    #[serde(default)]
    provocation: bool,
    #[serde(default)]
    riposte: u32,
    #[serde(default)]
    immobile: bool,
    #[serde(default)]
    tags: Vec<String>,
}

impl Serialize for CardSpec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            CardSpec::Unit(unit) => SpecOut {
                cost: unit.stats.cost,
                force: (!unit.immobile).then_some(unit.stats.force),
                attack_cost: (!unit.immobile).then_some(unit.stats.attack_cost),
                health: unit.stats.health,
                provocation: unit.stats.provocation,
                riposte: unit.stats.riposte,
                immobile: unit.immobile,
                tags: &unit.tags,
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for CardSpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let wire = SpecIn::deserialize(deserializer)?;
        let (force, attack_cost) = if wire.immobile {
            for (field, value) in [("force", wire.force), ("attack_cost", wire.attack_cost)] {
                if value.is_some() {
                    return Err(D::Error::custom(format!(
                        "an immobile unit has no `{field}`"
                    )));
                }
            }
            (0, 0)
        } else {
            (
                wire.force.ok_or_else(|| D::Error::missing_field("force"))?,
                wire.attack_cost
                    .ok_or_else(|| D::Error::missing_field("attack_cost"))?,
            )
        };
        Ok(CardSpec::Unit(UnitSpec {
            stats: CreatureStats {
                cost: wire.cost,
                force,
                attack_cost,
                health: wire.health,
                provocation: wire.provocation,
                riposte: wire.riposte,
            },
            immobile: wire.immobile,
            tags: wire.tags,
        }))
    }
}
