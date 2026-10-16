import type { ActionPreview, HeroView } from "../engine/types";
import { isActivate } from "../engine/types";
import { describeConsequence, type ConsequenceLabels } from "./consequence";
import { fr } from "./fr";

interface ActionBarProps {
  hero: HeroView;
  legal: ActionPreview[];
  enabled: boolean;
  labels: ConsequenceLabels;
  onActivate(preview: ActionPreview): void;
  onConcede(): void;
}

/** Hero effects (with the cost the engine announces) and "Abandonner". */
export function ActionBar({ hero, legal, enabled, labels, onActivate, onConcede }: ActionBarProps) {
  return (
    <div role="group" aria-label={fr.actions} className="actions">
      {hero.effects.map((effect, index) => {
        const preview = legal.find((p) => isActivate(p.action) && p.action.activate.effect === index);
        const cost = preview?.cost ?? effect.cost;
        const notes = preview ? describeConsequence(preview, labels).slice(preview.cost === null ? 0 : 1) : [];
        return (
          <button
            key={effect.name}
            type="button"
            className="btn btn-effect"
            disabled={!enabled || preview === undefined}
            aria-label={`${effect.name}, coût ${cost}${preview?.hands_over ? ", passe la main" : ""}`}
            onClick={() => preview && onActivate(preview)}
          >
            <span className="effect-name">{effect.name}</span>
            <span>{fr.cost(cost)}</span>
            {notes.map((n) => (
              <span key={n} className="card-note">
                {n}
              </span>
            ))}
          </button>
        );
      })}
      <button type="button" className="btn btn-danger" disabled={!enabled} onClick={onConcede}>
        {fr.concede}
      </button>
    </div>
  );
}
