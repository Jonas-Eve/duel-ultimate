import type { ActionPreview, CardView } from "../engine/types";
import { isPlay } from "../engine/types";
import type { DeckEmblem } from "../game/controller";
import { CardTile } from "./CardTile";
import { fr } from "./fr";

interface HandProps {
  cards: CardView[];
  /** The engine's previews: the cost and the legend of each own card come from here (found by card id). Empty for the other camp's hand. */
  legal: ActionPreview[];
  /** A touch on a card asks for its detail: nothing is played from the tile, "Jouer" lives in the sheet. */
  onOpen(card: CardView): void;
  /** The accessible name of the row ("Ta main", or the camp's hand in two camps). */
  label: string;
  /** The deck symbol of the side that owns this hand. */
  emblem?: DeckEmblem;
}

/** The preview that plays this card, when the engine offers one (never during the computer's turn nor after the end). */
export function playPreviewOf(legal: ActionPreview[], card: CardView): ActionPreview | undefined {
  return legal.find((p) => isPlay(p.action) && p.action.play.card === card.id);
}

/** "Reste N" under the card when the play keeps the main: N is the preview's `budget_after`, never computed here. */
function keepsTime(preview: ActionPreview | undefined): string | undefined {
  if (preview === undefined || preview.hands_over || preview.ends_game !== null || preview.budget_after === null) return undefined;
  return fr.keepsLegend(preview.budget_after);
}

/** A hand. Every tile opens the card's detail, always (reading is possible whoever has the main). */
export function Hand({ cards, legal, onOpen, label, emblem }: HandProps) {
  return (
    <div role="group" aria-label={label} tabIndex={0} className="row hand">
      {cards.length === 0 && <p className="empty">{fr.emptyHand}</p>}
      {cards.map((card) => {
        const preview = playPreviewOf(legal, card);
        return (
          <CardTile
            key={card.id}
            card={card}
            emblem={emblem}
            cost={preview?.cost ?? card.cost}
            timeLeft={keepsTime(preview)}
            handsOver={preview?.hands_over ?? false}
            onPress={() => onOpen(card)}
            opensDialog
          />
        );
      })}
    </div>
  );
}
