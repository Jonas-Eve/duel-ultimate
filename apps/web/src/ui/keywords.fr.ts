// The single source of the French explanation of each passive effect (keyword) a card can carry.
// A card field that is a keyword must have its entry here (`tests/card-detail-keywords.test.ts`), and so
// must each keyword an effect can grant (`KEYWORD_NAMES` of `crates/cards/src/effects.rs`).
export type KeywordKey = "provocation" | "immobile" | "ranged";

export const KEYWORDS_FR: Record<KeywordKey, { label: string; explanation: string }> = {
  provocation: {
    label: "Provocation",
    explanation: "tant qu'elle est en jeu, l'adversaire ne peut attaquer qu'une créature qui provoque ; son héros n'est pas ciblable.",
  },
  immobile: {
    label: "Immobile",
    explanation: "ne peut pas attaquer, même si elle gagne de la force.",
  },
  // Not a field of a card: a keyword that an effect grants ("Tir accordé sur ...").
  ranged: {
    label: "Tir",
    explanation: "ses attaques ignorent la provocation.",
  },
};
