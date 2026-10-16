import type { Seat } from "../engine/types";
import type { Snapshot } from "../game/controller";
import type { ScreenModel } from "../game/screen-model";
import type { ConsequenceLabels } from "./consequence";
import { endReason, fr, type Outcome } from "./fr";

/** Who is who for wording a result: the human's seat (null when one person plays both camps) and the heroes' names. */
export function outcomeOf(snapshot: Snapshot, model: ScreenModel): Outcome {
  const heroes = new Map<Seat, string>([
    [model.top.seat, model.top.hero.name],
    [model.bottom.seat, model.bottom.hero.name],
  ]);
  return { you: snapshot.humanSeat, heroName: (seat) => heroes.get(seat) ?? "" };
}

export function consequenceLabels(outcome: Outcome): ConsequenceLabels {
  return {
    winner: (seat) => fr.winnerPhrase(seat, outcome.you),
    reason: (result) => endReason(result, outcome),
  };
}
