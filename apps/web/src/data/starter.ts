// The starter set, imported as text from the versioned data folder (never copied into the
// app). `DATA_VERSION` is the one version constant; `tests/data-version.test.ts` pins it
// against the imported path. Vite must be allowed to read `data/` (see `vite.config.ts`).
import cardsRaw from "../../../../data/starter/1.0.0/cards.json?raw";
import decksRaw from "../../../../data/starter/1.0.0/decks.json?raw";
import { EngineError } from "../engine/errors";

export const DATA_VERSION = "starter@1.0.0";
export const CARDS_JSON: string = cardsRaw;
export const DECKS_JSON: string = decksRaw;

export interface DeckInfo {
  id: string;
  name: string;
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
    return { id, name };
  });
}

export const DECKS: DeckInfo[] = parseDeckList(DECKS_JSON);
