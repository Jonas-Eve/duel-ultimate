import { beforeAll, describe, expect, test } from "vitest";
import { EngineError, SessionDisposedError, isFatalCode, parseEngineError } from "../src/engine/errors";
import type { WasmGameLike } from "../src/engine/session";
import type { Action, GameResult } from "../src/engine/types";
import { isAttack } from "../src/engine/types";
import { messageFor } from "../src/ui/errors.fr";
import { CARDS_JSON, DECKS_JSON } from "../src/data/starter";
import { PAIRS, initTestEngine, randomPolicy, starterConfig, testFactory } from "./support/engine";

beforeAll(async () => {
  await initTestEngine();
});

const json = (code: string, message = "english text") => new Error(JSON.stringify({ code, message }));

describe("parseEngineError", () => {
  test("a binding error keeps its code and message", () => {
    const e = parseEngineError(json("card_not_in_hand", "no such card"));
    expect(e).toBeInstanceOf(EngineError);
    expect(e.code).toBe("card_not_in_hand");
    expect(e.message).toBe("no such card");
    expect(e.fatal).toBe(false);
  });

  test("an EngineError passes through", () => {
    const original = new EngineError("invalid_seed", "x");
    expect(parseEngineError(original)).toBe(original);
  });

  test.each([
    ["invalid JSON", new Error("{not json")],
    ["JSON without code", new Error(JSON.stringify({ message: "x" }))],
    ["JSON with a non-string code", new Error(JSON.stringify({ code: 3, message: "x" }))],
    ["JSON that is not an object", new Error("[1,2]")],
    ["an empty code", new Error(JSON.stringify({ code: "", message: "x" }))],
    ["a RuntimeError", new WebAssembly.RuntimeError("unreachable")],
    ["null pointer", new Error("null pointer passed to rust")],
    ["SessionDisposedError", new SessionDisposedError()],
    ["undefined", undefined],
    ["a string", "boom"],
    ["null", null],
  ])("%s is a fatal internal_error", (_name, e) => {
    const p = parseEngineError(e);
    expect(p.code).toBe("internal_error");
    expect(p.fatal).toBe(true);
    expect(typeof p.message).toBe("string");
  });
});

describe("isFatalCode", () => {
  test("fatal codes", () => {
    for (const c of ["internal_error", "serialization_failed", "computer_stuck", "computer_chain_limit"]) {
      expect(isFatalCode(c)).toBe(true);
    }
  });
  test("rule and creation errors are not fatal", () => {
    for (const c of [
      "game_over", "card_not_in_hand", "unknown_effect", "effect_not_applicable", "attacker_not_on_board",
      "target_not_on_board", "target_forbidden_by_provocation", "invalid_setup", "invalid_json",
      "unknown_deck", "input_too_large", "invalid_seat", "invalid_seed", "wasm_load_failed",
      "not_active_seat", "invalid_policy",
    ]) {
      expect(isFatalCode(c)).toBe(false);
    }
  });
});

// Real errors raised by the real WASM: parseEngineError recognises them, the French table covers them.
function thrown(fn: () => unknown): unknown {
  try {
    fn();
  } catch (e) {
    return e;
  }
  throw new Error("expected a throw");
}

function expectCode(e: unknown, code: string) {
  const p = parseEngineError(e);
  expect(p.code).toBe(code);
  expect(messageFor(code)).not.toContain("Une erreur inattendue");
  expect(messageFor(code)).not.toBe(p.message);
}

const start = (one = "rush", two = "bulwark", seed = 1) => {
  const c = starterConfig(one, two, seed);
  return testFactory(c.cards, c.decks, c.deckOne, c.deckTwo, JSON.stringify([c.dataVersion]), c.seed);
};
const live = <T>(game: WasmGameLike, fn: (g: WasmGameLike) => T): T => {
  try {
    return fn(game);
  } finally {
    game.free();
  }
};
const legal = (g: WasmGameLike) => JSON.parse(g.legal_actions()) as Action[];

