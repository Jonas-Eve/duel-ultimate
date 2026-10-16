import type { CardView } from "../engine/types";
import { fr } from "./fr";

/** The attack flow's state of a tile, always written as text as well as drawn as a border. */
export type CardMark = "selected" | "target" | "unavailable";

export interface CardTileProps {
  card: CardView;
  /** Hand cards: the cost comes from the engine preview, not from the card (shown as "Coût N"). */
  cost?: number;
  /** Lines of `describeConsequence` after the cost line (time left, hand-over...). */
  notes?: string[];
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

/** The complete accessible name of a card: "Pillard, coût 2, force 3, coût d'attaque 1, PV 2, passe la main, provocation, riposte 1". */
export function cardLabel(card: CardView, cost: number, handsOver: boolean): string {
  return (
    `${card.name ?? fr.unknownCard}, coût ${cost}, force ${card.force}, coût d'attaque ${card.attack_cost}, PV ${card.health}` +
    (handsOver ? ", passe la main" : "") +
    (card.provocation ? ", provocation" : "") +
    (card.riposte > 0 ? `, riposte ${card.riposte}` : "")
  );
}

const markText: Record<CardMark, string> = { selected: fr.selected, target: fr.targetPossible, unavailable: fr.targetUnavailable };

function ShieldIcon() {
  return (
    <svg aria-hidden="true" focusable="false" viewBox="0 0 16 16" width="14" height="14" className="icon">
      <path d="M8 1 2 3v4c0 3.5 2.5 6.3 6 8 3.5-1.7 6-4.5 6-8V3L8 1z" fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

export function CardTile({ card, cost, notes = [], handsOver = false, onPress, disabled = false, inactive = false, pressed, mark }: CardTileProps) {
  // The visible mark is part of the accessible name too (label in name).
  const label = cardLabel(card, cost ?? card.cost, handsOver) + (mark === undefined ? "" : `, ${markText[mark].toLowerCase()}`);
  const body = (
    <>
      <span className="card-name">{card.name ?? fr.unknownCard}</span>
      <span className="card-stats">
        {cost !== undefined && <span>{fr.cost(cost)}</span>}
        <span>{fr.force(card.force)}</span>
        <span>{fr.attackCost(card.attack_cost)}</span>
        <span>{fr.health(card.health)}</span>
      </span>
      {(card.provocation || card.riposte > 0) && (
        <span className="card-badges">
          {card.provocation && (
            <span className="badge">
              <ShieldIcon />
              {fr.provocation}
            </span>
          )}
          {card.riposte > 0 && <span className="badge">{fr.riposte(card.riposte)}</span>}
        </span>
      )}
      {mark !== undefined && <span className="card-mark">{markText[mark]}</span>}
      {notes.map((n) => (
        <span key={n} className="card-note">
          {n}
        </span>
      ))}
    </>
  );
  if (onPress === undefined) {
    return (
      <div role="group" aria-label={label} className="card">
        {body}
      </div>
    );
  }
  const classes = ["card", "card-button", mark === undefined ? "" : `card-${mark}`].filter(Boolean).join(" ");
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
