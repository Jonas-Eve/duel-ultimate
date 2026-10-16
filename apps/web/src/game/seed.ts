export type SeedParse =
  | { ok: true; value: number }
  | { ok: false; reason: "empty" | "invalid" | "out_of_range" };

const todo = (): never => {
  throw new Error("not implemented");
};
export const parseSeed = (_text: string): SeedParse => ({ ok: false, reason: "invalid" });
export const randomSeed = (_crypto?: Pick<Crypto, "getRandomValues">): number => todo();
export const deriveAiSeed = (seed: number): number => seed;
export const mulberry32 = (_seed: number): (() => number) => () => 0;
