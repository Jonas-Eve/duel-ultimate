import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import { fixtureSetup } from "./support/fixture-setup";
import { webRoot } from "./support/paths";

const wasmDir = resolve(webRoot, "src/wasm");
const wasmFile = resolve(wasmDir, "duel_engine_wasm_bg.wasm");

type Binding = typeof import("../src/wasm/duel_engine_wasm");
let binding: Binding;

beforeAll(async () => {
  for (const f of [wasmFile, resolve(wasmDir, "duel_engine_wasm.d.ts")]) {
    if (!existsSync(f)) throw new Error(`${f} is missing: run \`npm test\` / \`npm run build\` (the pre* hook generates it)`);
  }
  binding = await import("../src/wasm/duel_engine_wasm");
  // Real module, loaded synchronously from the bytes on disk (no Vite plugin, no fetch).
  binding.initSync({ module: readFileSync(wasmFile) });
});

function errorOf(fn: () => unknown): Error {
  try {
    fn();
  } catch (e) {
    return e as Error;
  }
  throw new Error("expected a throw");
}

describe("engine-wasm in the browser build", () => {
  test("legal_actions of a fresh game is non-empty and ends with concede", () => {
    const game = new binding.Game(fixtureSetup(), 1);
    try {
      const actions = JSON.parse(game.legal_actions()) as unknown[];
      expect(actions.length).toBeGreaterThan(0);
      expect(actions[actions.length - 1]).toBe("concede");
    } finally {
      game.free();
    }
  });

  test("applying a legal action returns a JSON array of events", () => {
    const game = new binding.Game(fixtureSetup(), 1);
    try {
      const actions = JSON.parse(game.legal_actions()) as unknown[];
      const events = JSON.parse(game.apply(JSON.stringify(actions[0])));
      expect(Array.isArray(events)).toBe(true);
      expect(events.length).toBeGreaterThan(0);
    } finally {
      game.free();
    }
  });

  test("a refused action throws an Error whose message is the JSON {code, message}", () => {
    const game = new binding.Game(fixtureSetup(), 1);
    try {
      const e = errorOf(() => game.apply(JSON.stringify({ play: { card: 999999 } })));
      expect(e).toBeInstanceOf(Error);
      const body = JSON.parse(e.message);
      expect(body.code).toBe("card_not_in_hand");
      expect(typeof body.message).toBe("string");
    } finally {
      game.free();
    }
  });

  test("invalid JSON is a structured error, not a wasm abort", () => {
    const e = errorOf(() => new binding.Game("{", 1));
    expect(JSON.parse(e.message).code).toBe("invalid_json");
  });
});
