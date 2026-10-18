import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import { EVENT_KINDS, eventKind } from "../src/engine/types";
import type { Action, ActionPreview, EngineEvent, PlayerView } from "../src/engine/types";
import { DATA_VERSION } from "../src/data/starter";
import { CLIENT_CODES, ENGINE_CODES, EngineError } from "../src/engine/errors";
import { PAIRS, initTestEngine, newSession, randomPolicy, rulesFactory, testFactory } from "./support/engine";
import { repoRoot } from "./support/paths";
import { kindsGame } from "./support/kinds-game";
import type { CardView } from "../src/engine/types";

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

describe("Action additive (#128)", () => {
  const refusal = (fn: () => unknown): EngineError => {
    try {
      fn();
    } catch (e) {
      expect(e).toBeInstanceOf(EngineError);
      return e as EngineError;
    }
    throw new Error("expected a refusal");
  };
  const firstPlay = (actions: Action[]) => actions.find((a): a is { play: { card: number } } => typeof a === "object" && "play" in a)!;
  const picks = (n: number) => Array.from({ length: n }, () => ({ hero: "One" as const }));

  test("no legal action carries picks or source", () => {
    for (const [one, two] of PAIRS) {
      const s = newSession(one, two, 3);
      try {
        for (let i = 0; i < 200 && s.view("One").result === null; i++) {
          const legal = s.legalActions();
          for (const a of legal) {
            const text = JSON.stringify(a);
            expect(text).not.toContain("picks");
            expect(text).not.toContain("source");
          }
          s.apply(randomPolicy(i)(legal));
        }
      } finally {
        s.dispose();
      }
    }
  });

  test("an empty picks list is accepted", () => {
    const s = newSession("rush", "bulwark", 1);
    try {
      const play = firstPlay(s.legalActions());
      expect(() => s.apply({ play: { card: play.play.card, picks: [] } })).not.toThrow();
    } finally {
      s.dispose();
    }
  });

  test("hostile picks and sources get their stable non-fatal codes, and the game goes on", () => {
    const s = newSession("rush", "bulwark", 1);
    try {
      const card = firstPlay(s.legalActions()).play.card;
      const before = JSON.stringify(s.view("One"));
      const cases: [Action, string][] = [
        [{ play: { card, picks: picks(1) } }, "invalid_picks"],
        [{ activate: { effect: 0, picks: picks(1) } }, "invalid_picks"],
        [{ play: { card, picks: picks(9) } }, "too_many_picks"],
        [{ activate: { effect: 0, source: 10000 } }, "source_not_on_board"],
      ];
      for (const [action, code] of cases) {
        const error = refusal(() => s.apply(action));
        expect(error.code).toBe(code);
        expect(error.fatal).toBe(false);
        expect(ENGINE_CODES).toContain(code);
      }
      expect(JSON.stringify(s.view("One"))).toBe(before);
      expect(() => s.apply({ play: { card } })).not.toThrow();
    } finally {
      s.dispose();
    }
  });
});

