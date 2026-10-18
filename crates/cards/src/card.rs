use crate::effects::CardEffect;
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
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        effects: Vec<CardEffect>,
    },
    /// A spell: played from the hand, its `on_play` effects resolve, then it is discarded. It has
    /// no `force`, `health` and the like (serde refuses them as unknown fields). `effects` is
    /// written even when empty; an empty list is refused by validation (`SpellWithoutEffects`).
    Spell {
        id: String,
        name: String,
        cost: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
        #[serde(default)]
        effects: Vec<CardEffect>,
    },
}

impl Card {
    pub fn id(&self) -> &str {
        match self {
            Card::Creature { id, .. } | Card::Spell { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Card::Creature { name, .. } | Card::Spell { name, .. } => name,
        }
    }

    /// The numeric stats of the card, as the engine consumes them. An immobile unit has no
    /// `force` nor `attack_cost` in its data: internally they read 0 (never written back). A
    /// spell has the shape an instance carries: its `cost` and zeros (a spell has no life).
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
            Card::Spell { cost, .. } => CreatureStats {
                cost: *cost,
                force: 0,
                attack_cost: 0,
                health: 0,
                provocation: false,
                riposte: 0,
            },
        }
    }

    /// The card as the engine setup consumes it.
    pub fn spec(&self) -> CardSpec {
        match self {
            Card::Creature {
                immobile,
                tags,
                effects,
                ..
            } => CardSpec::Unit(UnitSpec {
                stats: self.stats(),
                immobile: *immobile,
                tags: tags.clone(),
                effects: effects.clone(),
            }),
            Card::Spell {
                cost,
                tags,
                effects,
                ..
            } => CardSpec::Spell(SpellSpec {
                cost: *cost,
                tags: tags.clone(),
                effects: effects.clone(),
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

/// A unit as the engine setup consumes it: its stats, the keyword `immobile`, its `tags` and its
/// `effects` (the normal attack is never written there: it stays derived from the flat fields).
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
    pub effects: Vec<CardEffect>,
}

/// A spell as the engine setup consumes it: its `cost`, its `tags` and its `effects` (all
/// `on_play`, at least one: validated by [`validate_card_spec`](crate::validate_card_spec)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellSpec {
    pub cost: u32,
    pub tags: Vec<String>,
    pub effects: Vec<CardEffect>,
}

/// A card as the engine setup consumes it: validated by `duel-cards`
/// ([`validate_card_spec`](crate::validate_card_spec)), so that new kinds of card join as variants
/// without touching the contract between crates.
///
/// A unit's JSON is written without `kind`: the six keys of [`CreatureStats`] in their order
/// (`cost, force, attack_cost, health, provocation, riposte`), then `immobile` (only when true),
/// `tags` and `effects` (only when not empty); an immobile unit omits `force` and `attack_cost`.
/// A spell is written `{"kind":"spell","cost":..,"tags"?:[..],"effects":[..]}`. Read, `kind` is
/// optional for a unit (`unit` or `creature`), required for a spell; every field is read as an
/// option so that a spell can refuse the fields of a unit (`unknown field`, the same serde error
/// family, `invalid_json` at the binding) and a unit its missing ones. `provocation` and `riposte`
/// default, unknown keys are refused. `Serialize` and `Deserialize` are written by hand around a
/// flat struct (no `serde_json::Value` buffer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardSpec {
    Unit(UnitSpec),
    Spell(SpellSpec),
}

impl CardSpec {
    /// The stats of the unit, `None` for a spell.
    pub fn creature(&self) -> Option<&CreatureStats> {
        match self {
            CardSpec::Unit(unit) => Some(&unit.stats),
            CardSpec::Spell(_) => None,
        }
    }

    /// The tags of the card, whatever its kind.
    pub fn tags(&self) -> &[String] {
        match self {
            CardSpec::Unit(unit) => &unit.tags,
            CardSpec::Spell(spell) => &spell.tags,
        }
    }

    /// True for a unit with neither `immobile`, tags nor effects: the only thing the engine plays
    /// today.
    pub fn is_plain_unit(&self) -> bool {
        match self {
            CardSpec::Unit(unit) => {
                !unit.immobile && unit.tags.is_empty() && unit.effects.is_empty()
            }
            CardSpec::Spell(_) => false,
        }
    }
}

impl From<CreatureStats> for CardSpec {
    fn from(stats: CreatureStats) -> Self {
        CardSpec::Unit(UnitSpec {
            stats,
            immobile: false,
            tags: Vec::new(),
            effects: Vec::new(),
        })
    }
}

/// Write side of the wire form of a unit: borrows the spec, skips what the format omits.
#[derive(Serialize)]
struct UnitOut<'a> {
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
    #[serde(skip_serializing_if = "<[CardEffect]>::is_empty")]
    effects: &'a [CardEffect],
}

