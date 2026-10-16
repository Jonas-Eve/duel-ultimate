import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import { repoRoot, webRoot } from "./support/paths";

const wasmDir = resolve(webRoot, "src/wasm");
const wasmFile = resolve(wasmDir, "duel_engine_wasm_bg.wasm");

type Binding = typeof import("../src/wasm/duel_engine_wasm");
let binding: Binding;

beforeAll(async () => {
  if (!existsSync(wasmFile)) throw new Error(`${wasmFile} is missing: run \`npm test\` / \`npm run build\` (the pre* hook generates it)`);
  binding = await import("../src/wasm/duel_engine_wasm");
  binding.initSync({ module: readFileSync(wasmFile) });
});

const fixture = (name: string) => readFileSync(resolve(repoRoot, "data/fixture/1.0.0", name), "utf8");
const VERSIONS = JSON.stringify(["fixture@1.0.0"]);
const cardNames = new Set(
  (JSON.parse(fixture("cards.json")) as { name: string }[]).map((c) => c.name),
);

interface Preview {
  action: unknown;
  cost: number | null;
  hands_over: boolean;
  budget_after: number | null;
  ends_game: { winner: string; reason: string } | null;
}

function newGame(seed = 1) {
  return binding.Game.from_data(fixture("cards.json"), fixture("decks.json"), "aggro", "control", VERSIONS, seed);
}

const previewOf = (game: InstanceType<Binding["Game"]>) => JSON.parse(game.legal_actions_with_time()) as Preview[];

describe("Game.legal_actions_with_time() in the browser build", () => {
  test("same actions in the same order as legal_actions(), with the five agreed fields", () => {
    const game = newGame(3);
    try {
      for (let step = 0; step < 100; step++) {
        const actions = JSON.parse(game.legal_actions()) as unknown[];
        const previews = previewOf(game);
        expect(previews.map((p) => p.action)).toEqual(actions);
        for (const p of previews) {
          expect(Object.keys(p).sort()).toEqual(["action", "budget_after", "cost", "ends_game", "hands_over"]);
        }
        expect(game.legal_actions_with_time()).not.toContain("drew");
        const playable = actions.filter((a) => a !== "concede");
        if (playable.length === 0) break;
        game.apply(JSON.stringify(playable[(step * 7) % playable.length]));
      }
    } finally {
      game.free();
    }
  });

  test("the time facts come from the engine: effect cost, concede ends the game, finished game is empty", () => {
    const game = newGame(3);
    try {
      const previews = previewOf(game);
      const effect = previews.find((p) => JSON.stringify(p.action) === '{"activate":{"effect":0}}') as Preview;
      expect(typeof effect.cost).toBe("number");
      expect(typeof effect.budget_after).toBe("number");
      const concede = previews[previews.length - 1];
      expect(concede).toEqual({
        action: "concede",
        cost: null,
        hands_over: false,
        budget_after: null,
        ends_game: { winner: "Two", reason: "concede" },
      });
      game.apply('"concede"');
      expect(previewOf(game)).toEqual([]);
    } finally {
      game.free();
    }
  });
});
