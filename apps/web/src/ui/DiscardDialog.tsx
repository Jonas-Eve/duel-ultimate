import type { CardView } from "../engine/types";
import { Dialog } from "./Dialog";
import { fr } from "./fr";

/** A dead card. Its current health is always 0 in a discard, so it reads "Détruite", never "PV 0" (the starting health is not in the view). */
function DiscardCard({ card }: { card: CardView }) {
  return (
    <li className="card discard-card">
      <span className="card-name">{card.name ?? fr.unknownCard}</span>
      <span className="card-stats">
        <span>{fr.cost(card.cost)}</span>
        <span>{fr.force(card.force)}</span>
        <span>{fr.attackCost(card.attack_cost)}</span>
      </span>
      {(card.provocation || card.riposte > 0) && (
        <span className="card-badges">
          {card.provocation && <span className="badge">{fr.provocation}</span>}
          {card.riposte > 0 && <span className="badge">{fr.riposte(card.riposte)}</span>}
        </span>
      )}
      <span className="card-note">{fr.destroyed}</span>
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