/// Write side of the wire form of a spell.
#[derive(Serialize)]
struct SpellOut<'a> {
    kind: &'static str,
    cost: u32,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    tags: &'a [String],
    effects: &'a [CardEffect],
}

/// The kinds a spec may name; `creature` reads as a unit.
#[derive(Deserialize)]
enum SpecKind {
    #[serde(rename = "unit", alias = "creature")]
    Unit,
    #[serde(rename = "spell")]
    Spell,
}

/// Read side of the wire form (flat, every field optional).
#[derive(Deserialize)]
#[serde(rename = "CardSpec", deny_unknown_fields)]
struct SpecIn {
    #[serde(default)]
    kind: Option<SpecKind>,
    #[serde(default)]
    cost: Option<u32>,
    #[serde(default)]
    force: Option<u32>,
    #[serde(default)]
    attack_cost: Option<u32>,
    #[serde(default)]
    health: Option<u32>,
    #[serde(default)]
    provocation: Option<bool>,
    #[serde(default)]
    riposte: Option<u32>,
    #[serde(default)]
    immobile: Option<bool>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    effects: Vec<CardEffect>,
}

/// The keys a spell may carry (for the `unknown field` error of a unit field).
const SPELL_FIELDS: &[&str] = &["kind", "cost", "tags", "effects"];

impl Serialize for CardSpec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            CardSpec::Unit(unit) => UnitOut {
                cost: unit.stats.cost,
                force: (!unit.immobile).then_some(unit.stats.force),
                attack_cost: (!unit.immobile).then_some(unit.stats.attack_cost),
                health: unit.stats.health,
                provocation: unit.stats.provocation,
                riposte: unit.stats.riposte,
                immobile: unit.immobile,
                tags: &unit.tags,
                effects: &unit.effects,
            }
            .serialize(serializer),
            CardSpec::Spell(spell) => SpellOut {
                kind: "spell",
                cost: spell.cost,
                tags: &spell.tags,
                effects: &spell.effects,
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for CardSpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let wire = SpecIn::deserialize(deserializer)?;
        if matches!(wire.kind, Some(SpecKind::Spell)) {
            let unit_fields = [
                ("force", wire.force.is_some()),
                ("attack_cost", wire.attack_cost.is_some()),
                ("health", wire.health.is_some()),
                ("provocation", wire.provocation.is_some()),
                ("riposte", wire.riposte.is_some()),
                ("immobile", wire.immobile.is_some()),
            ];
            if let Some((field, _)) = unit_fields.iter().find(|(_, present)| *present) {
                return Err(D::Error::unknown_field(field, SPELL_FIELDS));
            }
            return Ok(CardSpec::Spell(SpellSpec {
                cost: wire.cost.ok_or_else(|| D::Error::missing_field("cost"))?,
                tags: wire.tags,
                effects: wire.effects,
            }));
        }
        let cost = wire.cost.ok_or_else(|| D::Error::missing_field("cost"))?;
        let health = wire
            .health
            .ok_or_else(|| D::Error::missing_field("health"))?;
        let immobile = wire.immobile.unwrap_or(false);
        let (force, attack_cost) = if immobile {
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
                cost,
                force,
                attack_cost,
                health,
                provocation: wire.provocation.unwrap_or(false),
                riposte: wire.riposte.unwrap_or(0),
            },
            immobile,
            tags: wire.tags,
            effects: wire.effects,
        }))
    }
}
