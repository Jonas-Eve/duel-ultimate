// The optional art of the starter set (`art.json` and `art/`), read from the versioned data
// folder, never copied into the app. Display only: no rule, the engine never sees it. Both globs
// tolerate an absent file (the set has no art yet): a static `?raw` import would break the build.
// Literal paths, same version as `DATA_VERSION` (`tests/art.test.ts` pins it). `App` is the only
// reader of this module; `ui/` gets the lookup through `ArtContext`.
import type { ArtInfo, ArtLicenseId, ArtLookup } from "../game/art";
import { CARDS_JSON } from "./starter";

const rawArt = import.meta.glob("../../../../data/starter/1.1.0/art.json", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const imageUrls = import.meta.glob("../../../../data/starter/1.1.0/art/*.{webp,svg}", { query: "?url", import: "default", eager: true }) as Record<string, string>;

/** Same closed list as `ART_LICENSES` in `crates/cards/src/art.rs` (pinned by `tests/art.test.ts`). */
export const ART_LICENSES: readonly ArtLicenseId[] = ["CC0-1.0", "CC-BY-4.0"];

const isLicense = (v: unknown): v is ArtLicenseId => typeof v === "string" && (ART_LICENSES as readonly string[]).includes(v);
const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/** Parses `text` as JSON, `undefined` when it is not. */
function parse(text: string | undefined): unknown {
  if (text === undefined) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

/** Card id -> display name, for display only. A card without a string id and name is skipped. */
function namesById(cardsJson: string): Map<string, string> {
  const out = new Map<string, string>();
  const data = parse(cardsJson);
  if (!Array.isArray(data)) return out;
  for (const c of data) {
    if (isRecord(c) && typeof c.id === "string" && typeof c.name === "string") out.set(c.id, c.name);
  }
  return out;
}

/**
 * Builds the lookup by display name. Pure and total: it never throws, a bad entry is ignored and
 * a name without a usable image has no art (the placeholder stays). `file` is never turned into a
 * URL: it is only a key into `images` (base name -> URL), a closed table built by the bundler.
 * The rules (licence list, file names, lengths) live in Rust (`parse_art`); only shapes are read here.
 */
export function buildArtIndex(cardsJson: string, artJson: string | undefined, images: Readonly<Record<string, string>>): ArtLookup {
  const table = new Map(Object.entries(images));
  const names = namesById(cardsJson);
  const perName = new Map<string, number>();
  for (const name of names.values()) perName.set(name, (perName.get(name) ?? 0) + 1);

  const parsed = parse(artJson);
  const entries = Array.isArray(parsed) ? parsed : [];
  const perId = new Map<string, number>();
  for (const e of entries) if (isRecord(e) && typeof e.id === "string") perId.set(e.id, (perId.get(e.id) ?? 0) + 1);

  const index = new Map<string, ArtInfo>();
  for (const e of entries) {
    if (!isRecord(e)) continue;
    const { id, file, author, license, source } = e;
    if (typeof id !== "string" || typeof file !== "string" || typeof author !== "string" || typeof source !== "string" || !isLicense(license)) continue;
    if (perId.get(id) !== 1) continue; // two entries for one card: ambiguous
    const name = names.get(id);
    if (name === undefined || perName.get(name) !== 1) continue; // unknown card, or a name shared by several ids
    if (/[/\\:]/.test(file)) continue; // a key, never a path or a URL
    const url = table.get(file);
    if (url === undefined) continue;
    index.set(name, { url, author, license, source });
  }
  return (name) => (typeof name === "string" ? index.get(name) : undefined);
}

const baseName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

export const ART: ArtLookup = buildArtIndex(
  CARDS_JSON,
  Object.values(rawArt)[0],
  Object.fromEntries(Object.entries(imageUrls).map(([path, url]) => [baseName(path), url])),
);
