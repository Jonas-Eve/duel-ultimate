import type { CardView } from "../../src/engine/types";

/**
 * The expected accessible name of a card tile, spelled out literally (never imported from the
 * component, so a wording change breaks the tests on purpose):
 * "Nom, coût N (main seulement), attaque N, riposte N, vie N[, provocation][, passe la main | il te restera N de temps][, marque]".
 */
export function expectedCardLabel(
  card: CardView,
  opts: { inHand?: boolean; cost?: number; handsOver?: boolean; timeLeft?: string; mark?: string } = {},
): string {
  return (
    `${card.name}` +
    (opts.inHand ? `, coût ${opts.cost ?? card.cost}` : "") +
    `, attaque ${card.force}, riposte ${card.riposte}, vie ${card.health}` +
    (card.provocation ? ", provocation" : "") +
    (opts.handsOver ? ", passe la main" : opts.timeLeft !== undefined ? `, ${opts.timeLeft}` : "") +
    (opts.mark !== undefined ? `, ${opts.mark}` : "")
  );
}
