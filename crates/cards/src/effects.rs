//! Effects of units and spells (#207, task 2 of #137): the closed sets of the catalog
//! `docs/design/effets.md` (#125), wave 1 only, as data.
//!
//! An effect is a trigger, a cost (activated effects only) and a bounded list of operations;
//! the selector lives in each operation and carries the side. Everything outside these closed
//! sets is refused by serde (`CardsError::Json`). The tables of an operation (which triggers,
//! which selectors, which Phi class) are the single source read by the validation and by the
//! cross-check test against the catalog. Nothing here is behaviour: the engine refuses these
//! cards until task 3.
use serde::{Deserialize, Serialize};

/// Maximum number of effects of a card.
pub const MAX_CARD_EFFECTS: usize = 4;
/// Maximum number of operations of an effect.
pub const MAX_OPS_PER_EFFECT: usize = 3;
/// Maximum number of picks an effect expects (attackers and targets). Duplicated from the engine
/// (`duel_engine::MAX_PICKS`, 8), which this crate cannot depend on: a test of the engine keeps
/// `MAX_EFFECT_PICKS <= MAX_PICKS`. With a board limit of 5 each pick has about 12 targets, so 3
/// picks is about 1 700 actions per use; 8 would blow up the preview (one clone per legal action).
pub const MAX_EFFECT_PICKS: usize = 3;
/// Slots listed for one attack at most (above the valid maximum, so the bound check still sees
/// an excess).
const PICK_CAP: u32 = 4;

/// Names written in the data, one list per closed set (literals, so that a source scan can read
/// them). Each is pinned by a test against the serialization of every variant.
pub const OP_NAMES: [&str; 5] = [
    "attack",
    "deal_damage",
    "draw",
    "modify_stat",
    "grant_keyword",
];
pub const SELECT_NAMES: [&str; 5] = ["self", "hero", "unit", "hero_or_unit", "all_units"];
pub const SIDE_NAMES: [&str; 3] = ["own", "opposing", "both"];
pub const TRIGGER_NAMES: [&str; 3] = ["on_play", "activated", "continuous"];
pub const DURATION_NAMES: [&str; 2] = ["instant", "while_source_in_play"];
pub const KEYWORD_NAMES: [&str; 1] = ["ranged"];
pub const STAT_NAMES: [&str; 4] = ["force", "riposte", "cost", "attack_cost"];
pub const MODE_NAMES: [&str; 2] = ["simultaneous", "chained"];

/// The side of the board a selector designates, from the point of view of the card's owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Own,
    Opposing,
    Both,
}

/// What an operation designates. `self` carries no side (the source is always the owner's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "select", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selector {
    /// The source of the effect. Written `{"select":"self"}`; a struct variant (not a unit one)
    /// so that `deny_unknown_fields` refuses `{"select":"self","side":"own"}`.
    #[serde(rename = "self")]
    Source {},
    Hero {
        side: Side,
    },
    Unit {
        side: Side,
    },
    HeroOrUnit {
        side: Side,
    },
    AllUnits {
        side: Side,
    },
}

/// A selector without its side: the key of the tables of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectorKind {
    Source,
    Hero,
    Unit,
    HeroOrUnit,
    AllUnits,
}

impl SelectorKind {
    pub const ALL: [SelectorKind; 5] = [
        SelectorKind::Source,
        SelectorKind::Hero,
        SelectorKind::Unit,
        SelectorKind::HeroOrUnit,
        SelectorKind::AllUnits,
    ];

    /// The name written in the data.
    pub fn name(self) -> &'static str {
        match self {
            SelectorKind::Source => "self",
            SelectorKind::Hero => "hero",
            SelectorKind::Unit => "unit",
            SelectorKind::HeroOrUnit => "hero_or_unit",
            SelectorKind::AllUnits => "all_units",
        }
    }
}

impl Selector {
    pub fn kind(&self) -> SelectorKind {
        match self {
            Selector::Source {} => SelectorKind::Source,
            Selector::Hero { .. } => SelectorKind::Hero,
            Selector::Unit { .. } => SelectorKind::Unit,
            Selector::HeroOrUnit { .. } => SelectorKind::HeroOrUnit,
            Selector::AllUnits { .. } => SelectorKind::AllUnits,
        }
    }

    /// The side designated; `self` is the owner's.
    pub fn side(&self) -> Side {
        match self {
            Selector::Source {} => Side::Own,
            Selector::Hero { side }
            | Selector::Unit { side }
            | Selector::HeroOrUnit { side }
            | Selector::AllUnits { side } => *side,
        }
    }
}

/// When an effect applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    OnPlay,
    Activated,
    Continuous,
}

