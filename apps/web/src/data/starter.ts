// The starter set, imported as text from the versioned data folder (never copied into the
// app). `DATA_VERSION` is the one version constant; `tests/data-version.test.ts` pins it
// against the imported path. Vite must be allowed to read `data/` (see `vite.config.ts`).
import cardsRaw from "../../../../data/starter/1.1.0/cards.json?raw";
import decksRaw from "../../../../data/starter/1.1.0/decks.json?raw";
import { EngineError } from "../engine/errors";
import type { DeckEmblem } from "../game/controller";

export const DATA_VERSION = "starter@1.1.0";
export const CARDS_JSON: string = cardsRaw;
export const DECKS_JSON: string = decksRaw;

/**
 * Provisional deck -> symbol table (#94); it stays provisional (deck art is out of #88's scope). The one place
 * a deck id meets an emblem; a deck absent from it has no chip.
 */
export const DECK_EMBLEMS: Readonly<Record<string, DeckEmblem>> = { rush: "bolt", bulwark: "battlements" };

export interface DeckInfo {
  id: string;
  name: string;
  emblem?: DeckEmblem;
}

/** Reads the deck list for display only (no rule); a malformed file is a readable load error. */
export function parseDeckList(json: string): DeckInfo[] {
  let data: unknown;
  try {
    data = JSON.parse(json);
  } catch {
    throw new EngineError("internal_error", "decks.json is not valid JSON");
  }
  if (!Array.isArray(data) || data.length === 0) {
    throw new EngineError("internal_error", "decks.json is not a non-empty list");
  }
  return data.map((d: unknown) => {
    const { id, name } = (typeof d === "object" && d !== null ? d : {}) as { id?: unknown; name?: unknown };
    if (typeof id !== "string" || typeof name !== "string") {
      throw new EngineError("internal_error", "a deck of decks.json has no id or name");
    }
    const emblem = Object.hasOwn(DECK_EMBLEMS, id) ? DECK_EMBLEMS[id] : undefined;
    return emblem === undefined ? { id, name } : { id, name, emblem };
  });
}

export const DECKS: DeckInfo[] = parseDeckList(DECKS_JSON);
