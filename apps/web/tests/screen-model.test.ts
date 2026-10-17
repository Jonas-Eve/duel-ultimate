import { beforeAll, describe, expect, test } from "vitest";
import type { Seat } from "../src/engine/types";
import { GameController, type DeckEmblem, type GameConfig, type Mode, type Snapshot } from "../src/game/controller";
import { toScreenModel } from "../src/game/screen-model";
import { EMBLEMS } from "./support/config";
import { initTestEngine } from "./support/engine";
import { randomPreview, realDeps, settle } from "./support/policies";

beforeAll(async () => {
  await initTestEngine();
});

const NAMES: Record<string, string> = { rush: "Ruée", bulwark: "Rempart" };
const config = (mode: Mode, one: string, two: string, seed: number): GameConfig => ({
  mode,
  deckOne: { id: one, name: NAMES[one], emblem: EMBLEMS[one] },
  deckTwo: { id: two, name: NAMES[two], emblem: EMBLEMS[two] },
  seed,
});
/** Same as `config` but with the decks' emblems given explicitly (or none). */
const withEmblems = (c: GameConfig, one: DeckEmblem | undefined, two: DeckEmblem | undefined): GameConfig => ({
  ...c,
  deckOne: { id: c.deckOne.id, name: c.deckOne.name, emblem: one },
  deckTwo: { id: c.deckTwo.id, name: c.deckTwo.name, emblem: two },
});
const other = (s: Seat): Seat => (s === "One" ? "Two" : "One");

/** Every human-turn snapshot of a full random game. */
async function humanSnapshots(mode: Mode, one: string, two: string, seed: number, limit = 400): Promise<Snapshot[]> {
  const c = GameController.create(config(mode, one, two, seed), realDeps());
  const out: Snapshot[] = [];
  try {
    c.start();
    const choose = randomPreview(seed);
    for (let i = 0; i < limit; i++) {
      const s = await settle(c);
      out.push(s);
      if (s.phase !== "human-turn") break;
      c.submit(choose(s));
    }
    return out;
  } finally {
    c.dispose();
  }
}

describe("against the computer", () => {
  test("the human is at the bottom with a visible hand, the computer on top with a hidden one, at every step", async () => {
    for (const s of await humanSnapshots("computer", "rush", "bulwark", 4)) {
      const m = toScreenModel(s);
      const v = s.views.One!;
      expect(m.bottom.seat).toBe("One");
      expect(m.bottom.hand).toEqual(v.you.hand);
      expect(m.bottom.handCount).toBe(v.you.hand.length);
      expect(m.bottom.deckCount).toBe(v.you.deck_count);
      expect(m.bottom.board).toEqual(v.you.board);
      expect(m.bottom.discard).toEqual(v.you.discard);
      expect(m.bottom.hero).toEqual(v.you.hero);
      expect(m.top.seat).toBe("Two");
      expect(m.top.hand).toBeNull();
      expect(m.top.handCount).toBe(v.opponent.hand_count);
      expect(m.top.deckCount).toBe(v.opponent.deck_count);
      expect(m.top.board).toEqual(v.opponent.board);
      expect(m.top.discard).toEqual(v.opponent.discard);
      expect(m.top.hero).toEqual(v.opponent.hero);
      expect(m.bottom.deckName).toBe(s.players.One.name);
      expect(m.top.deckName).toBe(s.players.Two.name);
    }
  });

  test("labels, budget and active come from the view, never computed", async () => {
    const snaps = await humanSnapshots("computer", "rush", "bulwark", 4);
    for (const s of snaps) {
      const m = toScreenModel(s);
      expect(m.bottom.label).toBe("Toi — Ruée");
      expect(m.top.label).toBe("Ordinateur — Rempart");
      expect(m.budget).toBe(s.views.One!.budget);
      expect(m.active).toBe(s.views.One!.active);
      expect(m.bottomIsActive).toBe(s.views.One!.active === "One");
      expect(m.bottom.active).toBe(m.bottomIsActive);
      expect(m.top.active).toBe(!m.bottomIsActive);
    }
  });

  test("the human stays at the bottom even while the computer holds the main", async () => {
    const c = GameController.create(config("computer", "bulwark", "rush", 3), realDeps());
    try {
      c.start();
      const m = toScreenModel(c.getSnapshot());
      expect(c.getSnapshot().phase).toBe("computer-turn");
      expect(m.bottom.seat).toBe("One");
      expect(m.bottomIsActive).toBe(false);
      expect(m.top.active).toBe(true);
      expect(m.top.hand).toBeNull();
    } finally {
      c.dispose();
    }
  });
});

