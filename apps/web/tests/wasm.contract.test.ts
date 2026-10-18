import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import { EVENT_KINDS, eventKind } from "../src/engine/types";
import type { Action, ActionPreview, EngineEvent, PlayerView } from "../src/engine/types";
import { DATA_VERSION } from "../src/data/starter";
import { CLIENT_CODES, ENGINE_CODES } from "../src/engine/errors";
import { PAIRS, initTestEngine, newSession, randomPolicy, rulesFactory, testFactory } from "./support/engine";
import { repoRoot } from "./support/paths";

beforeAll(async () => {
  await initTestEngine();
});

const keys = (o: object) => Object.keys(o).sort();

/** Exact keys of each event variant, as the Rust `enum Event` serialises them. */
const EVENT_KEYS: Record<string, string[]> = {
  game_started: ["budget", "data_versions", "first"],
  effect_activated: ["cost", "effect", "player"],
  drew: ["card", "player"],
  played: ["card", "cost", "player"],
  attacked: ["attacker", "cost", "player"],
  damaged: ["amount", "player"],
  creature_damaged: ["amount", "creature", "player"],
  riposted: ["amount", "creature", "player"],
  hero_riposted: ["creature", "player"],
  died: ["creature", "player"],
  token_moved: ["budget", "player"],
  active_player_changed: ["player"],
  game_won: ["reason", "winner"],
};

const snake = (s: string) => s.replace(/(?<!^)([A-Z])/g, "_$1").toLowerCase();

