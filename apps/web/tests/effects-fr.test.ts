import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "vitest";
import type { EffectLine, OpLine, Selector, Side } from "../src/engine/types";
import { effectLineText } from "../src/ui/card-effects";
import { MODE_FR, OP_FR, SELECTOR_FR, STAT_FR, TRIGGER_FR } from "../src/ui/effects.fr";
import { KEYWORDS_FR } from "../src/ui/keywords.fr";
import { repoRoot } from "./support/paths";
import { constantNames } from "./support/rust-sets";

// The French table of the effect lines is closed: one entry per element of each closed set of the Rust
// catalogue (`crates/cards/src/effects.rs`, read as source like `error-codes.test.ts` reads `code()`), no
// entry without an element. An element added on one side only breaks this test.

const rust = readFileSync(resolve(repoRoot, "crates/cards/src/effects.rs"), "utf8");

const keys = (o: object) => Object.keys(o).sort();
const sorted = (a: string[]) => [...a].sort();

describe("the extraction itself", () => {
  test("constantNames reads what it should", () => {
    expect(constantNames('pub const A_NAMES: [&str; 2] = ["a_b", "c"];', "A_NAMES")).toEqual(["a_b", "c"]);
    expect(constantNames('pub const A_NAMES: [&str; 2] = [\n    "a_b",\n    "c",\n];\npub const B: [&str; 1] = ["z"];', "A_NAMES")).toEqual(["a_b", "c"]);
    expect(constantNames("nothing", "A_NAMES")).toEqual([]);
  });

  test("it reads at least 5 operations, 5 selectors, 3 sides, 3 triggers, 4 stats (not a blind regex)", () => {
    expect(constantNames(rust, "OP_NAMES").length).toBeGreaterThanOrEqual(5);
    expect(constantNames(rust, "SELECT_NAMES").length).toBeGreaterThanOrEqual(5);
    expect(constantNames(rust, "SIDE_NAMES").length).toBeGreaterThanOrEqual(3);
    expect(constantNames(rust, "TRIGGER_NAMES").length).toBeGreaterThanOrEqual(3);
    expect(constantNames(rust, "STAT_NAMES").length).toBeGreaterThanOrEqual(4);
    expect(constantNames(rust, "KEYWORD_NAMES").length).toBeGreaterThanOrEqual(1);
    expect(constantNames(rust, "MODE_NAMES").length).toBeGreaterThanOrEqual(2);
  });
});

describe("every element of the Rust closed sets has a French entry and no entry is dead", () => {
  test("operations", () => {
    expect(keys(OP_FR)).toEqual(sorted(constantNames(rust, "OP_NAMES")));
  });
  test("selectors", () => {
    expect(keys(SELECTOR_FR)).toEqual(sorted(constantNames(rust, "SELECT_NAMES")));
  });
  test("sides (each selector has an entry for every side)", () => {
    for (const [select, bySide] of Object.entries(SELECTOR_FR)) {
      expect(keys(bySide), select).toEqual(sorted(constantNames(rust, "SIDE_NAMES")));
    }
  });
  test("triggers", () => {
    expect(keys(TRIGGER_FR)).toEqual(sorted(constantNames(rust, "TRIGGER_NAMES")));
  });
  test("stats", () => {
    expect(keys(STAT_FR)).toEqual(sorted(constantNames(rust, "STAT_NAMES")));
    expect(STAT_FR.cost).toBe("coût d'invocation");
    expect(STAT_FR.attack_cost).toBe("coût d'attaque");
    expect(STAT_FR.force).toBe("force");
    expect(STAT_FR.riposte).toBe("riposte");
  });
  test("keywords (the single source of keywords.fr.ts holds the granted ones, plus the card fields)", () => {
    for (const k of constantNames(rust, "KEYWORD_NAMES")) expect(Object.keys(KEYWORDS_FR), k).toContain(k);
  });
  test("attack modes", () => {
    expect(keys(MODE_FR)).toEqual(sorted(constantNames(rust, "MODE_NAMES")));
  });
});

const SIDES = constantNames(rust, "SIDE_NAMES") as Side[];
const selectors = (): Selector[] => [
  { select: "self" },
  ...(["hero", "unit", "hero_or_unit", "all_units"] as const).flatMap((select) => SIDES.map((side): Selector => ({ select, side }))),
];
const lineOf = (op: OpLine, trigger: EffectLine["trigger"] = "on_play"): EffectLine => ({
  trigger,
  cost: trigger === "activated" ? 2 : undefined,
  ops: [op],
  picks: [],
});

describe("the texts", () => {
  test("every text is French prose and says \"unité\", never \"créature\"", () => {
    const texts: string[] = [];
    const sel = selectors();
    for (const target of sel) {
      for (const n of [0, 1, 2]) {
        texts.push(effectLineText(lineOf({ op: "deal_damage", target, amount: n, polarity: "bonus" })).visible);
        texts.push(effectLineText(lineOf({ op: "draw", target, count: n, polarity: "bonus" })).visible);
        texts.push(effectLineText(lineOf({ op: "modify_stat", target, stat: "force", delta: n - 1, polarity: "bonus" }, "continuous")).visible);
        texts.push(effectLineText(lineOf({ op: "grant_keyword", target, keyword: "ranged", polarity: "bonus" }, "continuous")).visible);
        texts.push(effectLineText(lineOf({ op: "attack", target, amount: n, polarity: "bonus", ignores_provocation: true, scales_with_force: true, attackers: 2, mode: "chained" }, "activated")).visible);
      }
    }
    for (const stat of constantNames(rust, "STAT_NAMES")) texts.push(STAT_FR[stat as keyof typeof STAT_FR]);
    for (const s of Object.values(SELECTOR_FR)) texts.push(...Object.values(s));
    for (const t of texts) {
      expect(t.length).toBeGreaterThan(0);
      expect(t, t).not.toMatch(/créature/i);
      expect(t, t).not.toMatch(/\b(the|damage|draw|spell)\b/i);
    }
    for (const side of SIDES) {
      for (const select of ["unit", "hero_or_unit", "all_units"] as const) expect(SELECTOR_FR[select][side], `${select}/${side}`).toMatch(/unité/);
    }
    expect(SELECTOR_FR.self.own).toBe("cette unité");
  });

  test("an operation sentence never has a parenthesis: they are the cost's, with the hourglass", () => {
    for (const target of selectors()) {
      for (const op of [
        { op: "deal_damage", target, amount: 2, polarity: "bonus" },
        { op: "draw", target, count: 2, polarity: "bonus" },
        { op: "modify_stat", target, stat: "attack_cost", delta: -1, polarity: "malus" },
        { op: "grant_keyword", target, keyword: "ranged", polarity: "neutral" },
      ] as OpLine[]) {
        const { visible } = effectLineText(lineOf(op, "continuous"));
        expect(visible, visible).not.toMatch(/[()]/);
      }
    }
  });
});
