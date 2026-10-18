import { beforeAll, describe, expect, test } from "vitest";
import { EngineError, SessionDisposedError } from "../src/engine/errors";
import type { GameFactory, GameSession } from "../src/engine/session";
import { createWasmSession } from "../src/engine/wasm-session";
import { DATA_VERSION } from "../src/data/starter";
import { PAIRS, initTestEngine, instrumentedFactory, newSession, randomPolicy, starterConfig, testFactory } from "./support/engine";

beforeAll(async () => {
  await initTestEngine();
});

describe("creation from the starter set", () => {
  test.each(PAIRS)("%s against %s", (one, two) => {
    const s = newSession(one, two, 5);
    try {
      expect(s.startEvents.length).toBeGreaterThan(0);
      expect(s.startEvents[0]).toMatchObject({ game_started: { data_versions: [DATA_VERSION] } });
      const v = s.view("One");
      expect(v.seat).toBe("One");
      expect(v.result).toBeNull();
      expect(s.view("Two").seat).toBe("Two");
    } finally {
      s.dispose();
    }
  });

  test("the initiative decides who opens: rush/bulwark -> One, bulwark/rush -> Two", () => {
    for (let seed = 0; seed < 20; seed++) {
      const a = newSession("rush", "bulwark", seed);
      const b = newSession("bulwark", "rush", seed);
      try {
        expect(a.view("One").active).toBe("One");
        expect(b.view("One").active).toBe("Two");
      } finally {
        a.dispose();
        b.dispose();
      }
    }
  });

  test.each(["rush", "bulwark"])("mirror %s: over 40 seeds both seats open", (deck) => {
    const opening = new Set<string>();
    for (let seed = 0; seed < 40; seed++) {
      const s = newSession(deck, deck, seed);
      opening.add(s.view("One").active);
      s.dispose();
    }
    expect(opening).toEqual(new Set(["One", "Two"]));
  });
});

describe("legal actions", () => {
  test("legalActionsWithTime has the same length and order as legalActions, and replays as is", () => {
    const s = newSession("rush", "bulwark", 9);
    try {
      const pick = randomPolicy(9);
      for (let i = 0; i < 40 && s.view("One").result === null; i++) {
        const raw = s.legalActions();
        const timed = s.legalActionsWithTime();
        expect(timed.map((p) => p.action)).toEqual(raw);
        const wanted = JSON.stringify(pick(raw));
        const chosen = timed.find((p) => JSON.stringify(p.action) === wanted)!;
        const events = s.apply(chosen.action);
        expect(events.length).toBeGreaterThan(0);
      }
    } finally {
      s.dispose();
    }
  });

  test("a refused action throws an EngineError and leaves the state intact", () => {
    const s = newSession("rush", "bulwark", 9);
    try {
      const before = JSON.stringify(s.view("One"));
      let caught: unknown;
      try {
        s.apply({ play: { card: 999999 } });
      } catch (e) {
        caught = e;
      }
      expect(caught).toBeInstanceOf(EngineError);
      expect((caught as EngineError).code).toBe("card_not_in_hand");
      expect(JSON.stringify(s.view("One"))).toBe(before);
    } finally {
      s.dispose();
    }
  });
});

