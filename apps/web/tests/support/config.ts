import type { DeckEmblem, GameConfig } from "../../src/game/controller";

export const NAMES: Record<string, string> = { rush: "Ruée", bulwark: "Rempart" };

/** The emblem the starter data gives each deck (the table of `data/starter.ts`, spelled out here on purpose). */
export const EMBLEMS: Record<string, DeckEmblem> = { rush: "bolt", bulwark: "battlements" };

export const config = (one: string, two: string, seed: number): GameConfig => ({
  mode: "computer",
  deckOne: { id: one, name: NAMES[one], emblem: EMBLEMS[one] },
  deckTwo: { id: two, name: NAMES[two], emblem: EMBLEMS[two] },
  seed,
});
