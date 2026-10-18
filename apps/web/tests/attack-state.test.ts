import { describe, expect, test } from "vitest";
import type { Action, ActionPreview } from "../src/engine/types";
import { deriveAttackState, previewOn } from "../src/ui/attack-state";

const p = (action: Action, over: Partial<ActionPreview> = {}): ActionPreview => ({
  action,
  cost: 1,
  hands_over: false,
  budget_after: 2,
  ends_game: null,
  attacker_destroyed: false,
  ...over,
});
const hero = (attacker: number) => p({ attack: { attacker, target: "hero" } });
const onCreature = (attacker: number, creature: number) => p({ attack: { attacker, target: { creature } } });

const legal: ActionPreview[] = [
  p({ play: { card: 9 } }),
  p({ activate: { effect: 0 } }),
  hero(3),
  onCreature(3, 20),
  onCreature(3, 21),
  onCreature(4, 21),
  p("concede", { cost: null }),
];

describe("deriveAttackState: everything is read from the engine previews", () => {
  test("targets are exactly the previews of the selected attacker", () => {
    const s = deriveAttackState(3, legal);
    expect(s.selected).toBe(3);
    expect(s.attacks).toEqual([legal[2], legal[3], legal[4]]);
    expect(s.heroTargetable).toBe(true);
    expect([...s.creatureTargets].sort()).toEqual([20, 21]);
  });

  test("another attacker has its own targets, the hero is absent from the set when no preview names it", () => {
    const s = deriveAttackState(4, legal);
    expect(s.heroTargetable).toBe(false);
    expect([...s.creatureTargets]).toEqual([21]);
  });

  test("facing a taunt the engine only lists the taunting creatures: the hero is not targetable", () => {
    const s = deriveAttackState(3, [onCreature(3, 20), p("concede", { cost: null })]);
    expect(s.heroTargetable).toBe(false);
    expect([...s.creatureTargets]).toEqual([20]);
  });

  test("no selection: nothing is offered", () => {
    const s = deriveAttackState(null, legal);
    expect(s).toEqual({ selected: null, attacks: [], heroTargetable: false, creatureTargets: new Set() });
  });

  test("a stale selection (no attack for that id) is ignored by derivation", () => {
    expect(deriveAttackState(99, legal).selected).toBeNull();
    expect(deriveAttackState(99, legal).attacks).toEqual([]);
  });

  test("an empty legal list (computer turn, game over) ignores the selection", () => {
    expect(deriveAttackState(3, []).selected).toBeNull();
  });

  test("a play preview for the same number is not an attack", () => {
    expect(deriveAttackState(9, legal).selected).toBeNull();
  });
});

describe("previewOn", () => {
  test("returns the very preview object of that target (what the controller needs)", () => {
    const s = deriveAttackState(3, legal);
    expect(previewOn(s.attacks, "hero")).toBe(legal[2]);
    expect(previewOn(s.attacks, { creature: 21 })).toBe(legal[4]);
  });

  test("undefined for a target the engine did not offer", () => {
    const s = deriveAttackState(3, legal);
    expect(previewOn(s.attacks, { creature: 77 })).toBeUndefined();
    expect(previewOn([], "hero")).toBeUndefined();
  });
});
