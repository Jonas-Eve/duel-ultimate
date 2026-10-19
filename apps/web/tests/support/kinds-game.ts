import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { repoRoot } from "./paths";
import { initTestEngine } from "./engine";

const GOLDEN = resolve(repoRoot, "crates/engine/tests/golden/setup-starter-1.1.0-rush-bulwark.json");

/**
 * The setup of the golden starter game with, on One's side, a spell with a two-attacker attack, a
 * tagged immobile unit and a unit with a special attack; the whole deck of One is in its opening
 * hand. The cards of the real engine, never a mock: the setup JSON goes through `new Game(...)`.
 */
export function kindsSetup(): Record<string, any> {
  const setup = JSON.parse(readFileSync(GOLDEN, "utf8"));
  setup.one.hero.starting_hand = 30;
  const cards = setup.one.cards;
  cards[0] = {
    kind: "spell",
    cost: 1,
    tags: ["rituel"],
    effects: [
      {
        trigger: "on_play",
        ops: [{ op: "attack", target: { select: "hero_or_unit", side: "opposing" }, amount: 3, attackers: 2, mode: "simultaneous" }],
      },
    ],
  };
  cards[1] = { cost: 1, health: 4, immobile: true, tags: ["bâtiment"] };
  cards[2] = {
    ...cards[2],
    effects: [{ trigger: "activated", cost: 2, ops: [{ op: "attack", target: { select: "hero_or_unit", side: "opposing" }, amount: 5 }] }],
  };
  setup.one.card_names[0] = "zz-spell";
  setup.one.card_names[1] = "zz-immobile";
  setup.one.card_names[2] = "zz-special";
  return setup;
}

export async function kindsGame(seed = 3) {
  const binding = await initTestEngine();
  return new binding.Game(JSON.stringify(kindsSetup()), seed);
}