describe("two camps", () => {
  test("the camp that holds the main is at the bottom, both hands are visible, decks are counts only", async () => {
    const snaps = await humanSnapshots("two-camps", "rush", "bulwark", 5);
    let swaps = 0;
    let previousBottom: Seat | null = null;
    for (const s of snaps) {
      const m = toScreenModel(s);
      const active = s.views.One!.active;
      expect(m.bottom.seat).toBe(active);
      expect(m.top.seat).toBe(other(active));
      expect(m.bottomIsActive).toBe(true);
      expect(m.active).toBe(active);
      const own = s.views[active]!.you;
      const foe = s.views[other(active)]!.you;
      expect(m.bottom.hand).toEqual(own.hand);
      expect(m.top.hand).toEqual(foe.hand);
      expect(m.top.hand).not.toBeNull();
      expect(m.bottom.deckCount).toBe(own.deck_count);
      expect(m.top.deckCount).toBe(foe.deck_count);
      expect(m.bottom.handCount).toBe(own.hand.length);
      expect(m.top.handCount).toBe(foe.hand.length);
      expect(m.bottom.board).toEqual(own.board);
      expect(m.top.board).toEqual(foe.board);
      expect(m.bottom.hero).toEqual(own.hero);
      expect(m.top.hero).toEqual(foe.hero);
      expect(m.budget).toBe(s.views[active]!.budget);
      expect("deck" in m.bottom).toBe(false);
      if (previousBottom !== null && previousBottom !== m.bottom.seat) swaps += 1;
      previousBottom = m.bottom.seat;
    }
    expect(swaps).toBeGreaterThan(0);
  });

  test("labels name the camp and the deck, and the mirror is told apart by the camp number", async () => {
    const [s] = await humanSnapshots("two-camps", "rush", "bulwark", 5, 1);
    const m = toScreenModel(s);
    const byseat = { [m.bottom.seat]: m.bottom.label, [m.top.seat]: m.top.label } as Record<Seat, string>;
    expect(byseat.One).toBe("Camp 1 — Ruée");
    expect(byseat.Two).toBe("Camp 2 — Rempart");

    const [mirror] = await humanSnapshots("two-camps", "bulwark", "bulwark", 5, 1);
    const mm = toScreenModel(mirror);
    const labels = [mm.bottom.label, mm.top.label].sort();
    expect(labels).toEqual(["Camp 1 — Rempart", "Camp 2 — Rempart"]);
  });

  test("a finished game still has a screen model", async () => {
    const snaps = await humanSnapshots("two-camps", "rush", "bulwark", 1);
    const last = snaps[snaps.length - 1];
    expect(last.phase).toBe("over");
    const m = toScreenModel(last);
    expect(m.top.seat).not.toBe(m.bottom.seat);
  });
});

describe("the deck emblem of each side (#94)", () => {
  test("against the computer: each side carries the emblem of its own seat's deck", () => {
    const c = GameController.create(config("computer", "rush", "bulwark", 2), realDeps());
    try {
      c.start();
      const m = toScreenModel(c.getSnapshot());
      expect(m.bottom.emblem).toBe("bolt");
      expect(m.top.emblem).toBe("battlements");
    } finally {
      c.dispose();
    }
  });

  test("two camps: the emblem follows the seat, not the camp holding the main, through every swap", async () => {
    let swaps = 0;
    for (const s of await humanSnapshots("two-camps", "rush", "bulwark", 5)) {
      const m = toScreenModel(s);
      for (const side of [m.top, m.bottom]) expect(side.emblem).toBe(side.seat === "One" ? "bolt" : "battlements");
      if (m.bottom.seat === "Two") swaps += 1;
    }
    expect(swaps).toBeGreaterThan(0);
  });

  test.each(["computer", "two-camps"] as const)("%s: no emblem on a deck means undefined on its side (the other keeps its own)", (mode) => {
    const c = GameController.create(withEmblems(config(mode, "rush", "bulwark", 2), undefined, "battlements"), realDeps());
    try {
      c.start();
      const m = toScreenModel(c.getSnapshot());
      for (const side of [m.top, m.bottom]) expect(side.emblem).toBe(side.seat === "One" ? undefined : "battlements");
    } finally {
      c.dispose();
    }
  });
});
