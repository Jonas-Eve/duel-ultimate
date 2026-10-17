// What the detail sheet shows of a card, arranged from the card's own fields: pure, nothing is
// computed (the attack's cost is `attack_cost`, its damage is `force`, as the engine's view says).
import type { CardView } from "../engine/types";
import { fr } from "./fr";
import { KEYWORDS_FR, type KeywordKey } from "./keywords.fr";

/** An active effect: it has a name, a time cost and a damage. The attack is the only one the engine knows. */
export interface EffectLine {
  name: string;
  cost: number;
  damage: number;
}
/** A passive effect: permanent, no cost; `explanation` is the French text of the single source. */
export interface KeywordLine {
  key: KeywordKey;
  label: string;
  explanation: string;
}
export interface CardEffects {
  active: EffectLine[];
  passive: KeywordLine[];
  /** Shown whatever they are: attack and riposte at 0 included. */
  values: { cost: number; attack: number; riposte: number; health: number };
}

export function cardEffects(card: CardView): CardEffects {
  const passive: KeywordLine[] = [];
  if (card.provocation) passive.push({ key: "provocation", ...KEYWORDS_FR.provocation });
  return {
    active: [{ name: fr.detailAttackEffect, cost: card.attack_cost, damage: card.force }],
    passive,
    values: { cost: card.cost, attack: card.force, riposte: card.riposte, health: card.health },
  };
}

/**
 * The one place an effect line's words are made: `visible` is what is drawn (the hourglass sits between
 * the parentheses, aria-hidden), `hidden` the spoken addition (rendered `sr-only`). The accessible name of
 * the line is `visible + hidden`, so the visible text is contained in it (label in name).
 */
export function effectLineText(line: EffectLine): { visible: string; hidden: string } {
  return {
    visible: `${line.name} (${line.cost}) : inflige ${line.damage} dégât${line.damage > 1 ? "s" : ""}`,
    hidden: `, pour ${line.cost} ${fr.detailTimeWord}`,
  };
}

export function effectLineLabel(line: EffectLine): string {
  const { visible, hidden } = effectLineText(line);
  return visible + hidden;
}
