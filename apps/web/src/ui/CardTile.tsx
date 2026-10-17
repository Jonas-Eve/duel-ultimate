import type { CardView } from "../engine/types";
import type { DeckEmblem } from "../game/controller";
import { ArtPlaceholder, BackArrowIcon, DestroyedIcon, HeartIcon, HourglassIcon, ShieldIcon, StarIcon } from "./card-icons";
import { fr } from "./fr";

/** The attack flow's state of a tile, always written as text as well as drawn as a border. */
export type CardMark = "selected" | "target" | "unavailable";

export interface CardTileProps {
  card: CardView;
  /** Hand cards only: the cost comes from the engine preview, not from the card. Never given on a board. */
  cost?: number;
  /** Hand cards: the visible legend "Reste N" (N is the preview's `budget_after`), when the card keeps the main. It is also in the name, verbatim. */
  timeLeft?: string;
  handsOver?: boolean;
  /** A real `<button>` only when given (the attack flow of #66 adds the selection); else a plain element. */
  onPress?: () => void;
  disabled?: boolean;
  /** `aria-disabled` instead of `disabled`: still focusable and announced, a press does nothing (target choice: unavailable cards). */
  inactive?: boolean;
  /** The press opens a dialog (the card detail): `aria-haspopup="dialog"`. */
  opensDialog?: boolean;
  mark?: CardMark;
  /** A card in a discard: a "destroyed" icon and legend instead of the life (never "PV 0"). Not interactive. */
  destroyed?: boolean;
  /** The deck symbol drawn on the art (the card's own seat); absent = no chip. */
  emblem?: DeckEmblem;
}

/** The complete accessible name: "Pillard, coût 2, attaque 3, riposte 0, vie 2, provocation, Reste 3 | passe la main, sélectionnée". */
export function cardLabel(card: CardView, opts: { cost?: number; handsOver?: boolean; timeLeft?: string; mark?: CardMark; destroyed?: boolean } = {}): string {
  return (
    `${card.name ?? fr.unknownCard}` +
    (opts.cost === undefined ? "" : `, ${fr.costWord} ${opts.cost}`) +
    `, ${fr.attackWord} ${card.force}, ${fr.riposteWord} ${card.riposte}` +
    (opts.destroyed ? `, ${fr.destroyed.toLowerCase()}` : `, ${fr.lifeWord} ${card.health}`) +
    (card.provocation ? `, ${fr.provocation.toLowerCase()}` : "") +
    (opts.handsOver ? `, ${fr.passesLegend.toLowerCase()}` : opts.timeLeft === undefined ? "" : `, ${opts.timeLeft}`) +
    (opts.mark === undefined ? "" : `, ${markText[opts.mark].toLowerCase()}`)
  );
}

const markText: Record<CardMark, string> = { selected: fr.selected, target: fr.targetPossible, unavailable: fr.targetUnavailable };

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
      <ArtPlaceholder emblem={emblem} name={card.name} />
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

export function CardTile({ card, cost, timeLeft, handsOver = false, destroyed = false, emblem, onPress, disabled = false, inactive = false, mark, opensDialog = false }: CardTileProps) {
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
          {cost !== undefined && (handsOver ? <span className="card-legend">{fr.passesLegend}</span> : timeLeft !== undefined && <span className="card-legend">{timeLeft}</span>)}
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
      aria-haspopup={opensDialog ? "dialog" : undefined}
      aria-disabled={inactive ? true : undefined}
      className={classes}
      disabled={disabled}
      onClick={inactive ? undefined : onPress}
    >
      {body}
    </button>
  );
}
