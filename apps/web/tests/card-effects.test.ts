import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import type { CardView, EffectLine, OpLine } from "../src/engine/types";
import { cardEffects, effectLineLabel, effectLineText } from "../src/ui/card-effects";
import { KEYWORDS_FR } from "../src/ui/keywords.fr";
import { createWasmSession } from "../src/engine/wasm-session";
import { attackLine, cardView } from "./support/card-view";
import { initTestEngine, testFactory } from "./support/engine";
import { newKindsViews } from "./support/new-kinds";
import { repoRoot } from "./support/paths";

const card = cardView;

describe("cardEffects: the card's view arranged for the detail sheet, nothing computed", () => {
  test("the effect lines are the view's lines, the normal attack and a special one in the same format", () => {
    const special: EffectLine = { ...attackLine(5, 2), index: 0 };
    const c = card({ force: 2, attack_cost: 1, effects: [attackLine(2, 1), special] });
    expect(cardEffects(c).active).toEqual(c.effects);
    expect(cardEffects(card()).active).toEqual([attackLine(1, 2)]);
  });

  test("the values are the card's own, riposte 0 and attack 0 included", () => {
    expect(cardEffects(card({ riposte: 0, force: 0, health: 1, cost: 9 })).values).toEqual({ cost: 9, attack: 0, riposte: 0, health: 1 });
    expect(cardEffects(card({ riposte: 2 })).values.riposte).toBe(2);
  });

  test("an immobile unit has no attack value and no attack line (its view has none); riposte and life stay", () => {
    const c = card({ immobile: true, force: 0, riposte: 1, health: 4, effects: [] });
    const e = cardEffects(c);
    expect(e.values).toEqual({ cost: 3, attack: null, riposte: 1, health: 4 });
    expect(e.active).toEqual([]);
    expect(e.passive.map((k) => k.key)).toEqual(["immobile"]);
  });

  test("a spell has no values but its cost", () => {
    const c = card({ kind: "spell", force: 0, riposte: 0, health: 0, attack_cost: 0, effects: [] });
    expect(cardEffects(c).values).toEqual({ cost: 3, attack: null, riposte: null, health: null });
  });

  test("a passive effect only when the card has the keyword, with the French text of the single source", () => {
    expect(cardEffects(card()).passive).toEqual([]);
    const lines = cardEffects(card({ provocation: true })).passive;
    expect(lines).toEqual([{ key: "provocation", label: KEYWORDS_FR.provocation.label, explanation: KEYWORDS_FR.provocation.explanation }]);
    expect(lines[0].label).toBe("Provocation");
    expect(cardEffects(card({ provocation: true, immobile: true })).passive.map((k) => k.key)).toEqual(["provocation", "immobile"]);
    expect(KEYWORDS_FR.immobile).toEqual({ label: "Immobile", explanation: "ne peut pas attaquer, même si elle gagne de la force." });
    expect(KEYWORDS_FR.ranged).toEqual({ label: "Tir", explanation: "ses attaques ignorent la provocation." });
  });

  test("cardEffects computes nothing: it only arranges the view", () => {
    // A view that contradicts itself: the text follows the line (amount, cost), the values follow the fields.
    const c = card({ force: 9, attack_cost: 7, cost: 1, effects: [attackLine(3, 2)] });
    expect(effectLineLabel(cardEffects(c).active[0])).toBe("Attaque (2) : inflige 3 dégâts, pour 2 temps");
    expect(cardEffects(c).values.attack).toBe(9);
    // No polarity is derived: a malus line stays a malus line whatever its words say.
    const op: OpLine = { op: "deal_damage", target: { select: "hero", side: "own" }, amount: 2, polarity: "bonus" };
    expect(effectLineText({ trigger: "on_play", ops: [op], picks: [] }).visible).not.toMatch(/Malus/);
  });
});

