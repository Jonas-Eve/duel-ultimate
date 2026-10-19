import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import type { CardView } from "../src/engine/types";
import { isPlay } from "../src/engine/types";
import { KEYWORDS_FR } from "../src/ui/keywords.fr";
import { initTestEngine, newSession, rulesFactory } from "./support/engine";
import { repoRoot } from "./support/paths";
import { constantNames } from "./support/rust-sets";

// The detail sheet explains every field of a card. This test reads the keys of a REAL `CardView`
// of the engine and sorts each into where the sheet shows it: a new field breaks it until the
// sheet (and this table) say what to do with it.

beforeAll(async () => {
  await initTestEngine();
});

type Role = "identity" | "value" | "active-effect" | "keyword" | "tags" | "effects" | "meta";
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
  // The fields of #209, finalised by #210 (the sheet shows them all): the kind says which values a card has,
  // `immobile` is a keyword (explained from the single source), the tags are one text line, the effect lines
  // are the generic lines of the sheet (the normal attack and the declared ones in the same format).
  kind: "identity",
  immobile: "keyword",
  tags: "tags",
  effects: "effects",
};

describe("the fields of a CardView", () => {
  test("the roles of the new fields are final", () => {
    expect(ROLES.kind).toBe("identity");
    expect(ROLES.immobile).toBe("keyword");
    expect(ROLES.tags).toBe("tags");
    expect(ROLES.effects).toBe("effects");
  });

  test("every key of a real card view is classified, and no classified key is missing", () => {
    const s = newSession("rush", "bulwark", 3);
    try {
      const card = s.view("One").you.hand[0];
      // `unavailable`, `immobile` and `tags` are optional: absent from a plain card that can be played.
      const optional = ["unavailable", "immobile", "tags"];
      expect(Object.keys(card).sort()).toEqual(Object.keys(ROLES).filter((k) => !optional.includes(k)).sort());
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
      expect(Object.keys(card).sort()).toEqual(Object.keys(ROLES).filter((k) => !["immobile", "tags"].includes(k)).sort());
      expect(card.unavailable).toBe("board_full");
      expect(ROLES.unavailable).toBe("meta");
    } finally {
      s.dispose();
    }
  });

  test("the keys of KEYWORDS_FR are the keyword fields of the view plus the KEYWORD_NAMES of Rust", () => {
    const keywords = Object.entries(ROLES)
      .filter(([, role]) => role === "keyword")
      .map(([key]) => key);
    // `ranged` is not a field of a card but a keyword that an effect grants.
    const granted = constantNames(readFileSync(resolve(repoRoot, "crates/cards/src/effects.rs"), "utf8"), "KEYWORD_NAMES");
    expect(granted.length).toBeGreaterThan(0);
    expect(Object.keys(KEYWORDS_FR).sort()).toEqual([...new Set([...keywords, ...granted])].sort());
    for (const k of Object.values(KEYWORDS_FR)) {
      expect(k.label.length).toBeGreaterThan(0);
      expect(k.explanation.length).toBeGreaterThan(10);
    }
    expect(KEYWORDS_FR.provocation.explanation).toBe("tant qu'elle est en jeu, l'adversaire ne peut attaquer qu'une créature qui provoque ; son héros n'est pas ciblable.");
  });
});
