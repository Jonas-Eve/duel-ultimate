import { useId } from "react";
import type { ActionPreview } from "../engine/types";
import { describeConsequence, type ConsequenceLabels } from "./consequence";
import { fr } from "./fr";

interface PlayActionsProps {
  /** The engine's preview of playing this card, or undefined when none is offered (the computer's turn). */
  preview: ActionPreview | undefined;
  labels: ConsequenceLabels;
  /** Why "Jouer" is inactive, shown as text when there is no preview. */
  reason: string;
  onPlay(preview: ActionPreview): void;
}

/**
 * The play button of a hand card's detail, with its consequence in words (the cost, the hand-over, the end of
 * the game: all read from the preview, nothing computed here). Without a preview the button is `aria-disabled`
 * rather than `disabled` (still focusable and announced) and the reason is visible text. The consequence or the
 * reason is the button's description, so the spoken "Il te restera N de temps" is heard on it.
 */
export function PlayActions({ preview, labels, reason, onPlay }: PlayActionsProps) {
  const id = useId();
  return (
    <div className="play-actions">
      {preview === undefined ? (
        <p id={id} className="play-reason">
          {reason}
        </p>
      ) : (
        <ul id={id} className="play-consequence">
          {describeConsequence(preview, labels).map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}
      <button
        type="button"
        className="btn btn-primary btn-play"
        aria-describedby={id}
        aria-disabled={preview === undefined ? true : undefined}
        onClick={preview === undefined ? undefined : () => onPlay(preview)}
      >
        {preview === undefined || preview.cost === null ? fr.play : fr.playCost(preview.cost)}
      </button>
    </div>
  );
}
