import type { CardView } from "../engine/types";
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
}

/** The complete accessible name: "Pillard, coût 2, attaque 3, riposte 0, vie 2, provocation, passe la main, sélectionnée". */
export function cardLabel(card: CardView, opts: { cost?: number; handsOver?: boolean; timeLeft?: TimeLeft; mark?: CardMark } = {}): string {
  return (
    `${card.name ?? fr.unknownCard}` +
    (opts.cost === undefined ? "" : `, ${fr.costWord} ${opts.cost}`) +
    `, ${fr.attackWord} ${card.force}, ${fr.riposteWord} ${card.riposte}, ${fr.lifeWord} ${card.health}` +
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

/** The art slot: a placeholder drawn inline (no image to load, no hole), until real illustrations come. */
function ArtPlaceholder() {
  return (
    <span className="card-art">
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
export function CardFace({ card, cost }: { card: CardView; cost?: number }) {
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
      <ArtPlaceholder />
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
          <HeartIcon />
          {card.health}
        </span>
      </span>
    </span>
  );
}

export function CardTile({ card, cost, timeLeft, handsOver = false, onPress, disabled = false, inactive = false, pressed, mark }: CardTileProps) {
  // The visible mark is part of the accessible name too (label in name).
  const label = cardLabel(card, { cost, handsOver, timeLeft, mark });
  const hasTags = card.provocation || cost !== undefined || mark !== undefined;
  const body = (
    <>
      <CardFace card={card} cost={cost} />
      {hasTags && (
        <span className="card-tags">
          {card.provocation && (
            <span className="card-keyword">
              <ShieldIcon />
              {fr.provocation}
            </span>
          )}
          {cost !== undefined && (handsOver ? <span className="card-legend">{fr.passesLegend}</span> : timeLeft !== undefined && <span className="card-legend">{timeLeft.legend}</span>)}
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
