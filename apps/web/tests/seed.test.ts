import { describe, expect, test } from "vitest";
import { deriveAiSeed, mulberry32, parseSeed, randomSeed } from "../src/game/seed";

describe("parseSeed", () => {
  test.each(["-1", "1.5", "1e3", "0x10", "4294967296", "NaN", "０１", "1 2", "+5", "abc", "99999999999"])(
    "refuses %j",
    (text) => {
      expect(parseSeed(text).ok).toBe(false);
    },
  );

  test("reasons: invalid versus out_of_range", () => {
    expect(parseSeed("4294967296")).toEqual({ ok: false, reason: "out_of_range" });
    expect(parseSeed("9999999999")).toEqual({ ok: false, reason: "out_of_range" });
    expect(parseSeed("-1")).toEqual({ ok: false, reason: "invalid" });
    expect(parseSeed("０１")).toEqual({ ok: false, reason: "invalid" });
  });

  test("empty and blank mean empty (the caller draws a random seed)", () => {
    expect(parseSeed("")).toEqual({ ok: false, reason: "empty" });
    expect(parseSeed("   ")).toEqual({ ok: false, reason: "empty" });
  });

  test("accepts the bounds and trims", () => {
    expect(parseSeed("0")).toEqual({ ok: true, value: 0 });
    expect(parseSeed("4294967295")).toEqual({ ok: true, value: 4294967295 });
    expect(parseSeed(" 42 ")).toEqual({ ok: true, value: 42 });
    expect(parseSeed("007")).toEqual({ ok: true, value: 7 });
  });
});

describe("deriveAiSeed and mulberry32 (golden values)", () => {
  test("deriveAiSeed", () => {
    expect(deriveAiSeed(0)).toBe(2462723854);
    expect(deriveAiSeed(1)).toBe(2527132011);
    expect(deriveAiSeed(4294967295)).toBe(920564995);
  });

  test("deriveAiSeed never returns its input and stays a u32", () => {
    for (let s = 0; s < 1000; s++) {
      const d = deriveAiSeed(s);
      expect(d).not.toBe(s);
      expect(Number.isInteger(d) && d >= 0 && d <= 4294967295).toBe(true);
    }
  });

  const first5 = (seed: number) => {
    const r = mulberry32(seed);
    return Array.from({ length: 5 }, () => Number(r().toFixed(10)));
  };
  test("mulberry32 of the derived seeds", () => {
    expect(first5(deriveAiSeed(0))).toEqual([0.6225702704, 0.3551594908, 0.4311869193, 0.2533018789, 0.1734426755]);
    expect(first5(deriveAiSeed(1))).toEqual([0.1657054431, 0.3910158311, 0.3940110952, 0.5869032315, 0.3471093839]);
    expect(first5(deriveAiSeed(4294967295))).toEqual([0.5223135031, 0.9062833907, 0.8789158054, 0.3977179739, 0.9664158411]);
  });

  test("mulberry32 is deterministic and in [0, 1)", () => {
    const a = mulberry32(7);
    const b = mulberry32(7);
    for (let i = 0; i < 100; i++) {
      const x = a();
      expect(x).toBe(b());
      expect(x >= 0 && x < 1).toBe(true);
    }
  });
});

describe("randomSeed", () => {
  test("uses the injected crypto and returns its u32", () => {
    const crypto = {
      getRandomValues: <T extends ArrayBufferView | null>(a: T): T => {
        (a as unknown as Uint32Array)[0] = 4000000000;
        return a;
      },
    };
    expect(randomSeed(crypto)).toBe(4000000000);
  });

  test("the default crypto gives a u32", () => {
    const s = randomSeed();
    expect(Number.isInteger(s) && s >= 0 && s <= 4294967295).toBe(true);
  });

  test("throws without crypto, never falls back to Math.random", () => {
    const original = Math.random;
    Math.random = () => {
      throw new Error("Math.random must not be used");
    };
    try {
      expect(() => randomSeed(null as unknown as Crypto)).toThrow(/crypto/);
      expect(() => randomSeed({} as unknown as Crypto)).toThrow(/crypto/);
    } finally {
      Math.random = original;
    }
  });
});
