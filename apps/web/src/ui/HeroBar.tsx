import type { SideModel } from "../game/screen-model";
import { fr } from "./fr";

export function HeroBar({ side, label }: { side: SideModel; label: string }) {
  return (
    <div role="group" aria-label={label} className={side.active ? "hero-bar hero-bar-active" : "hero-bar"}>
      <span className="hero-name">{side.hero.name}</span>
      <span>{fr.health(side.hero.health)}</span>
      <span>{fr.handCount(side.handCount)}</span>
      <span>{fr.deckCount(side.deckCount)}</span>
    </div>
  );
}
