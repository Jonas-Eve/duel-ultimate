import type { CardView } from "../engine/types";
import type { DeckEmblem } from "../game/controller";
import { fr } from "./fr";

/** The attack flow's state of a tile, always written as text as well as drawn as a border. */
export type CardMark = "selected" | "target" | "unavailable";

/** A hand card's time legend, read from the engine preview (never computed): visible short form and spoken form. */
export interface TimeLeft {
  legend: string;
  spoken: string;
}

export interface CardTileProps {
  card: CardView;
  /** Hand cards only: the cost comes from the engine preview, not from the card. Never given on a board. */
  cost?: number;
  /** Hand cards: the time left after playing (the preview's `budget_after`), when the card keeps the main. */
  timeLeft?: TimeLeft;
  handsOver?: boolean;
  /** A real `<button>` only when given (the attack flow of #66 adds the selection); else a plain element. */
  onPress?: () => void;
  disabled?: boolean;
  /** `aria-disabled` instead of `disabled`: still focusable and announced, a press does nothing. */
  inactive?: boolean;
  /** `aria-pressed` (a toggle) when defined. */
  pressed?: boolean;
  mark?: CardMark;
  /** A card in a discard: a "destroyed" icon and legend instead of the life (never "PV 0"). Not interactive. */
  destroyed?: boolean;
  /** The deck symbol drawn on the art (the card's own seat); absent = no chip. */
  emblem?: DeckEmblem;
}

/** The complete accessible name: "Pillard, coût 2, attaque 3, riposte 0, vie 2, provocation, passe la main, sélectionnée". */
export function cardLabel(card: CardView, opts: { cost?: number; handsOver?: boolean; timeLeft?: TimeLeft; mark?: CardMark; destroyed?: boolean } = {}): string {
  return (
    `${card.name ?? fr.unknownCard}` +
    (opts.cost === undefined ? "" : `, ${fr.costWord} ${opts.cost}`) +
    `, ${fr.attackWord} ${card.force}, ${fr.riposteWord} ${card.riposte}` +
    (opts.destroyed ? `, ${fr.destroyed.toLowerCase()}` : `, ${fr.lifeWord} ${card.health}`) +
    (card.provocation ? `, ${fr.provocation.toLowerCase()}` : "") +
    (opts.handsOver ? `, ${fr.passesLegend.toLowerCase()}` : opts.timeLeft === undefined ? "" : `, ${opts.timeLeft.spoken}`) +
    (opts.mark === undefined ? "" : `, ${markText[opts.mark].toLowerCase()}`)
  );
}

const markText: Record<CardMark, string> = { selected: fr.selected, target: fr.targetPossible, unavailable: fr.targetUnavailable };

// All icons are decorative (aria-hidden): every value is also a number, every state also a text.
function StarIcon() {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width="8" height="8" className="icon" fill="currentColor">
      <path d="M6 0l1.6 4.2L12 4.6 8.6 7.4 9.7 12 6 9.4 2.3 12l1.1-4.6L0 4.6l4.4-.4z" />
    </svg>
  );
}

function BackArrowIcon() {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width="8" height="8" className="icon" fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d="M10 9V6H3M5 3 2.5 6 5 9" />
    </svg>
  );
}

function HeartIcon() {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width="8" height="8" className="icon" fill="currentColor">
      <path d="M6 11C1 7.5 0 5 0 3.4A3 3 0 0 1 6 2.2 3 3 0 0 1 12 3.4C12 5 11 7.5 6 11z" />
    </svg>
  );
}

function HourglassIcon() {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width="8" height="8" className="icon" fill="none" stroke="currentColor" strokeWidth="1.5">
      <path d="M2.5 1h7M2.5 11h7M3 1c0 3 3 3 3 5s-3 2-3 5M9 1c0 3-3 3-3 5s3 2 3 5" />
    </svg>
  );
}

function ShieldIcon() {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width="8" height="8" className="icon" fill="currentColor">
      <path d="M6 0 11 2v4c0 3-2.5 5-5 6-2.5-1-5-3-5-6V2z" />
    </svg>
  );
}

/** Distinct from the heart (a broken cross on a square): the life slot of a destroyed card. */
function DestroyedIcon() {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 12 12" width="8" height="8" className="icon" fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d="M1.5 1.5h9v9h-9zM3.5 3.5l5 5M8.5 3.5l-5 5" />
    </svg>
  );
}

