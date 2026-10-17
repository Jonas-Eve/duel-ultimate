// The deck emblem chip (#94) through the interface, on the real WASM engine: one chip per card face,
// following the card's own seat (boards, hands, discards), decorative, never in the accessible name.
import { screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { GameController } from "../src/game/controller";
import { expectedCardLabel } from "./support/card-label";
import { EMBLEMS, config } from "./support/config";
import { playUntil, randomPreview, realDeps } from "./support/policies";
import { makeCampsController, makeController, mountGame, useRealEngine } from "./support/ui";

useRealEngine();

const KINDS = ["bolt", "battlements"] as const;
const chips = (root: Element) => [...root.querySelectorAll(".card-emblem")];
const faces = (root: Element) => [...root.querySelectorAll(".card-face")];

describe("the emblem chip on card faces", () => {
  test("against the computer: Ruée cards carry a bolt, Rempart cards battlements, hand and boards alike", async () => {
    const c = makeController("rush", "bulwark", 3);
    await playUntil(c, randomPreview(3), (s) => (s.views.One?.opponent.board.length ?? 0) >= 1 && (s.views.One?.you.board.length ?? 0) >= 1, 60);
    mountGame(c);
    const groups: [string, string][] = [
      ["Ta main", "bolt"],
      ["Ton plateau", "bolt"],
      ["Plateau adverse", "battlements"],
    ];
    for (const [name, kind] of groups) {
      const g = screen.getByRole("group", { name });
      expect(faces(g).length, name).toBeGreaterThan(0);
      expect(g.querySelectorAll(`.card-emblem-${kind}`).length, name).toBe(faces(g).length);
      expect(chips(g).length, name).toBe(faces(g).length);
    }
    for (const chip of chips(document.body)) {
      expect(chip.getAttribute("aria-hidden")).toBe("true");
      expect(chip.closest(".card-art"), "the chip sits on the art").not.toBeNull();
    }
  });

  test("the accessible name of a card is unchanged and mentions no emblem", () => {
    const c = makeController("rush", "bulwark", 3);
    mountGame(c);
    const hand = screen.getByRole("group", { name: "Ta main" });
    const v = c.getSnapshot().views.One!;
    const names = within(hand).getAllByRole("button").map((b) => b.getAttribute("aria-label"));
    for (const card of v.you.hand) expect(names.some((n) => n?.startsWith(`${card.name}, coût ${card.cost}, attaque ${card.force}`))).toBe(true);
    for (const n of names) expect(n).not.toMatch(/éclair|créneaux|emblème|bolt|battlements/i);
    expect(expectedCardLabel(v.you.hand[0], { inHand: true })).toContain(v.you.hand[0].name);
  });

  test("two camps: the emblem follows each card's own seat, even after the camps swap", async () => {
    const c = makeCampsController("rush", "bulwark", 5);
    await playUntil(c, randomPreview(5), (s) => s.views.One?.active === "Two", 80);
    mountGame(c);
    const rows = screen.getAllByRole("group").filter((g) => g.classList.contains("row"));
    const camp = (g: HTMLElement) => (g.getAttribute("aria-label") ?? "").slice(0, 6);
    expect(rows.length).toBeGreaterThanOrEqual(4);
    for (const g of rows) {
      const kind = camp(g) === "Camp 1" ? "bolt" : camp(g) === "Camp 2" ? "battlements" : null;
      expect(kind, g.getAttribute("aria-label")).not.toBeNull();
      expect(g.querySelectorAll(`.card-emblem-${kind}`).length).toBe(faces(g).length);
      expect(chips(g).length).toBe(faces(g).length);
    }
  });

  test("a deck without an emblem has no chip and no hole (the face keeps its structure)", () => {
    const base = config("rush", "bulwark", 3);
    const c = GameController.create({ ...base, deckOne: { id: "rush", name: "Ruée" }, deckTwo: { id: "bulwark", name: "Rempart", emblem: EMBLEMS.bulwark } }, realDeps());
    mountGame(c);
    try {
      const hand = screen.getByRole("group", { name: "Ta main" });
      expect(faces(hand).length).toBeGreaterThan(0);
      expect(chips(hand)).toHaveLength(0);
      for (const f of faces(hand)) expect(f.querySelector(".card-art")).not.toBeNull();
    } finally {
      c.dispose();
    }
  });

  test("a discard card carries the emblem of its owner too", async () => {
    for (let seed = 0; seed < 30; seed++) {
      const c = makeController("rush", "bulwark", seed);
      try {
        await playUntil(c, randomPreview(seed), (s) => (s.views.One?.you.discard.length ?? 0) >= 1, 80);
      } catch {
        continue;
      }
      const { user } = mountGame(c);
      await user.click(within(screen.getByRole("group", { name: "Ton héros" })).getByRole("button", { name: /^Défausse/ }));
      const dialog = screen.getByRole("dialog");
      const items = within(dialog).getAllByRole("listitem");
      expect(items.length).toBeGreaterThan(0);
      for (const li of items) expect(li.querySelectorAll(".card-emblem-bolt")).toHaveLength(1);
      return;
    }
    throw new Error("no game with a discard: widen the search");
  });

  test("only the two known kinds are ever drawn", () => {
    const c = makeController("rush", "bulwark", 3);
    mountGame(c);
    for (const chip of chips(document.body)) expect(KINDS.some((k) => chip.classList.contains(`card-emblem-${k}`))).toBe(true);
  });
});
