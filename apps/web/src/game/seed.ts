// Seeds: pure, no React. The seed is the only free input of the product; the binding would
// silently reduce a value outside u32, so it is validated here.

export type SeedParse =
  | { ok: true; value: number }
  | { ok: false; reason: "empty" | "invalid" | "out_of_range" };

const U32_MAX = 4294967295;

export function parseSeed(text: string): SeedParse {
  const t = text.trim();
  if (t === "") return { ok: false, reason: "empty" };
  if (!/^[0-9]{1,10}$/.test(t)) return { ok: false, reason: "invalid" };
  const value = Number(t);
  if (value > U32_MAX) return { ok: false, reason: "out_of_range" };
  return { ok: true, value };
}

/** The client's only entropy source (the engine has its own seed). Never falls back to `Math.random`. */
export function randomSeed(crypto: Pick<Crypto, "getRandomValues"> | null | undefined = globalThis.crypto): number {
  if (typeof crypto?.getRandomValues !== "function") {
    throw new Error("crypto.getRandomValues is unavailable");
  }
  return crypto.getRandomValues(new Uint32Array(1))[0];
}

/** The computer's seed, derived from the game seed (SplitMix32-style finaliser). */
export function deriveAiSeed(seed: number): number {
  let z = (seed + 0x9e3779b9) >>> 0;
  z = Math.imul(z ^ (z >>> 16), 0x85ebca6b) >>> 0;
  z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35) >>> 0;
  return (z ^ (z >>> 16)) >>> 0;
}

/** Small deterministic generator in [0, 1). */
export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
