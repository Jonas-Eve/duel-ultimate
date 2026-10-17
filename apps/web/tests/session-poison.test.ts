import { beforeAll, describe, expect, test } from "vitest";
import { EngineError } from "../src/engine/errors";
import { createWasmSession } from "../src/engine/wasm-session";
import { initTestEngine, starterConfig, testFactory } from "./support/engine";
import { faultyFactory, type Method } from "./support/faulty";

// Carry-over of the #62 review: a non-JSON throw (a Rust panic surfaces as a WebAssembly
// RuntimeError) leaves the WASM instance unusable. The wrapper must mark the session dead and
// refuse every later call with the fatal internal_error WITHOUT reaching the object again, and
// a `free()` that throws on a failure path must never mask the original error.

beforeAll(async () => {
  await initTestEngine();
});

const everyCall = (s: ReturnType<typeof createWasmSession>): (() => unknown)[] => [
  () => s.legalActions(),
  () => s.legalActionsWithTime(),
  () => s.view("One"),
  () => s.view("Two"),
  () => s.computerAction("One", 0),
  () => s.apply("concede"),
];

describe("a session that met a non-JSON throw is dead", () => {
  test.each<Method>(["view", "legal_actions", "legal_actions_with_time", "computer_action", "apply"])(
    "after a RuntimeError in %s, every call refuses with the fatal internal_error without reaching the WASM",
    (failing) => {
      const { factory, calls } = faultyFactory(failing);
      const s = createWasmSession(factory, starterConfig("rush", "bulwark", 3));
      const trigger: Record<string, () => unknown> = {
        view: () => s.view("One"),
        legal_actions: () => s.legalActions(),
        legal_actions_with_time: () => s.legalActionsWithTime(),
        computer_action: () => s.computerAction("One", 0),
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

  test("the fatal computer_stuck of the binding kills the session: nothing reaches the WASM afterwards", () => {
    const { factory, calls } = faultyFactory("computer_action", {
      thrower: () => {
        throw new Error(JSON.stringify({ code: "computer_stuck", message: "nothing but conceding" }));
      },
    });
    const s = createWasmSession(factory, starterConfig("rush", "bulwark", 3));
    let first: unknown;
    try {
      s.computerAction("One", 0);
    } catch (e) {
      first = e;
    }
    expect(first).toBeInstanceOf(EngineError);
    expect((first as EngineError).code).toBe("computer_stuck");
    expect((first as EngineError).fatal).toBe(true);
    const reached = calls.length;
    for (const call of everyCall(s)) expect(call).toThrow(expect.objectContaining({ code: "internal_error", fatal: true }));
    expect(calls.length).toBe(reached);
    s.dispose();
  });

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