describe("the effect line text: one function builds the visible text and the spoken addition", () => {
  test("visible 'Attaque (N) : inflige D dégât(s)', spoken ', pour N temps'; the label is both", () => {
    expect(effectLineText(attackLine(1, 2))).toEqual({ visible: "Attaque (2) : inflige 1 dégât", hidden: ", pour 2 temps" });
    expect(effectLineLabel(attackLine(1, 2))).toBe("Attaque (2) : inflige 1 dégât, pour 2 temps");
    expect(effectLineLabel(attackLine(4, 3))).toBe("Attaque (3) : inflige 4 dégâts, pour 3 temps");
    expect(effectLineLabel(attackLine(0, 1))).toBe("Attaque (1) : inflige 0 dégât, pour 1 temps");
  });

  test("a special attack line has the same shape and adds only its parameters", () => {
    const base = attackLine(5, 2);
    const withOp = (extra: Record<string, unknown>): EffectLine => ({ ...base, index: 0, ops: [{ ...(base.ops[0] as object), ...extra } as OpLine] });
    const plain = effectLineText(withOp({}));
    expect(plain).toEqual({ visible: "Attaque (2) : inflige 5 dégâts", hidden: ", pour 2 temps" });
    expect(effectLineText(withOp({ ignores_provocation: true })).visible).toBe("Attaque (2) : inflige 5 dégâts ; ignore la provocation");
    expect(effectLineText(withOp({ attackers: 2, mode: "simultaneous" })).visible).toBe("Attaque (2) : inflige 5 dégâts ; 2 attaquants ensemble");
    expect(effectLineText(withOp({ attackers: 2, mode: "chained" })).visible).toBe("Attaque (2) : inflige 5 dégâts ; 2 attaquants à la suite");
    expect(effectLineText(withOp({ scales_with_force: true })).visible).toBe("Attaque (2) : inflige 5 dégâts ; s'ajoute à la force");
    expect(effectLineText(withOp({ ignores_provocation: true, attackers: 2, mode: "chained", scales_with_force: true })).visible).toBe(
      "Attaque (2) : inflige 5 dégâts ; ignore la provocation ; 2 attaquants à la suite ; s'ajoute à la force",
    );
    // A parameter explicitly false or absent adds nothing; the spoken addition never changes.
    expect(effectLineText(withOp({ ignores_provocation: false, scales_with_force: false }))).toEqual(plain);
    expect(effectLineText(withOp({ ignores_provocation: true })).hidden).toBe(plain.hidden);
  });

  const target = (select: string, side?: string) => (side === undefined ? { select } : { select, side }) as any;
  const cases: [string, EffectLine, string][] = [
    ["on_play damage on an opposing unit", { trigger: "on_play", ops: [{ op: "deal_damage", target: target("unit", "opposing"), amount: 2, polarity: "bonus" }], picks: [] }, "À la pose : Inflige 2 dégâts sur une unité adverse"],
    ["one damage on both heroes", { trigger: "on_play", ops: [{ op: "deal_damage", target: target("hero_or_unit", "both"), amount: 1, polarity: "neutral" }], picks: [] }, "À la pose : Inflige 1 dégât sur un héros ou une unité"],
    ["damage on all own units", { trigger: "on_play", ops: [{ op: "deal_damage", target: target("all_units", "own"), amount: 1, polarity: "malus" }], picks: [] }, "À la pose : Inflige 1 dégât sur toutes vos unités — Malus"],
    ["draw, own", { trigger: "on_play", ops: [{ op: "draw", target: target("hero", "own"), count: 1, polarity: "bonus" }], picks: [] }, "À la pose : Pioche 1 carte"],
    ["draw, opposing", { trigger: "on_play", ops: [{ op: "draw", target: target("hero", "opposing"), count: 2, polarity: "malus" }], picks: [] }, "À la pose : L'adversaire pioche 2 cartes — Malus"],
    ["draw, both", { trigger: "on_play", ops: [{ op: "draw", target: target("hero", "both"), count: 1, polarity: "neutral" }], picks: [] }, "À la pose : Chaque joueur pioche 1 carte"],
    ["continuous force bonus", { trigger: "continuous", ops: [{ op: "modify_stat", target: target("all_units", "own"), stat: "force", delta: 2, polarity: "bonus" }], picks: [] }, "Tant qu'elle est en jeu : +2 force sur toutes vos unités"],
    ["continuous attack cost down", { trigger: "continuous", ops: [{ op: "modify_stat", target: target("unit", "opposing"), stat: "attack_cost", delta: -1, polarity: "malus" }], picks: [] }, "Tant qu'elle est en jeu : -1 coût d'attaque sur une unité adverse — Malus"],
    ["continuous summon cost and riposte", { trigger: "continuous", ops: [{ op: "modify_stat", target: target("self"), stat: "riposte", delta: 1, polarity: "bonus" }, { op: "modify_stat", target: target("unit", "both"), stat: "cost", delta: 1, polarity: "neutral" }], picks: [] }, "Tant qu'elle est en jeu : +1 riposte sur cette unité ; +1 coût d'invocation sur une unité"],
    ["continuous keyword", { trigger: "continuous", ops: [{ op: "grant_keyword", target: target("self"), keyword: "ranged", polarity: "bonus" }], picks: [] }, "Tant qu'elle est en jeu : Tir accordé sur cette unité"],
    ["activated line of several operations", { trigger: "activated", cost: 2, ops: [{ op: "deal_damage", target: target("hero_or_unit", "opposing"), amount: 1, polarity: "bonus" }, { op: "draw", target: target("hero", "own"), count: 1, polarity: "bonus" }], picks: [] }, "Effet (2) : Inflige 1 dégât sur le héros adverse ou une unité adverse ; Pioche 1 carte"],
    ["on_play attack of a spell", { trigger: "on_play", ops: [{ op: "attack", target: target("hero_or_unit", "opposing"), amount: 3, attackers: 2, mode: "simultaneous", polarity: "bonus" }], picks: [] }, "À la pose : Attaque : inflige 3 dégâts ; 2 attaquants ensemble"],
    ["a hero target", { trigger: "on_play", ops: [{ op: "deal_damage", target: target("hero_or_unit", "own"), amount: 2, polarity: "malus" }], picks: [] }, "À la pose : Inflige 2 dégâts sur votre héros ou une de vos unités — Malus"],
  ];
  test.each(cases)("%s", (_name, line, visible) => {
    const text = effectLineText(line);
    expect(text.visible).toBe(visible);
    expect(text.hidden).toBe(line.trigger === "activated" ? `, pour ${line.cost} temps` : "");
    expect(effectLineLabel(line)).toBe(text.visible + text.hidden);
  });

  test("polarity comes from the view and only a malus gets a mark", () => {
    const line = (polarity: "bonus" | "malus" | "neutral"): EffectLine => ({
      trigger: "on_play",
      ops: [{ op: "deal_damage", target: { select: "unit", side: "own" }, amount: 1, polarity }],
      picks: [],
    });
    expect(effectLineText(line("bonus")).visible).toBe("À la pose : Inflige 1 dégât sur une de vos unités");
    expect(effectLineText(line("neutral")).visible).toBe("À la pose : Inflige 1 dégât sur une de vos unités");
    expect(effectLineText(line("malus")).visible).toBe("À la pose : Inflige 1 dégât sur une de vos unités — Malus");
  });
});

