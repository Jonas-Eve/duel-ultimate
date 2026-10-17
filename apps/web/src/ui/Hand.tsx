import type { ActionPreview, CardView } from "../engine/types";
import { isPlay } from "../engine/types";
import type { DeckEmblem } from "../game/controller";
import { CardTile, type TimeLeft } from "./CardTile";
import type { ConsequenceLabels } from "./consequence";
import { fr } from "./fr";

interface HandProps {
  cards: CardView[];
  legal: ActionPreview[];
  /** False during the computer's turn and after the end: every card is inactive. */
  enabled: boolean;
  labels: ConsequenceLabels;
  onPlay(preview: ActionPreview): void;
  /** The accessible name of the row ("Ta main", or the camp's hand in two camps). */
  label: string;
  /** The other camp's hand in two camps: named in full, never played; a touch only opens its detail (`onInspect`). */
  readOnly?: boolean;
  /** Read-only hands: a touch on a card asks for its detail. */
  onInspect?(card: CardView): void;
  /** The deck symbol of the side that owns this hand. */
  emblem?: DeckEmblem;
}

/** "Reste N" under the card when the play keeps the main: N is the preview's `budget_after`, never computed here. */
function keepsTime(preview: ActionPreview | undefined, labels: ConsequenceLabels): TimeLeft | undefined {
  if (preview === undefined || preview.hands_over || preview.ends_game !== null || preview.budget_after === null) return undefined;
  const spoken = labels.timeLeft(preview.budget_after);
  return { legend: fr.keepsLegend(preview.budget_after), spoken: spoken.charAt(0).toLowerCase() + spoken.slice(1) };
}

/** A hand. Each card's cost and consequence come from its engine preview (found by card id). */
export function Hand({ cards, legal, enabled, labels, onPlay, label, readOnly = false, onInspect, emblem }: HandProps) {
  return (
    <div role="group" aria-label={label} tabIndex={0} className="row hand">
      {cards.length === 0 && <p className="empty">{fr.emptyHand}</p>}
      {cards.map((card) => {
        if (readOnly) {
          return <CardTile key={card.id} card={card} cost={card.cost} emblem={emblem} onPress={onInspect === undefined ? undefined : () => onInspect(card)} opensDialog={onInspect !== undefined} />;
        }
        const preview = legal.find((p) => isPlay(p.action) && p.action.play.card === card.id);
        return (
          <CardTile
            key={card.id}
            card={card}
            emblem={emblem}
            cost={preview?.cost ?? card.cost}
            timeLeft={keepsTime(preview, labels)}
            handsOver={preview?.hands_over ?? false}
            disabled={!enabled || preview === undefined}
            onPress={() => preview && onPlay(preview)}
          />
        );
      })}
    </div>
  );
}
