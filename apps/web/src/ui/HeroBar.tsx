import { useState } from "react";
import type { SideModel } from "../game/screen-model";
import { DiscardDialog } from "./DiscardDialog";
import { fr } from "./fr";

interface HeroBarProps {
  side: SideModel;
  label: string;
  /** While an attacker is selected: whether the hero is a possible target (read from the engine previews) and its press. */
  target?: { possible: boolean; onPress(): void };
}

export function HeroBar({ side, label, target }: HeroBarProps) {
  const [discardOpen, setDiscardOpen] = useState(false);
  return (
    <div role="group" aria-label={label} className={side.active ? "hero-bar hero-bar-active" : "hero-bar"}>
      <span className="hero-name">{side.hero.name}</span>
      <span>{fr.health(side.hero.health)}</span>
      <span>{fr.handCount(side.handCount)}</span>
      <span>{fr.deckCount(side.deckCount)}</span>
      {target !== undefined && (
        <button
          type="button"
          className={target.possible ? "btn btn-target hero-target" : "btn hero-target"}
          aria-disabled={target.possible ? undefined : true}
          aria-label={
            target.possible
              ? `Attaquer ${fr.heroAsTarget(side.hero.name)}, ${fr.targetPossible.toLowerCase()}`
              : `Le héros ${side.hero.name}, ${fr.targetUnavailable.toLowerCase()}`
          }
          onClick={target.possible ? target.onPress : undefined}
        >
          {target.possible ? fr.targetPossible : fr.targetUnavailable}
        </button>
      )}
      <button type="button" className="btn" onClick={() => setDiscardOpen(true)}>
        {fr.discardButton(side.discard.length)}
      </button>
      {discardOpen && <DiscardDialog cards={side.discard} who={label} onClose={() => setDiscardOpen(false)} />}
    </div>
  );
}