describe("real engine errors", () => {
  test("invalid_seat", () => {
    live(start(), (g) => expectCode(thrown(() => g.view("one")), "invalid_seat"));
  });

  test("invalid_policy and not_active_seat, and the order of the checks, from the real binding", () => {
    live(start(), (g) => {
      const active = (JSON.parse(g.view("One")) as { active: string }).active;
      const idle = active === "One" ? "Two" : "One";
      expectCode(thrown(() => g.computer_action(active, "unknown", 0)), "invalid_policy");
      expectCode(thrown(() => g.computer_action(idle, "random", 0)), "not_active_seat");
      expectCode(thrown(() => g.computer_action("one", "random", 0)), "invalid_seat");
      expectCode(thrown(() => g.computer_action(active, "x".repeat(1024 * 1024 + 1), 0)), "input_too_large");
      expect(JSON.parse(g.computer_action(active, "random", 0))).not.toBe("concede");
      expect(JSON.parse(g.computer_action(active, "greedy", 0))).not.toBe("concede");
      g.apply(JSON.stringify("concede"));
      expectCode(thrown(() => g.computer_action(active, "random", 0)), "game_over");
    });
  });

  test("unknown_deck", () => {
    expectCode(thrown(() => start("nope", "rush")), "unknown_deck");
  });

  test("invalid_json (truncated cards and a bad action)", () => {
    const c = starterConfig("rush", "bulwark", 1);
    expectCode(
      thrown(() => testFactory(CARDS_JSON.slice(0, 40), c.decks, "rush", "bulwark", JSON.stringify([c.dataVersion]), 1)),
      "invalid_json",
    );
    live(start(), (g) => expectCode(thrown(() => g.apply("{")), "invalid_json"));
  });

  test("invalid_setup (valid cards.json, decks referencing absent cards)", () => {
    expectCode(thrown(() => testFactory("[]", DECKS_JSON, "rush", "bulwark", "[]", 1)), "invalid_setup");
  });

  test("input_too_large", () => {
    live(start(), (g) => expectCode(thrown(() => g.apply("x".repeat(1024 * 1024 + 1))), "input_too_large"));
  });

  test("game_over after a concession", () => {
    live(start(), (g) => {
      g.apply(JSON.stringify("concede"));
      expectCode(thrown(() => g.apply(JSON.stringify({ play: { card: 1 } }))), "game_over");
    });
  });

  test("card_not_in_hand", () => {
    live(start(), (g) => expectCode(thrown(() => g.apply(JSON.stringify({ play: { card: 999999 } }))), "card_not_in_hand"));
  });

  test("unknown_effect", () => {
    live(start(), (g) => expectCode(thrown(() => g.apply(JSON.stringify({ activate: { effect: 99 } }))), "unknown_effect"));
  });

  test("attacker_not_on_board", () => {
    live(start(), (g) =>
      expectCode(thrown(() => g.apply(JSON.stringify({ attack: { attacker: 999999, target: "hero" } }))), "attacker_not_on_board"),
    );
  });

  test("effect_not_applicable (draw pile emptied by hero effects)", () => {
    for (const [one, two] of PAIRS.slice(0, 2)) {
      const g = start(one, two, 3);
      try {
        for (let step = 0; step < 200; step++) {
          const actions = legal(g);
          for (const effect of [0, 1]) {
            const wanted = JSON.stringify({ activate: { effect } });
            if (!actions.some((a) => JSON.stringify(a) === wanted)) {
              const e = thrown(() => g.apply(wanted));
              expectCode(e, "effect_not_applicable");
              return;
            }
          }
          const activate = actions.find((a) => typeof a === "object" && "activate" in a);
          g.apply(JSON.stringify(activate));
        }
      } finally {
        g.free();
      }
    }
    throw new Error("no draw pile was emptied within the bound");
  });

  /** Plays random games (bounded) until `found` accepts the position; fails explicitly otherwise. */
  function search(found: (g: WasmGameLike) => boolean, label: string): WasmGameLike {
    for (const [one, two] of PAIRS) {
      for (let seed = 1; seed <= 80; seed++) {
        const g = start(one, two, seed);
        const pick = randomPolicy(seed);
        for (let step = 0; step < 300; step++) {
          if (found(g)) return g;
          const actions = legal(g);
          if (actions.length <= 1) break;
          g.apply(JSON.stringify(pick(actions)));
        }
        g.free();
      }
    }
    throw new Error(`no position found for ${label}`);
  }

  const view = (g: WasmGameLike, seat: string) =>
    JSON.parse(g.view(seat)) as {
      active: string;
      result: GameResult | null;
      you: { board: { id: number; provocation: boolean }[] };
      opponent: { board: { id: number; provocation: boolean }[] };
    };

  test("target_not_on_board (own creature on board, unknown target)", () => {
    const g = search((x) => {
      const v = view(x, "One");
      const me = v.active;
      const mine = view(x, me).you.board;
      return v.result === null && mine.length > 0;
    }, "an attacker on board");
    try {
      const me = view(g, "One").active;
      const attacker = view(g, me).you.board[0].id;
      expectCode(thrown(() => g.apply(JSON.stringify({ attack: { attacker, target: { creature: 999999 } } }))), "target_not_on_board");
    } finally {
      g.free();
    }
  });

  test("target_forbidden_by_provocation (attacking the hero past a provoker)", () => {
    const g = search((x) => {
      const me = view(x, "One").active;
      const v = view(x, me);
      return v.result === null && v.you.board.length > 0 && v.opponent.board.some((c) => c.provocation);
    }, "an attacker facing a provocation");
    try {
      const me = view(g, "One").active;
      const attacker = view(g, me).you.board[0].id;
      const e = thrown(() => g.apply(JSON.stringify({ attack: { attacker, target: "hero" } })));
      expectCode(e, "target_forbidden_by_provocation");
      // Sanity: the refused action really was a hero attack, absent from the legal list.
      expect(legal(g).filter(isAttack).some((a) => a.attack.target === "hero")).toBe(false);
    } finally {
      g.free();
    }
  });
});
