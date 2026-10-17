import { beforeAll, describe, expect, test } from "vitest";
import { COMPUTER_SLICE_MS, COMPUTER_STEP_DELAY_MS, MAX_COMPUTER_ACTIONS_PER_CHAIN } from "../src/game/computer";
import * as computer from "../src/game/computer";
import { GameController } from "../src/game/controller";
import { CARDS_JSON, DECKS_JSON } from "../src/data/starter";
import { PAIRS, initTestEngine } from "./support/engine";
import { randomPreview, realDeps, settle } from "./support/policies";

beforeAll(async () => {
  await initTestEngine();
});

describe("constants", () => {
  test("the named limits of the design", () => {
    expect(MAX_COMPUTER_ACTIONS_PER_CHAIN).toBe(1000);
    expect(COMPUTER_SLICE_MS).toBe(12);
    expect(COMPUTER_STEP_DELAY_MS).toBe(0);
  });

  test("the file holds no policy any more: exactly the three constants (the choice is the engine's)", () => {
    expect(Object.keys(computer).sort()).toEqual(["COMPUTER_SLICE_MS", "COMPUTER_STEP_DELAY_MS", "MAX_COMPUTER_ACTIONS_PER_CHAIN"]);
  });
});

// The driver, played on the real engine through the controller.
describe("the driver on the real engine", () => {
  test("20 seeds x 4 pairs: every game ends, the computer never concedes, computerSteps is coherent", async () => {
    let games = 0;
    for (const [one, two] of PAIRS) {
      for (let seed = 0; seed < 5; seed++) {
        games += 1;
        const c = GameController.create(
          { mode: "computer", deckOne: { id: one, name: one }, deckTwo: { id: two, name: two }, seed },
          realDeps(),
        );
        try {
          c.start();
          for (;;) {
            const s = await settle(c);
            const tail = (() => {
              let n = 0;
              for (let i = s.records.length - 1; i >= 0 && s.records[i].by === "computer"; i--) n += 1;
              return n;
            })();
            if (tail > 0) expect(s.computerSteps, `${one}/${two} seed ${seed}`).toBe(tail);
            if (s.phase !== "human-turn") break;
            expect(c.submit(randomPreview(seed)(s)).ok).toBe(true);
          }
          const end = c.getSnapshot();
          expect(end.phase, `${one}/${two} seed ${seed}`).toBe("over");
          for (const r of end.records.filter((x) => x.by === "computer")) {
            for (const e of r.events) {
              if ("game_won" in e) expect(e.game_won.reason).not.toBe("concede");
            }
          }
        } finally {
          c.dispose();
        }
      }
    }
    expect(games).toBe(20);
  });
});

describe("the chain bound follows from the data", () => {
  test("for each pair, hero health + sum of (health + 2) over the 30 cards of both decks <= the chain limit", () => {
    const cards = JSON.parse(CARDS_JSON) as { id: string; health?: number }[];
    const decks = JSON.parse(DECKS_JSON) as { id: string; hero: { health: number }; cards: string[] }[];
    const health = new Map(cards.map((c) => [c.id, c.health ?? 0]));
    const potential = (id: string) => {
      const d = decks.find((x) => x.id === id)!;
      expect(d.cards).toHaveLength(30);
      return d.hero.health + d.cards.reduce((sum, c) => sum + (health.get(c) ?? NaN) + 2, 0);
    };
    for (const [one, two] of PAIRS) {
      expect(potential(one) + potential(two), `${one}/${two}`).toBeLessThanOrEqual(MAX_COMPUTER_ACTIONS_PER_CHAIN);
    }
  });
});
