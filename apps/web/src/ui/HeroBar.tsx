import { useState } from "react";
import type { SideModel } from "../game/screen-model";
import { DiscardDialog } from "./DiscardDialog";
import { fr } from "./fr";

export function HeroBar({ side, label }: { side: SideModel; label: string }) {
  const [discardOpen, setDiscardOpen] = useState(false);
  return (
    <div role="group" aria-label={label} className={side.active ? "hero-bar hero-bar-active" : "hero-bar"}>
      <span className="hero-name">{side.hero.name}</span>
      <span>{fr.health(side.hero.health)}</span>
      <span>{fr.handCount(side.handCount)}</span>
      <span>{fr.deckCount(side.deckCount)}</span>
      <button type="button" className="btn" onClick={() => setDiscardOpen(true)}>
        {fr.discardButton(side.discard.length)}
      </button>
      {discardOpen && <DiscardDialog cards={side.discard} who={label} onClose={() => setDiscardOpen(false)} />}
    </div>
  );
}
