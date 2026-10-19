// The closed French table of the effect lines (#210): one entry per element of each closed set of the
// catalogue of `crates/cards/src/effects.rs`, guarded by `tests/effects-fr.test.ts` (an element added on
// the Rust side only, or an entry without element, breaks it). Nothing here decides anything: the engine's
// view says which operation, which target, which polarity; this module only says it in French.
// A sentence of an operation never has a parenthesis: they belong to the cost, drawn with the hourglass.
import type { AttackMode, KeywordName, OpLine, Selector, Side, StatName, Trigger } from "../engine/types";
import { fr } from "./fr";
import { KEYWORDS_FR } from "./keywords.fr";

type SelectName = Selector["select"];
type OpName = OpLine["op"];

/** What an operation designates, after "sur": one phrase per selector and per side (`self` carries no side). */
export const SELECTOR_FR: Record<SelectName, Record<Side, string>> = {
  self: { own: "cette unité", opposing: "cette unité", both: "cette unité" },
  hero: { own: "votre héros", opposing: "le héros adverse", both: "les deux héros" },
  unit: { own: "une de vos unités", opposing: "une unité adverse", both: "une unité" },
  hero_or_unit: {
    own: "votre héros ou une de vos unités",
    opposing: "le héros adverse ou une unité adverse",
    both: "un héros ou une unité",
  },
  all_units: { own: "toutes vos unités", opposing: "toutes les unités adverses", both: "toutes les unités" },
};

/** A statistic as the data names it, in words. */
export const STAT_FR: Record<StatName, string> = {
  force: "force",
  riposte: "riposte",
  cost: "coût d'invocation",
  attack_cost: "coût d'attaque",
};

/** How an attack with several attackers chains them. */
export const MODE_FR: Record<AttackMode, string> = {
  simultaneous: "ensemble",
  chained: "à la suite",
};

/** The prefix of a line by its trigger (the cost of an activated effect follows it, in parentheses). */
export const TRIGGER_FR: Record<Trigger, string> = {
  activated: fr.effectGeneric,
  on_play: fr.effectOnPlay,
  continuous: fr.effectContinuous,
};

const plural = (n: number, one: string): string => `${n} ${one}${n > 1 ? "s" : ""}`;
const damage = (n: number): string => plural(n, "dégât");
const cards = (n: number): string => plural(n, "carte");
const signed = (n: number): string => (n > 0 ? `+${n}` : `${n}`);

/** What an operation sentence is made from: the phrase of its target, its side, and whether the line already says "Attaque". */
export interface OpContext {
  /** `SELECTOR_FR` of the operation's selector. */
  target: string;
  /** The side of the selector (`own` for `self`). */
  side: Side;
  /** The prefix of the line already reads "Attaque (N) :": an attack operation then only says what it does. */
  lead: boolean;
}

/** The sentence of each operation. An attack lists its parameters after the damage, only the ones present. */
export const OP_FR: { [K in OpName]: (op: Extract<OpLine, { op: K }>, ctx: OpContext) => string } = {
  attack: (op, ctx) => {
    const parts = [`${ctx.lead ? "inflige" : "Attaque : inflige"} ${damage(op.amount)}`];
    if (op.ignores_provocation) parts.push("ignore la provocation");
    if (op.mode !== undefined) parts.push(`${op.attackers ?? 2} attaquants ${MODE_FR[op.mode]}`);
    if (op.scales_with_force) parts.push("s'ajoute à la force");
    return parts.join(" ; ");
  },
  deal_damage: (op, ctx) => `Inflige ${damage(op.amount)} sur ${ctx.target}`,
  draw: (op, ctx) =>
    ctx.side === "own"
      ? `Pioche ${cards(op.count)}`
      : ctx.side === "opposing"
        ? `L'adversaire pioche ${cards(op.count)}`
        : `Chaque joueur pioche ${cards(op.count)}`,
  modify_stat: (op, ctx) => `${signed(op.delta)} ${STAT_FR[op.stat]} sur ${ctx.target}`,
  grant_keyword: (op, ctx) => `${KEYWORDS_FR[op.keyword satisfies KeywordName].label} accordé sur ${ctx.target}`,
};
