import type { GameConfig } from "../../src/game/controller";

export const NAMES: Record<string, string> = { rush: "Ruée", bulwark: "Rempart" };

export const config = (one: string, two: string, seed: number): GameConfig => ({
  mode: "computer",
  deckOne: { id: one, name: NAMES[one] },
  deckTwo: { id: two, name: NAMES[two] },
  seed,
});
