import { describe, expect, test } from "vitest";
import { EngineError } from "../src/engine/errors";
import type { Action } from "../src/engine/types";
import {
  COMPUTER_SLICE_MS,
  COMPUTER_STEP_DELAY_MS,
  MAX_COMPUTER_ACTIONS_PER_CHAIN,
  pickComputerAction,
  randomPolicy,
  type ComputerPolicy,
} from "../src/game/computer";
import { mulberry32 } from "../src/game/seed";

const play = (card: number): Action => ({ play: { card } });
const actions = (n: number): Action[] => Array.from({ length: n }, (_, i) => play(i));

describe("constants", () => {
  test("the named limits of the design", () => {
    expect(MAX_COMPUTER_ACTIONS_PER_CHAIN).toBe(1000);
    expect(COMPUTER_SLICE_MS).toBe(12);
    expect(COMPUTER_STEP_DELAY_MS).toBe(0);
  });
});

describe("randomPolicy", () => {
  test("never returns an index out of bounds, even at the edges of rng", () => {
    for (const n of [1, 2, 5, 83, 931]) {
      for (const r of [0, 0.5, 0.999999999767169, 1 - 2 ** -32]) {
        const i = randomPolicy(actions(n), () => r);
        expect(Number.isInteger(i) && i >= 0 && i < n, `n=${n} r=${r} -> ${i}`).toBe(true);
      }
    }
  });

  test("every index is reachable (n = 5, 2000 draws), the last one included", () => {
    const rng = mulberry32(1);
    const seen = new Set<number>();
    for (let k = 0; k < 2000; k++) seen.add(randomPolicy(actions(5), rng));
    expect([...seen].sort()).toEqual([0, 1, 2, 3, 4]);
  });

  test("the extremes of rng pick the first and the last action", () => {
    expect(randomPolicy(actions(7), () => 0)).toBe(0);
    expect(randomPolicy(actions(7), () => 1 - 2 ** -32)).toBe(6);
  });

  test("same seed, same choices", () => {
    const run = (seed: number) => {
      const rng = mulberry32(seed);
      return Array.from({ length: 50 }, () => randomPolicy(actions(9), rng));
    };
    expect(run(7)).toEqual(run(7));
    expect(run(7)).not.toEqual(run(8));
  });
});

describe("pickComputerAction", () => {
  test("concede is never offered to the policy, wherever it sits in the raw list", () => {
    for (const raw of [["concede", play(1), play(2)], [play(1), "concede", play(2)], [play(1), play(2), "concede"]] as Action[][]) {
      const offered: Action[][] = [];
      const spy: ComputerPolicy = (a) => {
        offered.push([...a]);
        return 0;
      };
      const rng = () => 0;
      const picked = pickComputerAction(raw, spy, rng);
      expect(offered).toHaveLength(1);
      expect(offered[0]).toEqual([play(1), play(2)]);
      expect(picked).not.toBe("concede");
      // And a greedy policy asking for the last index still cannot reach it.
      expect(pickComputerAction(raw, (a) => a.length - 1, rng)).toEqual(play(2));
    }
  });

  test("only concede left is the explicit computer_stuck error, never a concession", () => {
    let called = false;
    try {
      pickComputerAction(["concede"], () => {
        called = true;
        return 0;
      }, () => 0);
      throw new Error("expected a throw");
    } catch (e) {
      expect(e).toBeInstanceOf(EngineError);
      expect((e as EngineError).code).toBe("computer_stuck");
      expect((e as EngineError).fatal).toBe(true);
    }
    expect(called).toBe(false);
    expect(() => pickComputerAction([], randomPolicy, () => 0)).toThrow(EngineError);
  });

  test.each([-1, 3, 4, 1.5, Number.NaN, Number.POSITIVE_INFINITY])("an index %s from the policy is a fatal internal_error", (bad) => {
    try {
      pickComputerAction(actions(3), () => bad, () => 0);
      throw new Error("expected a throw");
    } catch (e) {
      expect(e).toBeInstanceOf(EngineError);
      expect((e as EngineError).code).toBe("internal_error");
      expect((e as EngineError).fatal).toBe(true);
    }
  });

  test("returns the action at the index chosen among the playable ones", () => {
    expect(pickComputerAction([play(5), "concede", play(6), play(7)], () => 1, () => 0)).toEqual(play(6));
  });
});
