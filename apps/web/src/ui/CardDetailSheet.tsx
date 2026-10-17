import { useId, type ReactNode } from "react";
import type { CardView } from "../engine/types";
import type { DeckEmblem } from "../game/controller";
import { CardDetail, type AttackLineProps } from "./CardDetail";
import { Dialog } from "./Dialog";
import { fr } from "./fr";

interface CardDetailSheetProps {
  card: CardView;
  emblem?: DeckEmblem;
  /** The deck's name, spoken by the deck symbol. */
  deckName: string;
  onClose(): void;
  /** Where the buttons go (play, attack): above "Fermer". Read-only when absent. */
  actions?: ReactNode;
  /** Own board creature: its attack line is a button (see `CardDetail`). */
  attack?: AttackLineProps;
}

/**
 * The card detail as a modal sheet (the generic `Dialog`): named by the card's name, focus on the sheet
 * itself, described by a one-sentence summary of the card, closed by Escape or "Fermer".
 */
export function CardDetailSheet({ card, emblem, deckName, onClose, actions, attack }: CardDetailSheetProps) {
  const summaryId = useId();
  const deckId = useId();
  return (
    <Dialog title={card.name ?? fr.unknownCard} onClose={onClose} hideTitle initialFocus="dialog" describedBy={emblem === undefined ? summaryId : `${summaryId} ${deckId}`}>
      <CardDetail card={card} emblem={emblem} deckName={deckName} summaryId={summaryId} deckId={deckId} attack={attack} />
      {actions}
      <button type="button" className="btn card-detail-close" onClick={onClose}>
        {fr.close}
      </button>
    </Dialog>
  );
}
