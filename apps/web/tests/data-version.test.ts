import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import { CARDS_JSON, DATA_VERSION, DECKS, DECKS_JSON, parseDeckList } from "../src/data/starter";
import { EngineError } from "../src/engine/errors";
import { initTestEngine, newSession } from "./support/engine";
import { repoRoot, webRoot } from "./support/paths";

beforeAll(async () => {
  await initTestEngine();
});

const source = readFileSync(resolve(webRoot, "src/data/starter.ts"), "utf8");

describe("the starter data and its version", () => {
  test("the source imports the versioned folder of the repository", () => {
    expect(source).toContain("data/starter/1.0.0/");
    for (const f of ["cards.json", "decks.json"]) expect(existsSync(resolve(repoRoot, "data/starter/1.0.0", f))).toBe(true);
  });

  test("DATA_VERSION follows the imported path (the two cannot diverge)", () => {
    const paths = [...source.matchAll(/data\/starter\/(\d+\.\d+\.\d+)\//g)].map((m) => m[1]);
    expect(paths.length).toBe(2);
    expect(new Set(paths).size).toBe(1);
    expect(DATA_VERSION).toBe(`starter@${paths[0]}`);
    expect(DATA_VERSION).toBe("starter@1.0.0");
  });

  test("the imported text is the file on disk", () => {
    expect(CARDS_JSON).toBe(readFileSync(resolve(repoRoot, "data/starter/1.0.0/cards.json"), "utf8"));
    expect(DECKS_JSON).toBe(readFileSync(resolve(repoRoot, "data/starter/1.0.0/decks.json"), "utf8"));
  });

  test("DECKS lists rush then bulwark", () => {
    expect(DECKS).toEqual([
      { id: "rush", name: "Ruée", emblem: "bolt" },
      { id: "bulwark", name: "Rempart", emblem: "battlements" },
    ]);
  });

  test("the emblem of each deck: Ruée = bolt, Rempart = battlements, one table, no other deck", () => {
    expect(Object.fromEntries(DECKS.map((d) => [d.id, d.emblem]))).toEqual({ rush: "bolt", bulwark: "battlements" });
    expect(parseDeckList('[{"id":"unknown","name":"X"}]')).toEqual([{ id: "unknown", name: "X" }]);
    expect(source.match(/DECK_EMBLEMS/g)?.length).toBeGreaterThanOrEqual(1);
  });

  test("the version the engine records is DATA_VERSION", () => {
    const s = newSession("rush", "bulwark", 1);
    try {
      expect(s.startEvents[0]).toMatchObject({ game_started: { data_versions: [DATA_VERSION] } });
    } finally {
      s.dispose();
    }
  });
});

describe("parseDeckList", () => {
  test.each(["{", "{}", "[]", '[{"id":"a"}]', '[{"id":1,"name":"x"}]', "[null]"])("rejects %s with a readable error", (json) => {
    expect(() => parseDeckList(json)).toThrow(EngineError);
    try {
      parseDeckList(json);
    } catch (e) {
      expect((e as EngineError).code).toBe("internal_error");
    }
  });

  test("keeps only id and name", () => {
    expect(parseDeckList('[{"id":"a","name":"A","hero":{}}]')).toEqual([{ id: "a", name: "A" }]);
  });
});
