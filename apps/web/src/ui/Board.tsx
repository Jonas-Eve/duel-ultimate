import type { CardView } from "../engine/types";
import { CardTile } from "./CardTile";
import { fr } from "./fr";

/** A row of creatures; it scrolls inside itself, never the page. Reachable by keyboard (`tabIndex`). */
export function Board({ cards, label }: { cards: CardView[]; label: string }) {
  return (
    <div role="group" aria-label={label} tabIndex={0} className="row board">
      {cards.length === 0 && <p className="empty">{fr.noCreature}</p>}
      {cards.map((c) => (
        <CardTile key={c.id} card={c} />
      ))}
    </div>
  );
}
