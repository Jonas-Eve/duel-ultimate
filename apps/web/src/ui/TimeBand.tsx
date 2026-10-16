import type { ScreenModel } from "../game/screen-model";
import { fr } from "./fr";

/**
 * The central band: who holds the main, the time left (read from the view) and that hero's maximum.
 * Two camps: the camp holding the main is named ("Au tour de Camp 2 — Rempart").
 */
export function TimeBand({ model, twoCamps = false }: { model: ScreenModel; twoCamps?: boolean }) {
  const holder = model.top.active ? model.top : model.bottom;
  return (
    <div role="group" aria-label={fr.timeAndToken} className={model.bottomIsActive ? "time-band time-band-you" : "time-band"}>
      <strong className="time-token">{twoCamps ? fr.turnOf(model.bottom.label) : model.bottomIsActive ? fr.yourTurn : fr.opponentTurn}</strong>
      <span>{fr.time(model.budget)}</span>
      <span>{fr.maxTime(holder.hero.max_time)}</span>
    </div>
  );
}
