import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import { repoRoot, webRoot } from "./support/paths";

const wasmDir = resolve(webRoot, "src/wasm");
const wasmFile = resolve(wasmDir, "duel_engine_wasm_bg.wasm");

type Binding = typeof import("../src/wasm/duel_engine_wasm");
let binding: Binding;

beforeAll(async () => {
  if (!existsSync(wasmFile)) throw new Error(`${wasmFile} is missing: run \`npm test\` / \`npm run build\` (the pre* hook generates it)`);
  binding = await import("../src/wasm/duel_engine_wasm");
  binding.initSync({ module: readFileSync(wasmFile) });
});

const fixture = (name: string) => readFileSync(resolve(repoRoot, "data/fixture/1.0.0", name), "utf8");
const VERSIONS = JSON.stringify(["fixture@1.0.0"]);
const cardNames = new Set(
  (JSON.parse(fixture("cards.json")) as { name: string }[]).map((c) => c.name),
);

interface Card {
  id: number;
  name: string | null;
  cost: number;
  force: number;
  attack_cost: number;
  health: number;
  provocation: boolean;
  riposte: number;
  kind: "unit" | "spell";
  immobile?: boolean;
  tags?: string[];
  effects: { index?: number; trigger: string; cost?: number; ops: unknown[]; picks: unknown[] }[];
  unavailable?: string;
}
interface Hero {
  name: string;
  health: number;
  max_time: number;
  effects: { name: string; cost: number; effect: { kind: string; count: number } }[];
}
interface View {
  seat: "One" | "Two";
  active: "One" | "Two";
  budget: number;
  result: { winner: string; reason: string } | null;
  you: { hero: Hero; hand: Card[]; deck_count: number; board: Card[]; discard: Card[] };
  opponent: { hero: Hero; hand_count: number; deck_count: number; board: Card[]; discard: Card[] };
}

function newGame(seed = 1) {
  return binding.Game.from_data(fixture("cards.json"), fixture("decks.json"), "aggro", "control", VERSIONS, seed);
}

const viewOf = (game: InstanceType<Binding["Game"]>, seat: string) => JSON.parse(game.view(seat)) as View;

function errorOf(fn: () => unknown): Error {
  try {
    fn();
  } catch (e) {
    return e as Error;
  }
  throw new Error("expected a throw");
}

const idsOf = (cards: Card[]) => cards.map((c) => c.id);

