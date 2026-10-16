import type { Seat } from "../engine/types";
import type { Snapshot } from "../game/controller";
import type { ScreenModel } from "../game/screen-model";
import type { ConsequenceLabels } from "./consequence";
import { endReason, fr, type Outcome } from "./fr";
import { logContext } from "./log-format";

/** Who is who for wording a result: the human's seat (null when one person plays both camps) and the heroes' names. */
export function outcomeOf(snapshot: Snapshot, model: ScreenModel): Outcome {
  const heroes = new Map<Seat, string>([
    [model.top.seat, model.top.hero.name],
    [model.bottom.seat, model.bottom.hero.name],
  ]);
  const labels = logContext(snapshot).labels;
  return { you: snapshot.humanSeat, heroName: (seat) => heroes.get(seat) ?? "", camp: (seat) => labels[seat] };
}

export function consequenceLabels(outcome: Outcome): ConsequenceLabels {
  return {
    winner: (seat) => fr.winnerPhrase(seat, outcome),
    reason: (result) => endReason(result, outcome),
    timeLeft: (n) => (outcome.you === null ? `Il restera ${n} de temps` : `Il te restera ${n} de temps`),
    handPasses: (n) => `Passe la main : ${outcome.you === null ? "l'autre camp" : "l'adversaire"} aura ${n} de temps`,
  };
}
