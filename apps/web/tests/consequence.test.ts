import { describe, expect, test } from "vitest";
import type { ActionPreview } from "../src/engine/types";
import { describeConsequence, type ConsequenceLabels } from "../src/ui/consequence";

const labels: ConsequenceLabels = {
  winner: (seat) => (seat === "One" ? "Tu gagnes" : "L'ordinateur gagne"),
  reason: (r) => `motif:${r.reason}`,
};
const preview = (over: Partial<ActionPreview>): ActionPreview => ({
  action: { play: { card: 1 } },
  cost: 2,
  hands_over: false,
  budget_after: 1,
  ends_game: null,
  ...over,
});

describe("describeConsequence, from the engine preview only", () => {
  test("simple action: the cost and the time left", () => {
    expect(describeConsequence(preview({ cost: 2, budget_after: 1 }), labels)).toEqual(["Coût : 2", "Il te restera 1 de temps"]);
  });

  test("the numbers come from the fields: other digits, other text", () => {
    expect(describeConsequence(preview({ cost: 4, budget_after: 6 }), labels)).toEqual(["Coût : 4", "Il te restera 6 de temps"]);
  });

  test("hand-over: the cost and the opponent's time", () => {
    expect(describeConsequence(preview({ cost: 3, hands_over: true, budget_after: 2 }), labels)).toEqual([
      "Coût : 3",
      "Passe la main : l'adversaire aura 2 de temps",
    ]);
    expect(describeConsequence(preview({ cost: 5, hands_over: true, budget_after: 6 }), labels)).toEqual([
      "Coût : 5",
      "Passe la main : l'adversaire aura 6 de temps",
    ]);
  });

  test("lethal attack (ends_game, no hand-over): ends the game, does not say it hands over", () => {
    const lines = describeConsequence(
      preview({ action: { attack: { attacker: 3, target: "hero" } }, cost: 1, hands_over: false, budget_after: null, ends_game: { winner: "One", reason: "hero_down" } }),
      labels,
    );
    expect(lines).toEqual(["Coût : 1", "Met fin à la partie : Tu gagnes. motif:hero_down"]);
    expect(lines.join(" ")).not.toMatch(/Passe la main/);
  });

  test("hand-over by exhaustion (hands_over AND ends_game): says both", () => {
    const lines = describeConsequence(
      preview({ cost: 3, hands_over: true, budget_after: 2, ends_game: { winner: "Two", reason: "exhaustion" } }),
      labels,
    );
    expect(lines).toEqual(["Coût : 3", "Passe la main et met fin à la partie : L'ordinateur gagne. motif:exhaustion"]);
  });

  test("concede (cost null): no cost line, ends the game", () => {
    const lines = describeConsequence(
      preview({ action: "concede", cost: null, budget_after: null, ends_game: { winner: "Two", reason: "concede" } }),
      labels,
    );
    expect(lines).toEqual(["Met fin à la partie : L'ordinateur gagne. motif:concede"]);
  });

  test("the winner and the reason come from ends_game", () => {
    const one = describeConsequence(preview({ ends_game: { winner: "One", reason: "exhaustion" } }), labels).join(" ");
    const two = describeConsequence(preview({ ends_game: { winner: "Two", reason: "hero_down" } }), labels).join(" ");
    expect(one).toContain("Tu gagnes");
    expect(one).toContain("motif:exhaustion");
    expect(two).toContain("L'ordinateur gagne");
    expect(two).toContain("motif:hero_down");
  });
});
