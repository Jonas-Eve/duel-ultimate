// What the detail sheet shows of a card, arranged from the card's own view: pure, nothing is computed
// (the lines of effect, their operations, polarity and costs come from the engine; the numbers are the
// view's fields). The words of a line are written here, by the closed table of `effects.fr.ts`.
import type { CardView, EffectLine, OpLine, Polarity, Selector, Side } from "../engine/types";
import { OP_FR, SELECTOR_FR, TRIGGER_FR, type OpContext } from "./effects.fr";
import { fr } from "./fr";
import { KEYWORDS_FR, type KeywordKey } from "./keywords.fr";

/** A passive effect: permanent, no cost; `explanation` is the French text of the single source. */
export interface KeywordLine {
  key: KeywordKey;
  label: string;
  explanation: string;
}
export interface CardEffects {
  /** The generic lines of effect, as the view gives them: the normal attack and every declared effect, one format. */
  active: EffectLine[];
  passive: KeywordLine[];
  /**
   * Shown whatever they are: attack and riposte at 0 included. `null` is a value the card does not have: the attack
   * of an immobile unit, and the attack, riposte and life of a spell (read from `kind` and `immobile`, never worked out).
   */
  values: { cost: number; attack: number | null; riposte: number | null; health: number | null };
}

export function cardEffects(card: CardView): CardEffects {
  const passive: KeywordLine[] = [];
  if (card.provocation) passive.push({ key: "provocation", ...KEYWORDS_FR.provocation });
  if (card.immobile === true) passive.push({ key: "immobile", ...KEYWORDS_FR.immobile });
  const spell = card.kind === "spell";
  return {
    active: card.effects,
    passive,
    values: {
      cost: card.cost,
      attack: spell || card.immobile === true ? null : card.force,
      riposte: spell ? null : card.riposte,
      health: spell ? null : card.health,
    },
  };
}

/** One operation of a line: its words and the polarity the engine derived (a malus gets a text mark). */
export interface OpPart {
  text: string;
  polarity: Polarity;
}
/** A line cut where the sheet draws it: the lead, the cost between parentheses (activated only), the operations, the spoken addition. */
export interface LineParts {
  lead: string;
  cost?: number;
  ops: OpPart[];
  hidden: string;
}

const sideOf = (selector: Selector): Side => (selector.select === "self" ? "own" : selector.side);

function opText(op: OpLine, lead: boolean): string {
  const side = sideOf(op.target);
  const ctx: OpContext = { target: SELECTOR_FR[op.target.select][side], side, lead };
  // `OP_FR[op.op]` takes the operation of its own name: the lookup is correlated, which TypeScript cannot follow.
  return (OP_FR[op.op] as (op: OpLine, ctx: OpContext) => string)(op, ctx);
}

/** An activated line made of attacks only reads "Attaque (N) : ..."; any other reads by its trigger. */
export function effectLineParts(line: EffectLine): LineParts {
  const attacksOnly = line.ops.length > 0 && line.ops.every((o) => o.op === "attack");
  const lead = line.trigger === "activated" && attacksOnly ? fr.detailAttackEffect : TRIGGER_FR[line.trigger];
  const lineLead = line.trigger === "activated" && attacksOnly;
  const cost = line.trigger === "activated" ? line.cost : undefined;
  return {
    lead,
    cost,
    ops: line.ops.map((op) => ({ text: opText(op, lineLead), polarity: op.polarity })),
    hidden: cost === undefined ? "" : `, pour ${cost} ${fr.detailTimeWord}`,
  };
}

/** The mark that follows the words of a malus operation, as text (never colour alone). */
export const malusMark = (text: string): string => `${text} \u2014 ${fr.malus}`;

/**
 * The one place an effect line's words are made: `visible` is what is drawn (the hourglass sits between
 * the parentheses, aria-hidden), `hidden` the spoken addition (rendered `sr-only`). The accessible name of
 * the line is `visible + hidden`, so the visible text is contained in it (label in name).
 */
export function effectLineText(line: EffectLine): { visible: string; hidden: string } {
  const { lead, cost, ops, hidden } = effectLineParts(line);
  const words = ops.map((o) => (o.polarity === "malus" ? malusMark(o.text) : o.text)).join(" ; ");
  return { visible: `${lead}${cost === undefined ? "" : ` (${cost})`} : ${words}`, hidden };
}

export function effectLineLabel(line: EffectLine): string {
  const { visible, hidden } = effectLineText(line);
  return visible + hidden;
}
