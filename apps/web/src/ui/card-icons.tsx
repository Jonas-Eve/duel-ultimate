// The drawn parts of a card, shared by the compact face (`CardTile`) and the detail sheet
// (`CardDetail`): icons, the deck symbol and the art slots. Pure drawing, no state.
import type { DeckEmblem } from "../game/controller";
import { useArtLookup } from "./ArtContext";

/** Every icon is decorative (aria-hidden): each value is also a number, each state also a text. */
interface IconProps {
  /** Side in px; 8 on the compact face, larger on the detail sheet. */
  size?: number;
}

export function StarIcon({ size = 8 }: IconProps) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width={size} height={size} className="icon" fill="currentColor">
      <path d="M6 0l1.6 4.2L12 4.6 8.6 7.4 9.7 12 6 9.4 2.3 12l1.1-4.6L0 4.6l4.4-.4z" />
    </svg>
  );
}

export function BackArrowIcon({ size = 8 }: IconProps) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width={size} height={size} className="icon" fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d="M10 9V6H3M5 3 2.5 6 5 9" />
    </svg>
  );
}

export function HeartIcon({ size = 8 }: IconProps) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width={size} height={size} className="icon" fill="currentColor">
      <path d="M6 11C1 7.5 0 5 0 3.4A3 3 0 0 1 6 2.2 3 3 0 0 1 12 3.4C12 5 11 7.5 6 11z" />
    </svg>
  );
}

export function HourglassIcon({ size = 8 }: IconProps) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width={size} height={size} className="icon" fill="none" stroke="currentColor" strokeWidth="1.5">
      <path d="M2.5 1h7M2.5 11h7M3 1c0 3 3 3 3 5s-3 2-3 5M9 1c0 3-3 3-3 5s3 2 3 5" />
    </svg>
  );
}

export function ShieldIcon({ size = 8 }: IconProps) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width={size} height={size} className="icon" fill="currentColor">
      <path d="M6 0 11 2v4c0 3-2.5 5-5 6-2.5-1-5-3-5-6V2z" />
    </svg>
  );
}

/** Two bars, a unit that does not move: the "Immobile" mark under the card. */
export function ImmobileIcon({ size = 8 }: IconProps) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width={size} height={size} className="icon" fill="none" stroke="currentColor" strokeWidth="2.4">
      <path d="M3.5 1.5v9M8.5 1.5v9" />
    </svg>
  );
}

/** Distinct from the heart (a broken cross on a square): the life slot of a destroyed card. */
export function DestroyedIcon({ size = 8 }: IconProps) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width={size} height={size} className="icon" fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d="M1.5 1.5h9v9h-9zM3.5 3.5l5 5M8.5 3.5l-5 5" />
    </svg>
  );
}

/**
 * The deck chip: a round token. Decorative (aria-hidden) on a compact face, where the side's label names the
 * deck; with a `label` it is an image named by the deck, as on the detail sheet.
 */
export function EmblemChip({ emblem, size = 16, label, id }: { emblem: DeckEmblem; size?: number; label?: string; id?: string }) {
  return (
    <svg
      {...(label === undefined ? { "aria-hidden": "true" as const } : { role: "img", "aria-label": label })}
      id={id}
      focusable="false"
      viewBox="0 0 16 16"
      width={size}
      height={size}
      className={`card-emblem card-emblem-${emblem}`}
    >
      <circle cx="8" cy="8" r="7.25" className="card-emblem-disc" />
      {emblem === "bolt" ? <path d="M9.2 2.5 4.5 9h3l-.8 4.5L11.5 7h-3z" className="card-emblem-glyph" /> : <path d="M3 12.5V6h2v1.5h1.5V6h3v1.5H11V6h2v6.5z" className="card-emblem-glyph" />}
    </svg>
  );
}

/** The landscape of a card without an image (no image to load, no hole). */
function Landscape() {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 40 30" width="100%" height="100%" preserveAspectRatio="xMidYMid slice">
      <path d="M0 30V20l9-9 7 7 8-12 16 14v10z" fill="currentColor" opacity="0.35" />
      <circle cx="30" cy="8" r="3" fill="currentColor" opacity="0.35" />
    </svg>
  );
}

/**
 * The art of a card: its image when the lookup knows the name, else the landscape. Decorative (`alt=""`, never
 * a `title`), no `onError`: an image that fails leaves the striped background of the slot, not a hole.
 */
function Art({ name }: { name: string | null | undefined }) {
  const lookup = useArtLookup();
  const info = name === null || name === undefined ? undefined : lookup(name);
  return info === undefined ? <Landscape /> : <img src={info.url} alt="" draggable={false} decoding="async" />;
}

/** The compact face's art slot, with the deck chip on it (the image comes after it in the DOM: no z-index). */
export function ArtPlaceholder({ emblem, name }: { emblem?: DeckEmblem; name?: string | null }) {
  return (
    <span className="card-art">
      {emblem !== undefined && <EmblemChip emblem={emblem} />}
      <Art name={name} />
    </span>
  );
}

/** The detail sheet's art slot: a fixed-height box (CSS) holding the card's image, or the landscape. */
export function DetailArt({ name }: { name?: string | null }) {
  return (
    <div className="card-detail-art">
      <Art name={name} />
    </div>
  );
}