/** The deck chip, top right of the art: a decorative round token (the deck is named by the side's label, never by this). */
function EmblemChip({ emblem }: { emblem: DeckEmblem }) {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 16 16" width="16" height="16" className={`card-emblem card-emblem-${emblem}`}>
      <circle cx="8" cy="8" r="7.25" className="card-emblem-disc" />
      {emblem === "bolt" ? <path d="M9.2 2.5 4.5 9h3l-.8 4.5L11.5 7h-3z" className="card-emblem-glyph" /> : <path d="M3 12.5V6h2v1.5h1.5V6h3v1.5H11V6h2v6.5z" className="card-emblem-glyph" />}
    </svg>
  );
}

/** The art slot: a placeholder drawn inline (no image to load, no hole), until real illustrations come. */
function ArtPlaceholder({ emblem }: { emblem?: DeckEmblem }) {
  return (
    <span className="card-art">
      {emblem !== undefined && <EmblemChip emblem={emblem} />}
      <svg aria-hidden="true" focusable="false" viewBox="0 0 40 30" width="100%" height="100%" preserveAspectRatio="xMidYMid slice">
        <path d="M0 30V20l9-9 7 7 8-12 16 14v10z" fill="currentColor" opacity="0.35" />
        <circle cx="30" cy="8" r="3" fill="currentColor" opacity="0.35" />
      </svg>
    </span>
  );
}

/**
 * The fixed-size face of a card (`--card-w` x 112 px whatever the content): name line (with the cost pill
 * in hand only), art slot, then the three values attack / riposte / life, riposte shown even at 0.
 * Purely visual: no state, no handler, no accessible name of its own (the tile around it carries it).
 */
export function CardFace({ card, cost, destroyed = false, emblem }: { card: CardView; cost?: number; destroyed?: boolean; emblem?: DeckEmblem }) {
  return (
    <span className="card-face">
      <span className="card-head">
        {cost !== undefined && (
          <span className="card-cost">
            <HourglassIcon />
            {cost}
          </span>
        )}
        <span className="card-name">{card.name ?? fr.unknownCard}</span>
      </span>
      <ArtPlaceholder emblem={emblem} />
      <span className="card-values">
        <span className="card-value">
          <StarIcon />
          {card.force}
        </span>
        <span className="card-value">
          <BackArrowIcon />
          {card.riposte}
        </span>
        <span className="card-value">
          {destroyed ? <DestroyedIcon /> : <HeartIcon />}
          {destroyed ? null : card.health}
        </span>
      </span>
    </span>
  );
}

export function CardTile({ card, cost, timeLeft, handsOver = false, destroyed = false, emblem, onPress, disabled = false, inactive = false, pressed, mark }: CardTileProps) {
  // The visible mark is part of the accessible name too (label in name).
  const label = cardLabel(card, { cost, handsOver, timeLeft, mark, destroyed });
  const hasTags = card.provocation || cost !== undefined || mark !== undefined || destroyed;
  const body = (
    <>
      <CardFace card={card} cost={cost} destroyed={destroyed} emblem={emblem} />
      {hasTags && (
        <span className="card-tags">
          {card.provocation && (
            <span className="card-keyword">
              <ShieldIcon />
              {fr.provocation}
            </span>
          )}
          {cost !== undefined && (handsOver ? <span className="card-legend">{fr.passesLegend}</span> : timeLeft !== undefined && <span className="card-legend">{timeLeft.legend}</span>)}
          {destroyed && <span className="card-legend">{fr.destroyed}</span>}
          {mark !== undefined && <span className="card-mark">{markText[mark]}</span>}
        </span>
      )}
    </>
  );
  if (onPress === undefined) {
    return (
      <div role="group" aria-label={label} className="card card-compact">
        {body}
      </div>
    );
  }
  const classes = ["card", "card-compact", "card-button", mark === undefined ? "" : `card-${mark}`].filter(Boolean).join(" ");
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={pressed}
      aria-disabled={inactive ? true : undefined}
      className={classes}
      disabled={disabled}
      onClick={inactive ? undefined : onPress}
    >
      {body}
    </button>
  );
}
