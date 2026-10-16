import type { ActionPreview, CardView } from "../engine/types";
import { isPlay } from "../engine/types";
import { CardTile } from "./CardTile";
import { describeConsequence, type ConsequenceLabels } from "./consequence";
import { fr } from "./fr";

interface HandProps {
  cards: CardView[];
  legal: ActionPreview[];
  /** False during the computer's turn and after the end: every card is inactive. */
  enabled: boolean;
  labels: ConsequenceLabels;
  onPlay(preview: ActionPreview): void;
}

/** The human's hand. Each card's cost and consequence come from its engine preview (found by card id). */
export function Hand({ cards, legal, enabled, labels, onPlay }: HandProps) {
  return (
    <div role="group" aria-label={fr.yourHand} tabIndex={0} className="row hand">
      {cards.length === 0 && <p className="empty">{fr.emptyHand}</p>}
      {cards.map((card) => {
        const preview = legal.find((p) => isPlay(p.action) && p.action.play.card === card.id);
        return (
          <CardTile
            key={card.id}
            card={card}
            cost={preview?.cost ?? card.cost}
            notes={preview ? describeConsequence(preview, labels).slice(preview.cost === null ? 0 : 1) : []}
            handsOver={preview?.hands_over ?? false}
            disabled={!enabled || preview === undefined}
            onPress={() => preview && onPlay(preview)}
          />
        );
      })}
    </div>
  );
}
