import { beforeAll, describe, expect, test, vi } from "vitest";
import { GameController } from "../src/game/controller";
import { startGame, type StartInput } from "../src/game/start";
import { initTestEngine } from "./support/engine";
import { instrumentedFactory, realDeps } from "./support/policies";

// Carry-over of the #62 review: `randomSeed` throws without `crypto.getRandomValues`. The caller
// in this layer must turn that into a clean client result (the UI then offers the manual-seed
// path), never an unhandled exception, and must never fall back to Math.random.

beforeAll(async () => {
  await initTestEngine();
});

const input = (seedText: string, over: Partial<StartInput> = {}): StartInput => ({
  mode: "computer",
  deckOne: { id: "rush", name: "Ruée" },
  deckTwo: { id: "bulwark", name: "Rempart" },
  seedText,
  ...over,
});

const fixedCrypto = (value: number) => ({
  getRandomValues: <T extends ArrayBufferView | null>(a: T): T => {
    (a as unknown as Uint32Array)[0] = value;
    return a;
  },
});

describe("startGame", () => {
  test("an empty seed draws one from crypto and starts a game with it", () => {
    const r = startGame(input(""), realDeps(), fixedCrypto(123456));
    expect(r.ok).toBe(true);
    if (!r.ok) throw new Error("unreachable");
    expect(r.controller).toBeInstanceOf(GameController);
    expect(r.controller.getSnapshot().seed).toBe(123456);
    r.controller.dispose();
  });

  test.each([null, {} as Pick<Crypto, "getRandomValues">])(
    "no crypto.getRandomValues: a clean entropy_unavailable result, no session created, no throw, no Math.random",
    (noCrypto) => {
      const createSession = vi.fn(realDeps().createSession);
      const mathRandom = vi.spyOn(Math, "random");
      let result: ReturnType<typeof startGame> | undefined;
      expect(() => {
        result = startGame(input("   "), realDeps({ createSession }), noCrypto);
      }).not.toThrow();
      expect(result).toMatchObject({ ok: false, reason: "entropy_unavailable" });
      expect(createSession).not.toHaveBeenCalled();
      expect(mathRandom).not.toHaveBeenCalled();
      mathRandom.mockRestore();
    },
  );

  test("the manual-seed path still works without crypto", () => {
    const r = startGame(input("2"), realDeps(), null);
    expect(r.ok).toBe(true);
    if (!r.ok) throw new Error("unreachable");
    expect(r.controller.getSnapshot().seed).toBe(2);
    r.controller.dispose();
  });

  test.each(["abc", "-1", "1.5", "4294967296", "99999999999"])("the typed seed %j is refused as invalid_seed before any session", (text) => {
    const createSession = vi.fn(realDeps().createSession);
    const r = startGame(input(text), realDeps({ createSession }), fixedCrypto(1));
    expect(r).toMatchObject({ ok: false, reason: "invalid_seed" });
    if (r.ok) throw new Error("unreachable");
    expect(r.error?.code).toBe("invalid_seed");
    expect(createSession).not.toHaveBeenCalled();
  });

  test("an engine creation failure is a result, not a throw, and leaks nothing", () => {
    const { factory, counters } = instrumentedFactory();
    const r = startGame(input("5", { deckOne: { id: "nope", name: "?" } }), realDeps({}, factory), null);
    expect(r).toMatchObject({ ok: false, reason: "engine" });
    if (r.ok) throw new Error("unreachable");
    expect(r.error?.code).toBe("unknown_deck");
    expect(counters.frees).toBe(counters.creations);
  });

  test("the two-camps mode starts the same way", () => {
    const r = startGame(input("5", { mode: "two-camps" }), realDeps(), null);
    expect(r.ok).toBe(true);
    if (!r.ok) throw new Error("unreachable");
    expect(r.controller.getSnapshot().humanSeat).toBeNull();
    r.controller.dispose();
  });
});
