import type { CardView } from "../engine/types";
import { CardTile } from "./CardTile";
import { Dialog } from "./Dialog";
import { fr } from "./fr";

/** A dead card: the compact face, a "destroyed" icon in place of the life (its starting health is not in the view, never "PV 0"). */
function DiscardCard({ card }: { card: CardView }) {
  return (
    <li className="discard-item">
      <CardTile card={card} destroyed />
    </li>
  );
}

/** The public discard of one side, cards in the order they died (the order of `view.discard`). */
export function DiscardDialog({ cards, who, onClose }: { cards: CardView[]; who: string; onClose(): void }) {
  return (
    <Dialog title={fr.discardTitle(who)} onClose={onClose}>
      {cards.length === 0 ? (
        <p>{fr.discardEmpty}</p>
      ) : (
        <ol className="discard-list">
          {cards.map((card) => (
            <DiscardCard key={card.id} card={card} />
          ))}
        </ol>
      )}
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={onClose}>
          {fr.close}
        </button>
      </div>
    </Dialog>
  );
}
