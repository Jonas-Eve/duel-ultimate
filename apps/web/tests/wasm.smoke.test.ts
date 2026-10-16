import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import { repoRoot, webRoot } from "./support/paths";

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

const fixture = (name: string) => readFileSync(resolve(repoRoot, "data/fixture/1.0.0", name), "utf8");
const VERSIONS = JSON.stringify(["fixture@1.0.0"]);

/** The `fixture` set, `aggro` against `control`: what the client builds a game from. */
function newGame(seed = 1) {
  return binding.Game.from_data(fixture("cards.json"), fixture("decks.json"), "aggro", "control", VERSIONS, seed);
}

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
    const game = newGame();
    try {
      const actions = JSON.parse(game.legal_actions()) as unknown[];
      expect(actions.length).toBeGreaterThan(0);
      expect(actions[actions.length - 1]).toBe("concede");
    } finally {
      game.free();
    }
  });

  test("applying a legal action returns a JSON array of events", () => {
    const game = newGame();
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
    const game = newGame();
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

  test("from_data records the versions in the start events", () => {
    const game = newGame();
    try {
      const events = JSON.parse(game.start_events());
      expect(events[0].game_started.data_versions).toEqual(["fixture@1.0.0"]);
    } finally {
      game.free();
    }
  });

  test("from_data with an unknown deck throws the structured unknown_deck error", () => {
    const e = errorOf(() =>
      binding.Game.from_data(fixture("cards.json"), fixture("decks.json"), "nope", "control", VERSIONS, 1),
    );
    expect(e).toBeInstanceOf(Error);
    expect(JSON.parse(e.message).code).toBe("unknown_deck");
  });

  test("from_data with malformed data throws invalid_json, not a TypeError", () => {
    const e = errorOf(() => binding.Game.from_data("{", fixture("decks.json"), "aggro", "control", VERSIONS, 1));
    expect(JSON.parse(e.message).code).toBe("invalid_json");
    const v = errorOf(() =>
      binding.Game.from_data(fixture("cards.json"), fixture("decks.json"), "aggro", "control", "not json", 1),
    );
    expect(JSON.parse(v.message).code).toBe("invalid_json");
  });

  test("a game built by from_data is the same game for the same seed", () => {
    const a = newGame(7);
    const b = newGame(7);
    try {
      expect(a.start_events()).toBe(b.start_events());
      expect(a.legal_actions()).toBe(b.legal_actions());
    } finally {
      a.free();
      b.free();
    }
  });
});
