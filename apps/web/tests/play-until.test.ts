import { beforeAll, describe, expect, test } from "vitest";
import { GameController } from "../src/game/controller";
import { initTestEngine } from "./support/engine";
import { playUntil, randomPreview, realDeps } from "./support/policies";
import { config } from "./support/config";

beforeAll(async () => {
  await initTestEngine();
});

const fresh = (seed = 3) => {
  const c = GameController.create(config("rush", "bulwark", seed), realDeps());
  c.start();
  return c;
};

describe("playUntil (test helper reused by #65, #66 and #67)", () => {
  test("stops as soon as the predicate holds and returns the steps and the snapshot", async () => {
    const c = fresh();
    try {
      const r = await playUntil(c, randomPreview(1), (s) => (s.views.One?.you.board.length ?? 0) >= 2, 50);
      expect(r.steps).toBeGreaterThanOrEqual(2);
      expect(r.snapshot).toBe(c.getSnapshot());
      expect(r.snapshot.views.One?.you.board.length).toBeGreaterThanOrEqual(2);
      // Not one action more than needed: one fewer step would not have satisfied it.
      expect(r.snapshot.records.filter((x) => x.by === "human")).toHaveLength(r.steps);
    } finally {
      c.dispose();
    }
  });

  test("zero steps when the predicate already holds", async () => {
    const c = fresh();
    try {
      const r = await playUntil(c, randomPreview(1), () => true, 5);
      expect(r.steps).toBe(0);
      expect(c.getSnapshot().records).toHaveLength(1);
    } finally {
      c.dispose();
    }
  });

  test("fails explicitly after maxSteps, having played exactly maxSteps actions", async () => {
    const c = fresh();
    try {
      await expect(playUntil(c, randomPreview(1), () => false, 3)).rejects.toThrow(/maxSteps = 3/);
      expect(c.getSnapshot().records.filter((x) => x.by === "human")).toHaveLength(3);
    } finally {
      c.dispose();
    }
  });

  test("fails explicitly when the game ends before the predicate holds", async () => {
    const c = fresh();
    try {
      c.concede();
      await expect(playUntil(c, randomPreview(1), () => false, 10)).rejects.toThrow(/ended \(over\)/);
    } finally {
      c.dispose();
    }
  });
});