describe("the CardView contract (#209), read on a real game with a spell, an immobile unit and a special attack", () => {
  const REQUIRED = ["attack_cost", "cost", "effects", "force", "health", "id", "kind", "name", "provocation", "riposte"];
  const OPTIONAL = ["immobile", "tags", "unavailable"];
  const LINE_REQUIRED = ["ops", "picks", "trigger"];
  const LINE_OPTIONAL = ["cost", "index"];
  const POLARITIES = ["bonus", "malus", "neutral"];

  const checkLine = (line: Record<string, unknown>) => {
    const k = keys(line);
    for (const key of LINE_REQUIRED) expect(k).toContain(key);
    for (const key of k) expect([...LINE_REQUIRED, ...LINE_OPTIONAL]).toContain(key);
    expect(["on_play", "activated", "continuous"]).toContain(line.trigger);
    if ("index" in line) expect(Number.isInteger(line.index)).toBe(true);
    if ("cost" in line) expect(typeof line.cost).toBe("number");
    expect(Array.isArray(line.ops)).toBe(true);
    for (const op of line.ops as Record<string, unknown>[]) {
      expect(["attack", "deal_damage", "draw", "modify_stat", "grant_keyword"]).toContain(op.op);
      expect(POLARITIES).toContain(op.polarity);
      const target = op.target as Record<string, unknown>;
      expect(["self", "hero", "unit", "hero_or_unit", "all_units"]).toContain(target.select);
      if (target.select === "self") expect(keys(target)).toEqual(["select"]);
      else expect(["own", "opposing", "both"]).toContain(target.side);
    }
    for (const pick of line.picks as Record<string, unknown>[]) {
      expect(keys(pick)).toEqual(["kind", "role", "side"]);
      expect(["attacker", "target"]).toContain(pick.role);
      expect(["unit", "hero_or_unit"]).toContain(pick.kind);
      expect(["own", "opposing", "both"]).toContain(pick.side);
    }
  };

  const checkCard = (c: CardView) => {
    const k = keys(c);
    for (const key of REQUIRED) expect(k, `${key} of ${JSON.stringify(c)}`).toContain(key);
    for (const key of k) expect([...REQUIRED, ...OPTIONAL], key).toContain(key);
    expect(["unit", "spell"]).toContain(c.kind);
    if ("immobile" in c) expect(c.immobile).toBe(true);
    if ("tags" in c) {
      expect(c.tags!.length).toBeGreaterThan(0);
      for (const t of c.tags!) expect(typeof t).toBe("string");
    }
    expect(Array.isArray(c.effects)).toBe(true);
    for (const line of c.effects) checkLine(line as unknown as Record<string, unknown>);
  };

  test("a CardView has exactly the contract keys", async () => {
    const game = await kindsGame();
    try {
      const view = JSON.parse(game.view("One")) as PlayerView;
      const hand = view.you.hand;
      for (const c of hand) checkCard(c);
      const byName = (n: string) => hand.find((c) => c.name === n)!;
      // The spell: kind, tags, its declared line only (with an index), two attackers then a target.
      const spell = byName("zz-spell");
      expect(spell.kind).toBe("spell");
      expect(spell.tags).toEqual(["rituel"]);
      expect(spell.effects).toHaveLength(1);
      expect(spell.effects[0].index).toBe(0);
      expect(spell.effects[0].picks.map((p) => p.role)).toEqual(["attacker", "attacker", "target"]);
      expect(spell.unavailable).toBe("unsupported_effect");
      // The immobile unit: the keyword, its tag, no attack line.
      const immobile = byName("zz-immobile");
      expect(immobile).toMatchObject({ kind: "unit", immobile: true, tags: ["bâtiment"], effects: [] });
      // The unit with a special attack: the synthesised line first (no index), then the declared one.
      const special = byName("zz-special");
      expect(special.effects.map((l) => l.index)).toEqual([undefined, 0]);
      expect(special.effects.map((l) => l.cost)).toEqual([special.attack_cost, 2]);
      expect(special.effects[0].ops[0]).toMatchObject({ op: "attack", amount: special.force, polarity: "bonus" });
      expect(special.effects[1].ops[0]).toMatchObject({ op: "attack", amount: 5, polarity: "bonus" });
      // A plain unit: only `kind` and the attack line were added; no tags, no keyword.
      const plain = hand.find((c) => !("tags" in c) && c.kind === "unit" && c.effects.length === 1)!;
      expect(plain).toBeDefined();
      expect(plain.effects[0].index).toBeUndefined();
      expect("immobile" in plain).toBe(false);
      // The other seat sees no tag of the hand it cannot see.
      expect(game.view("Two")).not.toContain("rituel");
      expect(game.view("Two")).not.toContain("bâtiment");
      // Every card of the two views, whatever the zone, honours the contract.
      for (const seat of ["One", "Two"] as const) {
        const v = JSON.parse(game.view(seat)) as PlayerView;
        for (const c of [...v.you.board, ...v.you.discard, ...v.opponent.board, ...v.opponent.discard]) checkCard(c);
      }
    } finally {
      game.free();
    }
  });
});
