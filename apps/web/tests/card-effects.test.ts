import { describe, expect, test } from "vitest";
import type { CardView } from "../src/engine/types";
import { cardEffects, effectLineLabel, effectLineText } from "../src/ui/card-effects";
import { KEYWORDS_FR } from "../src/ui/keywords.fr";

const card = (over: Partial<CardView> = {}): CardView => ({
  id: 7,
  name: "Garde du rempart",
  cost: 3,
  force: 1,
  attack_cost: 2,
  health: 6,
  provocation: false,
  riposte: 0,
  ...over,
});

describe("cardEffects: the card's fields arranged for the detail sheet, nothing computed", () => {
  test("the attack is one active effect line: its cost is attack_cost, its damage is force, as given", () => {
    expect(cardEffects(card()).active).toEqual([{ name: "Attaque", cost: 2, damage: 1 }]);
    expect(cardEffects(card({ force: 5, attack_cost: 4 })).active).toEqual([{ name: "Attaque", cost: 4, damage: 5 }]);
  });

  test("the values are the card's own, riposte 0 and attack 0 included", () => {
    expect(cardEffects(card({ riposte: 0, force: 0, health: 1, cost: 9 })).values).toEqual({ cost: 9, attack: 0, riposte: 0, health: 1 });
    expect(cardEffects(card({ riposte: 2 })).values.riposte).toBe(2);
  });

  test("a passive effect only when the card has the keyword, with the French text of the single source", () => {
    expect(cardEffects(card()).passive).toEqual([]);
    const lines = cardEffects(card({ provocation: true })).passive;
    expect(lines).toEqual([{ key: "provocation", label: KEYWORDS_FR.provocation.label, explanation: KEYWORDS_FR.provocation.explanation }]);
    expect(lines[0].label).toBe("Provocation");
  });
});

describe("the effect line text: one function builds the visible text and the spoken addition", () => {
  test("visible 'Nom (N) : inflige D dégât(s)', spoken ', pour N temps'; the label is both", () => {
    const one = { name: "Attaque", cost: 2, damage: 1 };
    expect(effectLineText(one)).toEqual({ visible: "Attaque (2) : inflige 1 dégât", hidden: ", pour 2 temps" });
    expect(effectLineLabel(one)).toBe("Attaque (2) : inflige 1 dégât, pour 2 temps");
    expect(effectLineLabel({ name: "Attaque", cost: 3, damage: 4 })).toBe("Attaque (3) : inflige 4 dégâts, pour 3 temps");
    expect(effectLineLabel({ name: "Attaque", cost: 1, damage: 0 })).toBe("Attaque (1) : inflige 0 dégât, pour 1 temps");
  });
});
