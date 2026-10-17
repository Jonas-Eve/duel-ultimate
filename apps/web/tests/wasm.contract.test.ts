import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeAll, describe, expect, test } from "vitest";
import { EVENT_KINDS, eventKind } from "../src/engine/types";
import type { Action, ActionPreview, EngineEvent, PlayerView } from "../src/engine/types";
import { DATA_VERSION } from "../src/data/starter";
import { CLIENT_CODES, ENGINE_CODES } from "../src/engine/errors";
import { PAIRS, initTestEngine, newSession, randomPolicy } from "./support/engine";
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
  test("exact keys of every event, action, preview and view met in 80 real games", () => {
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
        expect(keys(body)).toEqual(EVENT_KEYS[kind]);
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
      for (const c of [...v.you.hand, ...v.you.board, ...v.opponent.board, ...v.you.discard]) {
        expect(keys(c)).toEqual(["attack_cost", "cost", "force", "health", "id", "name", "provocation", "riposte"]);
      }
    };

    for (const [one, two] of PAIRS) {
      for (let seed = 0; seed < 20; seed++) {
        const s = newSession(one, two, seed * 7 + 1);
        games += 1;
        try {
          checkEvents(s.startEvents);
          expect(s.startEvents[0]).toEqual({
            game_started: { first: expect.stringMatching(/^(One|Two)$/), budget: expect.any(Number), data_versions: [DATA_VERSION] },
          });
          const pick = randomPolicy(seed);
          for (let i = 0; i < 400; i++) {
            const previews: ActionPreview[] = s.legalActionsWithTime();
            if (previews.length === 0) break;
            for (const p of previews) {
              expect(keys(p)).toEqual(["action", "budget_after", "cost", "ends_game", "hands_over"]);
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
    expect(games).toBe(80);
    expect([...seenEvents].sort()).toEqual([...EVENT_KINDS].sort());
    expect([...seenActions].sort()).toEqual(["activate", "attack", "attack_creature", "attack_hero", "concede", "play"].sort());
  }, 120_000);
});
