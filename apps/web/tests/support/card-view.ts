import type { CardView, EffectLine } from "../../src/engine/types";

/** The normal attack as the engine writes it in the view of a plain unit (no `index`, one target to pick). */
export function attackLine(force: number, attackCost: number): EffectLine {
  return {
    trigger: "activated",
    cost: attackCost,
    ops: [{ op: "attack", target: { select: "hero_or_unit", side: "opposing" }, amount: force, polarity: "bonus" }],
    picks: [{ role: "target", kind: "hero_or_unit", side: "opposing" }],
  };
}

/**
 * A `CardView` literal for a test that does not read it from the engine: a plain unit, with the attack line the
 * engine would write for the values given (unless `effects` is overridden), so the two new keys of #209 are
 * written in one place only.
 */
export function cardView(over: Partial<CardView> = {}): CardView {
  const base = {
    id: 7,
    name: "Garde du rempart",
    cost: 3,
    force: 1,
    attack_cost: 2,
    health: 6,
    provocation: false,
    riposte: 0,
    kind: "unit" as const,
    ...over,
  };
  return { effects: [attackLine(base.force, base.attack_cost)], ...base };
}