impl Trigger {
    /// The name written in the data.
    pub fn name(self) -> &'static str {
        match self {
            Trigger::OnPlay => "on_play",
            Trigger::Activated => "activated",
            Trigger::Continuous => "continuous",
        }
    }

    /// The duration of an effect, derived from its trigger and never stored: in wave 1 it has no
    /// degree of freedom, storing it would make an invalid state representable.
    pub fn duration(self) -> Duration {
        match self {
            Trigger::OnPlay | Trigger::Activated => Duration::Instant,
            Trigger::Continuous => Duration::WhileSourceInPlay,
        }
    }
}

/// How long an effect lasts (derived, see [`Trigger::duration`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Duration {
    Instant,
    WhileSourceInPlay,
}

/// Keywords an operation can grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keyword {
    Ranged,
}

/// Statistics `modify_stat` can change, written like the flat fields of a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Force,
    Riposte,
    Cost,
    AttackCost,
}

/// How a multi-attacker attack resolves (`attackers == 2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackMode {
    Simultaneous,
    Chained,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn one() -> u32 {
    1
}

fn is_one(value: &u32) -> bool {
    *value == 1
}

/// An operation of the catalog (wave 1, five variants). The class `N` operations of the catalog
/// are not variants: they are refused because they do not exist here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    /// A special attack. Its cost is the one of the effect (written once, never here). The
    /// normal attack is never written: it stays derived from the flat fields.
    Attack {
        target: Selector,
        amount: u32,
        #[serde(default, skip_serializing_if = "is_false")]
        ignores_provocation: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        scales_with_force: bool,
        #[serde(default = "one", skip_serializing_if = "is_one")]
        attackers: u32,
        /// Required if and only if `attackers == 2`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<AttackMode>,
    },
    DealDamage {
        target: Selector,
        amount: u32,
    },
    Draw {
        target: Selector,
        count: u32,
    },
    ModifyStat {
        target: Selector,
        stat: Stat,
        delta: i32,
    },
    GrantKeyword {
        target: Selector,
        keyword: Keyword,
    },
}

/// The Phi class of an operation (catalog, section 5): `S` static (values read, state untouched),
/// `D` decreasing (lowers Phi by at least 1 when it resolves), `N` neutral or increasing. No
/// operation of wave 1 is `N`: the class exists because the catalog names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhiClass {
    S,
    D,
    N,
}

/// An operation without its parameters: the key of the tables read by the validation and by the
/// cross-check against the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Attack,
    DealDamage,
    Draw,
    ModifyStat,
    GrantKeyword,
}

impl OpKind {
    pub const ALL: [OpKind; 5] = [
        OpKind::Attack,
        OpKind::DealDamage,
        OpKind::Draw,
        OpKind::ModifyStat,
        OpKind::GrantKeyword,
    ];

    /// The name written in the data.
    pub fn name(self) -> &'static str {
        match self {
            OpKind::Attack => "attack",
            OpKind::DealDamage => "deal_damage",
            OpKind::Draw => "draw",
            OpKind::ModifyStat => "modify_stat",
            OpKind::GrantKeyword => "grant_keyword",
        }
    }

    /// The triggers the operation accepts.
    pub fn triggers(self) -> &'static [Trigger] {
        match self {
            OpKind::Attack | OpKind::DealDamage | OpKind::Draw => {
                &[Trigger::OnPlay, Trigger::Activated]
            }
            OpKind::ModifyStat | OpKind::GrantKeyword => &[Trigger::Continuous],
        }
    }

    /// The selectors the operation accepts.
    pub fn selectors(self) -> &'static [SelectorKind] {
        match self {
            OpKind::Attack => &[SelectorKind::HeroOrUnit],
            OpKind::DealDamage => &[
                SelectorKind::HeroOrUnit,
                SelectorKind::Unit,
                SelectorKind::AllUnits,
            ],
            OpKind::Draw => &[SelectorKind::Hero],
            OpKind::ModifyStat | OpKind::GrantKeyword => &[
                SelectorKind::Source,
                SelectorKind::Unit,
                SelectorKind::AllUnits,
            ],
        }
    }

    /// The Phi class, by an exhaustive `match`: an operation added without a class does not
    /// compile. Consequence: `continuous` accepts S operations only, `on_play` and `activated`
    /// D operations only, so an effect never mixes classes and an activated effect always has a D
    /// operation (termination rule, catalog section 5).
    pub fn phi_class(self) -> PhiClass {
        match self {
            OpKind::Attack | OpKind::DealDamage | OpKind::Draw => PhiClass::D,
            OpKind::ModifyStat | OpKind::GrantKeyword => PhiClass::S,
        }
    }
}

