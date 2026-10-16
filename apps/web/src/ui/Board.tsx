import type { CardView } from "../engine/types";
import { CardTile, type CardTileProps } from "./CardTile";
import { fr } from "./fr";

interface BoardProps {
  cards: CardView[];
  label: string;
  /** Per-card interaction (the attack flow): the same tile for both camps, only these props differ. */
  tileProps?: (card: CardView) => Partial<CardTileProps>;
}

/** A row of creatures; it scrolls inside itself, never the page. Reachable by keyboard (`tabIndex`). */
export function Board({ cards, label, tileProps }: BoardProps) {
  return (
    <div role="group" aria-label={label} tabIndex={0} className="row board">
      {cards.length === 0 && <p className="empty">{fr.noCreature}</p>}
      {cards.map((c) => (
        <CardTile key={c.id} card={c} {...tileProps?.(c)} />
      ))}
    </div>
  );
}
