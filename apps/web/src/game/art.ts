// Presentation data of a card image (#101). Plain types: `data/` builds the lookup, `ui/` only
// receives it (through `ArtContext`), so neither needs to import the other.

/** The licences the client accepts; the closed list of `crates/cards/src/art.rs` (`ART_LICENSES`). */
export type ArtLicenseId = "CC0-1.0" | "CC-BY-4.0";

/** One image and its credit. `author` and `source` are plain text: never a link, never HTML. */
export interface ArtInfo {
  url: string;
  author: string;
  license: ArtLicenseId;
  source: string;
}

/** Finds the image of a card by its exact display name; `undefined` = no image (placeholder). */
export type ArtLookup = (name: string) => ArtInfo | undefined;
