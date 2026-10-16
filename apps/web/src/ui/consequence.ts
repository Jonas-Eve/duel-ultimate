// What an action will do, in words, read ONLY from the engine's preview: the client never
// computes a cost, a hand-over or an end of game (the engine's time rule is the single one).
import type { ActionPreview, GameResult, Seat } from "../engine/types";
import { isAttack } from "../engine/types";

export interface ConsequenceLabels {
  /** "Tu gagnes" / "L'ordinateur gagne". */
  winner(seat: Seat): string;
  /** The end-of-game reason, worded as on the end screen. */
  reason(result: GameResult): string;
}

/**
 * Lines to show before an action is played. Real combinations (fact V2): a simple action, a hand-over,
 * a lethal attack (`ends_game`, no hand-over), a hand-over that ends the game by exhaustion
 * (`hands_over` AND `ends_game`) and a concession (no cost). The first line is the cost, when there is one.
 */
export function describeConsequence(preview: ActionPreview, labels: ConsequenceLabels): string[] {
  const lines: string[] = [];
  if (preview.cost !== null) lines.push(`${isAttack(preview.action) ? "Coût d'attaque" : "Coût"} : ${preview.cost}`);
  const end = preview.ends_game;
  if (end !== null) {
    const head = preview.hands_over ? "Passe la main et met fin à la partie" : "Met fin à la partie";
    lines.push(`${head} : ${labels.winner(end.winner)}. ${labels.reason(end)}`);
  } else if (preview.hands_over) {
    lines.push(
      preview.budget_after === null
        ? "Passe la main"
        : `Passe la main : l'adversaire aura ${preview.budget_after} de temps`,
    );
  } else if (preview.budget_after !== null) {
    lines.push(`Il te restera ${preview.budget_after} de temps`);
  }
  return lines;
}
