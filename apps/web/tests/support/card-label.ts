import type { CardView } from "../../src/engine/types";

/**
 * The expected accessible name of a card tile, spelled out literally (never imported from the
 * component, so a wording change breaks the tests on purpose):
 * The time legend is the visible text verbatim ("Reste 3"), never the spoken form (that one describes the Play button).
 * Unit: "Nom, coût N (main seulement), attaque N, riposte N, vie N[, provocation][, immobile][, passe la main | Reste N][, marque]"
 * (an immobile unit has no "attaque N": its attack value is not shown).
 * Spell: "Nom, coût N (main seulement), sort[, passe la main | Reste N][, marque]".
 */
export function expectedCardLabel(
  card: CardView,
  opts: { inHand?: boolean; cost?: number; handsOver?: boolean; timeLeft?: string; mark?: string } = {},
): string {
  const head = `${card.name}` + (opts.inHand ? `, coût ${opts.cost ?? card.cost}` : "");
  const tail = (opts.handsOver ? ", passe la main" : opts.timeLeft !== undefined ? `, ${opts.timeLeft}` : "") + (opts.mark !== undefined ? `, ${opts.mark}` : "");
  if (card.kind === "spell") return `${head}, sort${tail}`;
  return (
    head +
    (card.immobile ? "" : `, attaque ${card.force}`) +
    `, riposte ${card.riposte}, vie ${card.health}` +
    (card.provocation ? ", provocation" : "") +
    (card.immobile ? ", immobile" : "") +
    tail
  );
}