describe("Game.view(seat) in the browser build", () => {
  test("the view of a fresh game has the agreed shape, in both seats", () => {
    const game = newGame();
    try {
      const one = viewOf(game, "One");
      const two = viewOf(game, "Two");
      expect(one.seat).toBe("One");
      expect(two.seat).toBe("Two");
      expect(one.active).toBe(two.active);
      expect(one.budget).toBe(two.budget);
      expect(one.result).toBeNull();
      expect(Object.keys(one).sort()).toEqual(["active", "budget", "opponent", "result", "seat", "you"]);
      expect(Object.keys(one.you).sort()).toEqual(["board", "deck_count", "discard", "hand", "hero"]);
      expect(Object.keys(one.opponent).sort()).toEqual(["board", "deck_count", "discard", "hand_count", "hero"]);
      expect(Object.keys(one.you.hero).sort()).toEqual(["effects", "health", "max_time", "name"]);
      expect(Object.keys(one.you.hand[0]).sort()).toEqual(
        ["attack_cost", "cost", "effects", "force", "health", "id", "kind", "name", "provocation", "riposte"],
      );
      // The attack of a plain unit is a line of the view, without an index (#209).
      for (const card of one.you.hand) {
        expect(card.kind).toBe("unit");
        expect(card.effects).toHaveLength(1);
        expect(card.effects[0].index).toBeUndefined();
        expect(card.effects[0].trigger).toBe("activated");
      }
      // The same public blocks, seen from the two seats.
      expect(one.you.hero).toEqual(two.opponent.hero);
      expect(one.opponent.hero).toEqual(two.you.hero);
      expect(one.you.hand.length).toBe(two.opponent.hand_count);
      expect(two.you.hand.length).toBe(one.opponent.hand_count);
      expect(one.you.deck_count).toBe(two.opponent.deck_count);
      // Names come from the data.
      for (const card of one.you.hand) expect(cardNames.has(card.name as string)).toBe(true);
    } finally {
      game.free();
    }
  });

  test("a seat never learns the other hand nor a pile, whoever is active, across a whole game", () => {
    const game = newGame(3);
    try {
      for (let step = 0; step < 400; step++) {
        const one = viewOf(game, "One");
        const two = viewOf(game, "Two");
        for (const [mine, witness] of [[one, two], [two, one]] as const) {
          const text = JSON.stringify(mine);
          const hidden = new Set(idsOf(witness.you.hand));
          const mentioned = new Set<number>(
            [...mine.you.hand, ...mine.you.board, ...mine.you.discard, ...mine.opponent.board, ...mine.opponent.discard].map((c) => c.id),
          );
          for (const id of mentioned) expect(hidden.has(id), `step ${step}: card ${id} leaked`).toBe(false);
          // Every id in the text is one of the mentioned cards: nothing hides in another field.
          const inText = [...text.matchAll(/"id":(\d+)/g)].map((m) => Number(m[1]));
          expect(new Set(inText)).toEqual(mentioned);
          expect(text).not.toMatch(/seed|data_versions/);
        }
        const actions = (JSON.parse(game.legal_actions()) as unknown[]).filter((a) => a !== "concede");
        if (actions.length === 0) break;
        game.apply(JSON.stringify(actions[(step * 5) % actions.length]));
      }
    } finally {
      game.free();
    }
  });

  test("the view follows the game: a played card moves from the hand to the board", () => {
    const game = newGame();
    try {
      const active = viewOf(game, "One").active;
      const before = viewOf(game, active);
      const play = (JSON.parse(game.legal_actions()) as { play?: { card: number } }[]).find((a) => a.play);
      expect(play).toBeDefined();
      const card = play!.play!.card;
      expect(idsOf(before.you.hand)).toContain(card);
      game.apply(JSON.stringify(play));
      const after = viewOf(game, active);
      expect(idsOf(after.you.hand)).not.toContain(card);
      expect(idsOf(after.you.board)).toContain(card);
      expect(idsOf(viewOf(game, active === "One" ? "Two" : "One").opponent.board)).toContain(card);
    } finally {
      game.free();
    }
  });

  test("a conceded game shows its result in the view", () => {
    const game = newGame();
    try {
      const loser = viewOf(game, "One").active;
      game.apply(JSON.stringify("concede"));
      const result = viewOf(game, "One").result;
      expect(result).toEqual({ winner: loser === "One" ? "Two" : "One", reason: "concede" });
    } finally {
      game.free();
    }
  });

  test.each(["Three", "one", "two", "", " One", "One\u0000", "null"])("seat %j throws the structured invalid_seat error", (seat) => {
    const game = newGame();
    try {
      const e = errorOf(() => game.view(seat));
      expect(e).toBeInstanceOf(Error);
      const body = JSON.parse(e.message);
      expect(body.code).toBe("invalid_seat");
      expect(typeof body.message).toBe("string");
    } finally {
      game.free();
    }
  });

  test("a refused view leaves the game playable and free() is safe afterwards", () => {
    const game = newGame();
    errorOf(() => game.view("Three"));
    expect(JSON.parse(game.legal_actions()).length).toBeGreaterThan(0);
    game.free();
  });
});

describe("the reason a card cannot be played (#171)", () => {
  const limited = () =>
    binding.Game.from_data(fixture("cards.json"), fixture("decks.json"), "aggro", "control", VERSIONS, 1, '{"hero_riposte":false,"board_limit":1}');

  test("absent from every card of a game without a full board", () => {
    const game = newGame();
    for (const seat of ["One", "Two"]) expect(game.view(seat)).not.toContain("unavailable");
  });

  test("present, as an engine code, on each card of the hand of the seat whose board is full only", () => {
    const game = limited();
    const first = viewOf(game, "One").active;
    const other = first === "One" ? "Two" : "One";
    const actions = JSON.parse(game.legal_actions()) as ({ play: { card: number } } | string)[];
    const play = actions.find((a) => typeof a === "object" && "play" in a)!;
    game.apply(JSON.stringify(play));

    const full = viewOf(game, first);
    expect(full.you.hand.length).toBeGreaterThan(0);
    for (const card of full.you.hand) expect(card.unavailable).toBe("board_full");
    for (const card of [...full.you.board, ...full.opponent.board, ...full.you.discard, ...full.opponent.discard]) {
      expect(card.unavailable).toBeUndefined();
    }
    // The other seat has room: its own hand has no key, and neither has the opponent block of the full seat.
    expect(game.view(other)).not.toContain("unavailable");
    expect(JSON.stringify(full.opponent)).not.toContain("unavailable");
  });
});
