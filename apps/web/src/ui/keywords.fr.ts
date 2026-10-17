// The single source of the French explanation of each passive effect (keyword) a card can carry.
// A card field that is a keyword must have its entry here (`tests/card-detail-keywords.test.ts`).
export type KeywordKey = "provocation";

export const KEYWORDS_FR: Record<KeywordKey, { label: string; explanation: string }> = {
  provocation: {
    label: "Provocation",
    explanation: "tant qu'elle est en jeu, l'adversaire ne peut attaquer qu'une créature qui provoque ; son héros n'est pas ciblable.",
  },
};