/// Who gains or loses from an operation, from the point of view of the owner of the card. Derived,
/// never stored: the single source for the view and for the price. Written `bonus`, `malus` or
/// `neutral` by the view of the engine (#209).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Polarity {
    Bonus,
    Malus,
    Neutral,
}

/// Who is asked to pick what, and of which kind of thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PickRole {
    Attacker,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PickKind {
    Unit,
    HeroOrUnit,
}

/// One pick an effect expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PickSlot {
    pub role: PickRole,
    pub kind: PickKind,
    pub side: Side,
}

impl Op {
    pub fn kind(&self) -> OpKind {
        match self {
            Op::Attack { .. } => OpKind::Attack,
            Op::DealDamage { .. } => OpKind::DealDamage,
            Op::Draw { .. } => OpKind::Draw,
            Op::ModifyStat { .. } => OpKind::ModifyStat,
            Op::GrantKeyword { .. } => OpKind::GrantKeyword,
        }
    }

    /// The selector of the operation.
    pub fn target(&self) -> &Selector {
        match self {
            Op::Attack { target, .. }
            | Op::DealDamage { target, .. }
            | Op::Draw { target, .. }
            | Op::ModifyStat { target, .. }
            | Op::GrantKeyword { target, .. } => target,
        }
    }

    /// The Phi class (see [`OpKind::phi_class`]).
    pub fn phi_class(&self) -> PhiClass {
        self.kind().phi_class()
    }

    /// The polarity for the owner of the card. The affected side is the `side` of the selector
    /// (`self`: own). The value for the affected: negative for `attack` and `deal_damage`,
    /// positive for `draw` and `grant_keyword ranged`, the sign of `delta` for `modify_stat`
    /// (inverted for `cost` and `attack_cost`: a cost that goes up is bad for its owner). A
    /// positive value on the own side is a bonus, on the opposing side a malus; a negative value
    /// the other way round; `both` is neutral (and so is a zero delta).
    pub fn polarity(&self) -> Polarity {
        let value: i32 = match self {
            Op::Attack { .. } | Op::DealDamage { .. } => -1,
            Op::Draw { .. } => 1,
            Op::GrantKeyword { keyword, .. } => match keyword {
                Keyword::Ranged => 1,
            },
            Op::ModifyStat { stat, delta, .. } => match stat {
                Stat::Force | Stat::Riposte => delta.signum(),
                Stat::Cost | Stat::AttackCost => -delta.signum(),
            },
        };
        match (self.target().side(), value) {
            (Side::Both, _) | (_, 0) => Polarity::Neutral,
            (Side::Own, v) if v > 0 => Polarity::Bonus,
            (Side::Own, _) => Polarity::Malus,
            (Side::Opposing, v) if v > 0 => Polarity::Malus,
            (Side::Opposing, _) => Polarity::Bonus,
        }
    }
}

/// An effect: a trigger, a cost (activated effects only) and a bounded list of operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardEffect {
    pub trigger: Trigger,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<u32>,
    pub ops: Vec<Op>,
}

impl CardEffect {
    /// The picks the effect expects, in order: for each operation, its attackers first (an
    /// `attack` asks for `attackers` slots `attacker/unit/own` when `attackers >= 2` or when the
    /// source is a spell), then its target (one slot for a `unit` or `hero_or_unit` selector,
    /// none for `self`, `hero` and `all_units`). The single source of the pick list of a special
    /// attack. Open point of #138: when the source unit is one of its own attackers the number of
    /// `attacker` slots will adjust; until then a unit's `attackers = 2` attack asks for 2.
    pub fn expected_picks(&self, is_spell: bool) -> Vec<PickSlot> {
        let mut picks = Vec::new();
        for op in &self.ops {
            if let Op::Attack { attackers, .. } = op
                && (*attackers >= 2 || is_spell)
            {
                // Capped: an unvalidated count must not allocate without bound.
                for _ in 0..(*attackers).min(PICK_CAP) {
                    picks.push(PickSlot {
                        role: PickRole::Attacker,
                        kind: PickKind::Unit,
                        side: Side::Own,
                    });
                }
            }
            let kind = match op.target().kind() {
                SelectorKind::Unit => Some(PickKind::Unit),
                SelectorKind::HeroOrUnit => Some(PickKind::HeroOrUnit),
                SelectorKind::Source | SelectorKind::Hero | SelectorKind::AllUnits => None,
            };
            if let Some(kind) = kind {
                picks.push(PickSlot {
                    role: PickRole::Target,
                    kind,
                    side: op.target().side(),
                });
            }
        }
        picks
    }
}
