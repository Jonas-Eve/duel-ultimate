import { beforeAll, describe, expect, test } from "vitest";
import { EngineError } from "../src/engine/errors";
import type { GameFactory, WasmGameLike } from "../src/engine/session";
import { createWasmSession } from "../src/engine/wasm-session";
import { initTestEngine, starterConfig, testFactory } from "./support/engine";

// Carry-over of the #62 review: a non-JSON throw (a Rust panic surfaces as a WebAssembly
// RuntimeError) leaves the WASM instance unusable. The wrapper must mark the session dead and
// refuse every later call with the fatal internal_error WITHOUT reaching the object again, and
// a `free()` that throws on a failure path must never mask the original error.

beforeAll(async () => {
  await initTestEngine();
});

type Method = "start_events" | "view" | "legal_actions" | "legal_actions_with_time" | "apply";

/** The real game, except that `failing` throws a non-JSON RuntimeError (or whatever `thrower` says). */
function faultyFactory(failing: Method, opts: { thrower?: () => never; freeThrows?: boolean } = {}) {
  const calls: string[] = [];
  const thrower =
    opts.thrower ??
    (() => {
      throw new WebAssembly.RuntimeError("unreachable");
    });
  const factory: GameFactory = (...args) => {
    const game = testFactory(...args);
    const wrap =
      <A extends unknown[]>(name: Method, fn: (...a: A) => string) =>
      (...a: A): string => {
        calls.push(name);
        if (name === failing) thrower();
        return fn(...a);
      };
    const wrapped: WasmGameLike = {
      start_events: wrap("start_events", () => game.start_events()),
      view: wrap("view", (seat: string) => game.view(seat)),
      legal_actions: wrap("legal_actions", () => game.legal_actions()),
      legal_actions_with_time: wrap("legal_actions_with_time", () => game.legal_actions_with_time()),
      apply: wrap("apply", (a: string) => game.apply(a)),
      free: () => {
        calls.push("free");
        game.free();
        if (opts.freeThrows) throw new Error("null pointer passed to rust");
      },
    };
    return wrapped;
  };
  return { factory, calls };
}

const everyCall = (s: ReturnType<typeof createWasmSession>): (() => unknown)[] => [
  () => s.legalActions(),
  () => s.legalActionsWithTime(),
  () => s.view("One"),
  () => s.view("Two"),
  () => s.apply("concede"),
];

describe("a session that met a non-JSON throw is dead", () => {
  test.each<Method>(["view", "legal_actions", "legal_actions_with_time", "apply"])(
    "after a RuntimeError in %s, every call refuses with the fatal internal_error without reaching the WASM",
    (failing) => {
      const { factory, calls } = faultyFactory(failing);
      const s = createWasmSession(factory, starterConfig("rush", "bulwark", 3));
      const trigger: Record<string, () => unknown> = {
        view: () => s.view("One"),
        legal_actions: () => s.legalActions(),
        legal_actions_with_time: () => s.legalActionsWithTime(),
        apply: () => s.apply("concede"),
      };
      let first: unknown;
      try {
        trigger[failing]();
      } catch (e) {
        first = e;
      }
      expect(first).toBeInstanceOf(EngineError);
      expect((first as EngineError).code).toBe("internal_error");
      expect((first as EngineError).fatal).toBe(true);

      const reached = calls.length;
      for (const call of everyCall(s)) {
        let caught: unknown;
        try {
          call();
        } catch (e) {
          caught = e;
        }
        expect(caught).toBeInstanceOf(EngineError);
        expect((caught as EngineError).code).toBe("internal_error");
        expect((caught as EngineError).fatal).toBe(true);
      }
      expect(calls.length, "no call may reach the WASM object after the failure").toBe(reached);
      s.dispose();
    },
  );

  test("a JSON rule error does not poison the session", () => {
    const s = createWasmSession(testFactory, starterConfig("rush", "bulwark", 3));
    try {
      expect(() => s.apply({ play: { card: 999999 } })).toThrow(EngineError);
      expect(s.view("One").result).toBeNull();
      expect(s.legalActions().length).toBeGreaterThan(0);
    } finally {
      s.dispose();
    }
  });

  test("a throw of a non-Error value also kills the session", () => {
    const { factory, calls } = faultyFactory("apply", {
      thrower: () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error
        throw "boom";
      },
    });
    const s = createWasmSession(factory, starterConfig("rush", "bulwark", 3));
    expect(() => s.apply("concede")).toThrow(EngineError);
    const reached = calls.length;
    expect(() => s.view("One")).toThrow(EngineError);
    expect(calls.length).toBe(reached);
    s.dispose();
  });

  test("dispose of a dead session still frees once, and a throwing free() is swallowed", () => {
    const { factory, calls } = faultyFactory("apply", { freeThrows: true });
    const s = createWasmSession(factory, starterConfig("rush", "bulwark", 3));
    expect(() => s.apply("concede")).toThrow(EngineError);
    expect(() => s.dispose()).not.toThrow();
    expect(() => s.dispose()).not.toThrow();
    expect(calls.filter((c) => c === "free")).toHaveLength(1);
  });
});

describe("free() never masks the original error", () => {
  test("a failing start_events with a throwing free() surfaces the start_events error", () => {
    const { factory, calls } = faultyFactory("start_events", {
      freeThrows: true,
      thrower: () => {
        throw new Error(JSON.stringify({ code: "serialization_failed", message: "original" }));
      },
    });
    let caught: unknown;
    try {
      createWasmSession(factory, starterConfig("rush", "bulwark", 3));
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(EngineError);
    expect((caught as EngineError).code).toBe("serialization_failed");
    expect((caught as EngineError).message).toBe("original");
    expect(calls.filter((c) => c === "free")).toHaveLength(1);
  });
});
