// Review of #70, points 1 and 2: the controller must be usable exactly as #64 will write it
// (`useSyncExternalStore(controller.subscribe, controller.getSnapshot)`, methods passed
// detached), and `dispose()` called from a subscriber must stop everything, even while
// `start()` or `submit()` is still running.
import { act, renderHook } from "@testing-library/react";
import { StrictMode, useSyncExternalStore } from "react";
import { beforeAll, describe, expect, test, vi } from "vitest";
import type { ActionPreview } from "../src/engine/types";
import { GameController, type GameConfig } from "../src/game/controller";
import { decorateSession, initTestEngine, newSession } from "./support/engine";
import { realDeps } from "./support/policies";

beforeAll(async () => {
  await initTestEngine();
});

const NAMES: Record<string, string> = { rush: "Ruée", bulwark: "Rempart" };
const config = (one: string, two: string, seed: number): GameConfig => ({
  mode: "computer",
  deckOne: { id: one, name: NAMES[one] },
  deckTwo: { id: two, name: NAMES[two] },
  seed,
});
const flush = async (): Promise<void> => {
  for (let i = 0; i < 20; i++) await Promise.resolve();
};

describe("subscribe and getSnapshot used detached (point 1)", () => {
  test("destructured: subscribe, publication, unsubscription, stable snapshot identity", () => {
    const c = GameController.create(config("rush", "bulwark", 3), realDeps());
    try {
      const { subscribe, getSnapshot } = c;
      const fn = vi.fn();
      const unsubscribe = subscribe(fn);
      c.start();
      expect(fn).toHaveBeenCalledTimes(1);
      const first = getSnapshot();
      expect(getSnapshot()).toBe(first);
      expect(first).toBe(c.getSnapshot());
      c.submit(first.legal[0]);
      expect(fn).toHaveBeenCalledTimes(2);
      const second = getSnapshot();
      expect(second).not.toBe(first);
      expect(getSnapshot()).toBe(second);
      unsubscribe();
      c.clearError();
      c.submit(second.legal[0]);
      expect(fn).toHaveBeenCalledTimes(2);
    } finally {
      c.dispose();
    }
  });

  test("detached subscribe after dispose does not throw and returns a no-op unsubscribe", () => {
    const c = GameController.create(config("rush", "bulwark", 3), realDeps());
    c.dispose();
    const { subscribe } = c;
    expect(() => subscribe(() => undefined)()).not.toThrow();
  });

  test.each([
    ["plain", false],
    ["StrictMode", true],
  ])("useSyncExternalStore(controller.subscribe, controller.getSnapshot), %s: renders and follows the controller", (_n, strict) => {
    const c = GameController.create(config("rush", "bulwark", 3), realDeps());
    try {
      const { result } = renderHook(() => useSyncExternalStore(c.subscribe, c.getSnapshot), strict ? { wrapper: StrictMode } : {});
      expect(result.current).toBe(c.getSnapshot());
      act(() => c.start());
      const before = result.current;
      expect(before.records).toHaveLength(1);
      act(() => {
        c.submit(before.legal[0]);
      });
      expect(result.current).toBe(c.getSnapshot());
      expect(result.current).not.toBe(before);
      expect(result.current.records.length).toBeGreaterThan(1);
    } finally {
      c.dispose();
    }
  });
});

describe("dispose() from a subscriber (point 2)", () => {
  test("during start() when the computer opens: no session call afterwards", async () => {
    const { session, probe } = decorateSession(newSession("bulwark", "rush", 3));
    const c = GameController.create(config("bulwark", "rush", 3), realDeps({ createSession: () => session }));
    c.subscribe(() => c.dispose());
    c.start();
    await flush();
    expect(probe.calls.dispose).toBe(1);
    expect(probe.calls.legalActions).toBe(0);
    expect(probe.calls.apply).toBe(0);
    expect(probe.calls.legalActionsWithTime).toBe(0);
  });

  test("during submit() when the action hands the main over: no session call afterwards", async () => {
    // Find a (seed, preview index) where the human's action hands the main to the computer.
    let found: { seed: number; index: number } | null = null;
    for (let seed = 0; seed < 30 && found === null; seed++) {
      const probeCtl = GameController.create(config("rush", "bulwark", seed), realDeps());
      probeCtl.start();
      const index = probeCtl.getSnapshot().legal.findIndex((p: ActionPreview) => p.hands_over && p.ends_game === null);
      probeCtl.dispose();
      if (index >= 0) found = { seed, index };
    }
    expect(found).not.toBeNull();
    const { seed, index } = found as { seed: number; index: number };

    const { session, probe } = decorateSession(newSession("rush", "bulwark", seed));
    const c = GameController.create(config("rush", "bulwark", seed), realDeps({ createSession: () => session }));
    c.start();
    c.subscribe(() => c.dispose());
    const before = { ...probe.calls };
    expect(c.submit(c.getSnapshot().legal[index])).toMatchObject({ ok: true });
    await flush();
    expect(probe.calls.dispose).toBe(1);
    expect(probe.calls.apply).toBe(before.apply + 1); // the human's own action, nothing else
    expect(probe.calls.legalActions).toBe(before.legalActions);
  });
});