describe("EVENT_KINDS against the Rust source", () => {
  test("is the list of the variants of `enum Event`, in order", () => {
    const rules = readFileSync(resolve(repoRoot, "crates/engine/src/rules.rs"), "utf8");
    const start = rules.indexOf("pub enum Event {");
    expect(start).toBeGreaterThan(-1);
    const body = rules.slice(start, rules.indexOf("\n}\n", start));
    const variants = [...body.matchAll(/^ {4}([A-Z][A-Za-z]*)\s*[{,(]/gm)].map((m) => snake(m[1]));
    expect(variants.length).toBeGreaterThanOrEqual(12);
    expect([...EVENT_KINDS]).toEqual(variants);
  });

  test("EVENT_KEYS of this test cover exactly EVENT_KINDS", () => {
    expect(Object.keys(EVENT_KEYS)).toEqual([...EVENT_KINDS]);
  });
});

describe("the computer's action (#142)", () => {
  test("the three codes of the new method are engine codes, computer_stuck is no longer a client one", () => {
    for (const code of ["not_active_seat", "invalid_policy", "computer_stuck"]) expect(ENGINE_CODES, code).toContain(code);
    expect(CLIENT_CODES).not.toContain("computer_stuck");
    expect(CLIENT_CODES).toContain("computer_chain_limit");
  });

  test("the binding takes (seat, policy, draw) and its result is exactly an action `legalActions` lists", () => {
    let checked = 0;
    for (const [one, two] of PAIRS) {
      for (let seed = 0; seed < 5; seed++) {
        const s = newSession(one, two, seed);
        try {
          for (let i = 0; i < 400 && s.view("One").result === null; i++) {
            const action = s.computerAction(s.view("One").active, (seed + i * 2654435761) >>> 0);
            expect(s.legalActions().map((a) => JSON.stringify(a))).toContain(JSON.stringify(action));
            expect(action).not.toBe("concede");
            checked += 1;
            s.apply(action);
          }
        } finally {
          s.dispose();
        }
      }
    }
    expect(checked).toBeGreaterThan(200);
  });

  test("the Rust sources hold the three codes as literal error_json calls", () => {
    const lib = readFileSync(resolve(repoRoot, "crates/engine-wasm/src/lib.rs"), "utf8");
    for (const code of ["not_active_seat", "invalid_policy", "computer_stuck"]) expect(lib).toContain(`"${code}"`);
  });
});

describe("JSON shapes of real games", () => {
  test("exact keys of every event, action, preview and view met in 160 real games (80 per rule set)", () => {
    const seenEvents = new Set<string>();
    const seenActions = new Set<string>();
    let games = 0;
    const checkEvents = (events: readonly EngineEvent[]) => {
      for (const e of events) {
        expect(Object.keys(e)).toHaveLength(1);
        const kind = eventKind(e);
        expect(EVENT_KINDS).toContain(kind);
        const body = (e as Record<string, object>)[kind];
        // `data_versions` is present on game_started because the setup carries versions.
        // `rules` is the last key of game_started, present only when the options differ from the engine default.
        const expected = kind === "game_started" && "rules" in body ? [...EVENT_KEYS[kind], "rules"].sort() : EVENT_KEYS[kind];
        expect(keys(body)).toEqual(expected);
        seenEvents.add(kind);
      }
    };
    const checkAction = (a: Action) => {
      if (a === "concede") return void seenActions.add("concede");
      expect(Object.keys(a)).toHaveLength(1);
      const kind = Object.keys(a)[0];
      seenActions.add(kind);
      const body = (a as Record<string, object>)[kind];
      if (kind === "play") expect(keys(body)).toEqual(["card"]);
      else if (kind === "activate") expect(keys(body)).toEqual(["effect"]);
      else {
        expect(kind).toBe("attack");
        expect(keys(body)).toEqual(["attacker", "target"]);
        const t = (body as { target: unknown }).target;
        if (t !== "hero") expect(t).toEqual({ creature: expect.any(Number) });
        seenActions.add(t === "hero" ? "attack_hero" : "attack_creature");
      }
    };
    const checkView = (v: PlayerView) => {
      expect(keys(v)).toEqual(["active", "budget", "opponent", "result", "seat", "you"]);
      expect(keys(v.you)).toEqual(["board", "deck_count", "discard", "hand", "hero"]);
      expect(keys(v.opponent)).toEqual(["board", "deck_count", "discard", "hand_count", "hero"]);
      expect(keys(v.you.hero)).toEqual(["effects", "health", "max_time", "name"]);
      const cardKeys = ["attack_cost", "cost", "force", "health", "id", "name", "provocation", "riposte"];
      for (const c of [...v.you.board, ...v.opponent.board, ...v.you.discard]) {
        expect(keys(c)).toEqual(cardKeys);
      }
      // A card of the own hand carries `unavailable` only when it cannot be played (a full board
      // under the standard rules, #171), and then the reason is the one the client knows.
      for (const c of v.you.hand) {
        if ("unavailable" in c) {
          expect(keys(c)).toEqual([...cardKeys, "unavailable"].sort());
          expect(c.unavailable).toBe("board_full");
        } else {
          expect(keys(c)).toEqual(cardKeys);
        }
      }
    };

    // The default rules (standard since #127), then the options off (legacy, the rules written out).
    for (const factory of [testFactory, rulesFactory('{"hero_riposte":false,"board_limit":null}')])
    for (const [one, two] of PAIRS) {
      for (let seed = 0; seed < 20; seed++) {
        const s = newSession(one, two, seed * 7 + 1, factory);
        games += 1;
        try {
          checkEvents(s.startEvents);
          expect(s.startEvents[0]).toEqual({
            game_started: expect.objectContaining({ first: expect.stringMatching(/^(One|Two)$/), budget: expect.any(Number), data_versions: [DATA_VERSION] }),
          });
          const pick = randomPolicy(seed);
          for (let i = 0; i < 400; i++) {
            const previews: ActionPreview[] = s.legalActionsWithTime();
            if (previews.length === 0) break;
            for (const p of previews) {
              expect(keys(p)).toEqual(["action", "attacker_destroyed", "budget_after", "cost", "ends_game", "hands_over"]);
              expect(typeof p.attacker_destroyed).toBe("boolean");
              checkAction(p.action);
              if (p.ends_game) expect(keys(p.ends_game)).toEqual(["reason", "winner"]);
            }
            checkView(s.view("One"));
            checkView(s.view("Two"));
            checkEvents(s.apply(pick(previews.map((p) => p.action))));
          }
        } finally {
          s.dispose();
        }
      }
    }
    expect(games).toBe(160);
    expect([...seenEvents].sort()).toEqual([...EVENT_KINDS].sort());
    expect([...seenActions].sort()).toEqual(["activate", "attack", "attack_creature", "attack_hero", "concede", "play"].sort());
  }, 120_000);
});

describe("the optional rules of Game.from_data (#147)", () => {
  const fixture = (name: string) => readFileSync(resolve(repoRoot, "data/fixture/1.0.0", name), "utf8");
  const VERSIONS = JSON.stringify(["fixture@1.0.0"]);
  const build = (rulesJson?: string | null, versions = VERSIONS) =>
    binding.Game.from_data(fixture("cards.json"), fixture("decks.json"), "aggro", "control", versions, 1, rulesJson);
  let binding: Awaited<ReturnType<typeof initTestEngine>>;

  beforeAll(async () => {
    binding = await initTestEngine();
  });

  test("non-default rules are recorded after data_versions, as the last key of game_started", () => {
    // The options off (`legacy()`) differ from the engine default (standard since #127): the whole
    // object is recorded, so that the game replays the same way whatever the default becomes.
    const game = build('{"hero_riposte":false,"board_limit":null}');
    try {
      const raw = game.start_events();
      expect(raw.endsWith('"rules":{"hero_riposte":false,"board_limit":null}}}]')).toBe(true);
      const started = (JSON.parse(raw)[0] as { game_started: Record<string, unknown> }).game_started;
      expect(Object.keys(started)).toEqual(["first", "budget", "data_versions", "rules"]);
      expect(started.rules).toEqual({ hero_riposte: false, board_limit: null });
      expect(raw.indexOf('"data_versions"')).toBeLessThan(raw.indexOf('"rules"'));
    } finally {
      game.free();
    }
  });

  test("absent rules (undefined or null) and the empty object are the default game, without a rules key", () => {
    const texts = [build(), build(null), build("{}")].map((game) => {
      try {
        return game.start_events();
      } finally {
        game.free();
      }
    });
    expect(new Set(texts).size).toBe(1);
    expect(texts[0]).not.toContain('"rules"');
  });

  test("spelling out the standard rules (the default since #127) records no rules key either", () => {
    const standard = build('{"hero_riposte":true,"board_limit":5}');
    try {
      expect(standard.start_events()).not.toContain('"rules"');
    } finally {
      standard.free();
    }
  });

  test("refusals are JSON {code, message} errors with codes the client knows", () => {
    const huge = " ".repeat(1024 * 1024 + 1);
    const cases: [string, string][] = [
      ['{"nope":1}', "invalid_json"],
      ["", "invalid_json"],
      ['{"board_limit":0}', "invalid_setup"],
      [huge, "input_too_large"],
    ];
    for (const [rules, expected] of cases) {
      let thrown: unknown = null;
      try {
        build(rules).free();
      } catch (e) {
        thrown = e;
      }
      expect(thrown, expected).toBeInstanceOf(Error);
      const body = JSON.parse((thrown as Error).message) as { code: string; message: string };
      expect(body.code).toBe(expected);
      expect(body.message.length).toBeGreaterThan(0);
      expect(ENGINE_CODES).toContain(body.code);
    }
  });
});