describe("computerAction", () => {
  const active = (s: GameSession) => s.view("One").active;

  test("returns an action the engine accepts, never concede, from the active seat", () => {
    for (let seed = 0; seed < 10; seed++) {
      const s = newSession("rush", "bulwark", seed);
      try {
        for (let i = 0; i < 300 && s.view("One").result === null; i++) {
          const legal = s.legalActions();
          const action = s.computerAction(active(s), (seed * 31 + i * 7919) >>> 0);
          expect(action).not.toBe("concede");
          expect(legal).toContainEqual(action);
          s.apply(action);
        }
      } finally {
        s.dispose();
      }
    }
  });

  test("the same state, seat and draw give the same action, and nothing moves", () => {
    const s = newSession("rush", "bulwark", 4);
    try {
      const view = JSON.stringify(s.view("One"));
      const legal = JSON.stringify(s.legalActions());
      const first = s.computerAction(active(s), 123);
      for (let i = 0; i < 5; i++) expect(s.computerAction(active(s), 123)).toEqual(first);
      expect(JSON.stringify(s.view("One"))).toBe(view);
      expect(JSON.stringify(s.legalActions())).toBe(legal);
    } finally {
      s.dispose();
    }
  });

  test("greedy does not depend on the draw: over 300 draws the same action comes out (#126)", () => {
    const s = newSession("rush", "bulwark", 4);
    try {
      const seen = new Set<string>();
      for (let draw = 0; draw < 300; draw++) seen.add(JSON.stringify(s.computerAction(active(s), draw)));
      expect(seen.size).toBe(1);
    } finally {
      s.dispose();
    }
  });

  test("the seat that does not hold the main is refused with not_active_seat (not fatal)", () => {
    const s = newSession("rush", "bulwark", 4);
    try {
      const idle = active(s) === "One" ? "Two" : "One";
      let caught: unknown;
      try {
        s.computerAction(idle, 0);
      } catch (e) {
        caught = e;
      }
      expect(caught).toBeInstanceOf(EngineError);
      expect((caught as EngineError).code).toBe("not_active_seat");
      expect((caught as EngineError).fatal).toBe(false);
      expect(s.computerAction(active(s), 0)).not.toBe("concede");
    } finally {
      s.dispose();
    }
  });

  test("after the end of the game: game_over", () => {
    const s = newSession("rush", "bulwark", 4);
    try {
      s.apply("concede");
      expect(() => s.computerAction("One", 0)).toThrow(expect.objectContaining({ code: "game_over" }));
    } finally {
      s.dispose();
    }
  });

  test.each([-1, 1.5, 4294967296, Number.NaN, Number.POSITIVE_INFINITY])("draw %s is refused before the binding, never reduced in silence", (draw) => {
    const { factory, counters } = instrumentedFactory();
    const calls: number[] = [];
    const spying: GameFactory = (...args) => {
      const game = factory(...args);
      const original = game.computer_action.bind(game);
      game.computer_action = (seat, policy, d) => {
        calls.push(d);
        return original(seat, policy, d);
      };
      return game;
    };
    const s = newSession("rush", "bulwark", 4, spying);
    try {
      let caught: unknown;
      try {
        s.computerAction(active(s), draw);
      } catch (e) {
        caught = e;
      }
      expect(caught).toBeInstanceOf(EngineError);
      expect((caught as EngineError).code).toBe("internal_error");
      expect(calls).toEqual([]);
      // The refusal does not kill the session: a valid draw still works.
      expect(s.computerAction(active(s), 0)).not.toBe("concede");
      expect(calls).toEqual([0]);
    } finally {
      s.dispose();
    }
    expect(counters.callsAfterFree).toBe(0);
  });

  test("the bounds 0 and 4294967295 are accepted", () => {
    const s = newSession("rush", "bulwark", 4);
    try {
      for (const draw of [0, 4294967295]) expect(s.computerAction(active(s), draw)).not.toBe("concede");
    } finally {
      s.dispose();
    }
  });

  test("the policy sent to the binding is the closed constant \"greedy\"", () => {
    const seen: string[] = [];
    const spying: GameFactory = (...args) => {
      const game = testFactory(...args);
      const original = game.computer_action.bind(game);
      game.computer_action = (seat, policy, d) => {
        seen.push(policy);
        return original(seat, policy, d);
      };
      return game;
    };
    const s = newSession("rush", "bulwark", 4, spying);
    try {
      s.computerAction(active(s), 1);
      expect(seen).toEqual(["greedy"]);
    } finally {
      s.dispose();
    }
  });
});

describe("seed validation", () => {
  test.each([4294967296, -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY])("seed %s is refused before the factory", (seed) => {
    const calls: number[] = [];
    const factory: GameFactory = (...args) => {
      calls.push(args[5]);
      return testFactory(...args);
    };
    let caught: unknown;
    try {
      createWasmSession(factory, starterConfig("rush", "bulwark", seed));
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(EngineError);
    expect((caught as EngineError).code).toBe("invalid_seed");
    expect(calls).toEqual([]);
  });

  test("the bounds 0 and 4294967295 are accepted", () => {
    for (const seed of [0, 4294967295]) newSession("rush", "bulwark", seed).dispose();
  });

  test("the factory receives the versions as a JSON string of an array", () => {
    let received: string | undefined;
    const factory: GameFactory = (c, d, one, two, versions, seed) => {
      received = versions;
      return testFactory(c, d, one, two, versions, seed);
    };
    newSession("rush", "bulwark", 1, factory).dispose();
    expect(received).toBe(JSON.stringify([DATA_VERSION]));
    expect(typeof received).toBe("string");
  });

  test("creation errors are EngineErrors (unknown deck)", () => {
    try {
      newSession("nope", "rush", 1);
      throw new Error("expected a throw");
    } catch (e) {
      expect(e).toBeInstanceOf(EngineError);
      expect((e as EngineError).code).toBe("unknown_deck");
    }
  });
});

describe("lifecycle", () => {
  test("dispose frees exactly once, even called three times", () => {
    const { factory, counters } = instrumentedFactory();
    const s = newSession("rush", "bulwark", 1, factory);
    s.dispose();
    s.dispose();
    s.dispose();
    expect(counters.creations).toBe(1);
    expect(counters.frees).toBe(1);
    expect(counters.callsAfterFree).toBe(0);
  });

  test("every method after dispose throws SessionDisposedError without reaching the WASM", () => {
    const { factory, counters } = instrumentedFactory();
    const s: GameSession = newSession("rush", "bulwark", 1, factory);
    s.dispose();
    const calls: (() => unknown)[] = [
      () => s.legalActions(),
      () => s.legalActionsWithTime(),
      () => s.view("One"),
      () => s.computerAction("One", 0),
      () => s.apply("concede"),
    ];
    for (const call of calls) expect(call).toThrow(SessionDisposedError);
    expect(counters.callsAfterFree).toBe(0);
  });

  test("N creations and disposals free exactly N times", () => {
    const { factory, counters } = instrumentedFactory();
    for (let i = 0; i < 25; i++) newSession("rush", "bulwark", i, factory).dispose();
    expect(counters.creations).toBe(25);
    expect(counters.frees).toBe(25);
  });

  test("a failed creation does not leak a game", () => {
    const { factory, counters } = instrumentedFactory();
    expect(() => newSession("nope", "rush", 1, factory)).toThrow(EngineError);
    expect(counters.creations).toBe(counters.frees);
  });
});
