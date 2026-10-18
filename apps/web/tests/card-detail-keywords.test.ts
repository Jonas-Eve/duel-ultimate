import { beforeAll, describe, expect, test } from "vitest";
import type { CardView } from "../src/engine/types";
import { isPlay } from "../src/engine/types";
import { KEYWORDS_FR } from "../src/ui/keywords.fr";
import { initTestEngine, newSession, rulesFactory } from "./support/engine";

// The detail sheet explains every field of a card. This test reads the keys of a REAL `CardView`
// of the engine and sorts each into where the sheet shows it: a new field breaks it until the
// sheet (and this table) say what to do with it.

beforeAll(async () => {
  await initTestEngine();
});

type Role = "identity" | "value" | "active-effect" | "keyword" | "meta";
const ROLES: Record<keyof CardView, Role> = {
  id: "identity",
  name: "identity",
  cost: "value",
  force: "value",
  health: "value",
  riposte: "value",
  attack_cost: "active-effect",
  provocation: "keyword",
  // The engine's reason this card cannot be played now (#171): a code, never shown as a value or a keyword;
  // the play button of the sheet turns it into its French sentence (`messageFor`).
  unavailable: "meta",
};

describe("the fields of a CardView", () => {
  test("every key of a real card view is classified, and no classified key is missing", () => {
    const s = newSession("rush", "bulwark", 3);
    try {
      const card = s.view("One").you.hand[0];
      // `unavailable` is optional: absent from a card that can be played.
      expect(Object.keys(card).sort()).toEqual(Object.keys(ROLES).filter((k) => k !== "unavailable").sort());
    } finally {
      s.dispose();
    }
  });

  test("a card of a full board's hand carries the one extra key, classified as meta", () => {
    const factory = rulesFactory('{"hero_riposte":false,"board_limit":1}');
    const s = newSession("rush", "bulwark", 3, factory);
    try {
      const play = s.legalActionsWithTime().find((p) => isPlay(p.action))!;
      s.apply(play.action);
      const card = s.view("One").you.hand[0];
      expect(Object.keys(card).sort()).toEqual(Object.keys(ROLES).sort());
      expect(card.unavailable).toBe("board_full");
      expect(ROLES.unavailable).toBe("meta");
    } finally {
      s.dispose();
    }
  });

  test("every keyword field has its French explanation in the single source, and only those", () => {
    const keywords = Object.entries(ROLES)
      .filter(([, role]) => role === "keyword")
      .map(([key]) => key);
    expect(Object.keys(KEYWORDS_FR).sort()).toEqual(keywords.sort());
    for (const k of Object.values(KEYWORDS_FR)) {
      expect(k.label.length).toBeGreaterThan(0);
      expect(k.explanation.length).toBeGreaterThan(10);
    }
    expect(KEYWORDS_FR.provocation.explanation).toBe("tant qu'elle est en jeu, l'adversaire ne peut attaquer qu'une créature qui provoque ; son héros n'est pas ciblable.");
  });
});
