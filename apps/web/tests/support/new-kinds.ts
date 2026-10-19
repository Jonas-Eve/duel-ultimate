import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { CardView } from "../../src/engine/types";
import { repoRoot } from "./paths";
import { initTestEngine, newSessionFromSetup } from "./engine";

const GOLDEN = resolve(repoRoot, "crates/engine/tests/golden/setup-starter-1.1.0-rush-bulwark.json");

/** The names the four new-kind cards carry in the hand of One. */
export const NEW_KIND_NAMES = { spell: "zz-spell", immobile: "zz-immobile", special: "zz-special", aura: "zz-aura" } as const;

/**
 * The setup of the golden starter game (`rush` for One, `bulwark` for Two) with four cards of One replaced by
 * what no starter card is yet, the whole deck of One being in its opening hand:
 * - a spell with an `on_play` effect (damage on an opposing unit, then a card drawn),
 * - an immobile unit tagged `tag` (default "bâtiment"),
 * - a unit with a special attack (activated, 2 time, 5 damage, ignores the provocation),
 * - a unit with continuous effects (a bonus and a malus).
 * Real engine data, never a mock: the JSON goes through `new Game(...)`.
 */
export function newKindsSetup(tag = "bâtiment"): Record<string, any> {
  const setup = JSON.parse(readFileSync(GOLDEN, "utf8"));
  setup.one.hero.starting_hand = 30;
  const cards = setup.one.cards;
  cards[0] = {
    kind: "spell",
    cost: 1,
    effects: [
      {
        trigger: "on_play",
        ops: [
          { op: "deal_damage", target: { select: "unit", side: "opposing" }, amount: 2 },
          { op: "draw", target: { select: "hero", side: "own" }, count: 1 },
        ],
      },
    ],
  };
  cards[1] = { cost: 1, health: 4, immobile: true, tags: [tag] };
  cards[2] = {
    ...cards[2],
    effects: [
      {
        trigger: "activated",
        cost: 2,
        ops: [{ op: "attack", target: { select: "hero_or_unit", side: "opposing" }, amount: 5, ignores_provocation: true }],
      },
    ],
  };
  cards[3] = {
    ...cards[3],
    effects: [
      {
        trigger: "continuous",
        ops: [
          { op: "modify_stat", target: { select: "all_units", side: "own" }, stat: "force", delta: 1 },
          { op: "modify_stat", target: { select: "all_units", side: "own" }, stat: "attack_cost", delta: 1 },
        ],
      },
    ],
  };
  const names = setup.one.card_names;
  names[0] = NEW_KIND_NAMES.spell;
  names[1] = NEW_KIND_NAMES.immobile;
  names[2] = NEW_KIND_NAMES.special;
  names[3] = NEW_KIND_NAMES.aura;
  return setup;
}

/** The four cards as the engine's view of the hand of One writes them. */
export async function newKindsViews(tag?: string): Promise<{ spell: CardView; immobile: CardView; special: CardView; aura: CardView }> {
  await initTestEngine();
  const session = newSessionFromSetup(newKindsSetup(tag));
  try {
    const hand = session.view("One").you.hand;
    const named = (name: string): CardView => {
      const card = hand.find((c) => c.name === name);
      if (card === undefined) throw new Error(`${name} is not in the opening hand`);
      return card;
    };
    return {
      spell: named(NEW_KIND_NAMES.spell),
      immobile: named(NEW_KIND_NAMES.immobile),
      special: named(NEW_KIND_NAMES.special),
      aura: named(NEW_KIND_NAMES.aura),
    };
  } finally {
    session.dispose();
  }
}