describe("the normal attack of every unit of the real data sets", () => {
  const decksOf = (path: string) => {
    // Every deck with its whole deck in the opening hand, so that the hand of One shows all 30 cards.
    const decks = JSON.parse(readFileSync(resolve(repoRoot, path), "utf8")) as { id: string; hero: { starting_hand: number } }[];
    for (const d of decks) d.hero.starting_hand = 30;
    return decks;
  };
  beforeAll(async () => {
    await initTestEngine();
  });

  test.each([
    ["starter@1.1.0", "data/starter/1.1.0"],
    ["fixture@1.0.0", "data/fixture/1.0.0"],
  ])("the normal attack text is byte for byte the old one (%s)", (_name, dir) => {
    const cards = readFileSync(resolve(repoRoot, dir, "cards.json"), "utf8");
    const decks = decksOf(`${dir}/decks.json`);
    let seen = 0;
    for (const deck of decks) {
      const session = createWasmSession(testFactory, { cards, decks: JSON.stringify(decks), deckOne: deck.id, deckTwo: deck.id, seed: 1, dataVersion: "t@1" });
      try {
        const hand = session.view("One").you.hand;
        expect(hand.length).toBe(30);
        for (const c of hand) {
          const line = cardEffects(c).active.find((l) => l.index === undefined)!;
          const old = { visible: `Attaque (${c.attack_cost}) : inflige ${c.force} dégât${c.force > 1 ? "s" : ""}`, hidden: `, pour ${c.attack_cost} temps` };
          expect(effectLineText(line), c.name ?? "").toEqual(old);
          expect(effectLineLabel(line)).toBe(old.visible + old.hidden);
          seen += 1;
        }
      } finally {
        session.dispose();
      }
    }
    expect(seen).toBe(decks.length * 30);
  });
});

describe("the new kinds, as the real engine writes them", () => {
  test("an immobile unit has no attack line and no attack value; a spell has no values", async () => {
    const { immobile, spell, special, aura } = await newKindsViews();
    expect(immobile.immobile).toBe(true);
    expect(cardEffects(immobile).active.some((l) => l.ops.some((o) => o.op === "attack"))).toBe(false);
    expect(cardEffects(immobile).values.attack).toBeNull();
    expect(cardEffects(spell).values).toEqual({ cost: 1, attack: null, riposte: null, health: null });
    const texts = (c: CardView) => cardEffects(c).active.map((l) => effectLineLabel(l));
    expect(texts(spell)).toEqual(["À la pose : Inflige 2 dégâts sur une unité adverse ; Pioche 1 carte"]);
    expect(texts(special)).toEqual(["Attaque (1) : inflige 2 dégâts, pour 1 temps", "Attaque (2) : inflige 5 dégâts ; ignore la provocation, pour 2 temps"]);
    expect(texts(aura)).toEqual(["Attaque (1) : inflige 2 dégâts, pour 1 temps", "Tant qu'elle est en jeu : +1 force sur toutes vos unités ; +1 coût d'attaque sur toutes vos unités — Malus"]);
  });
});
