import { beforeAll, describe, expect, test, vi } from "vitest";
import { EngineError } from "../src/engine/errors";
import type { ActionPreview, Seat } from "../src/engine/types";
import { GameController, type GameConfig, type Mode, type Snapshot } from "../src/game/controller";
import { deriveAiSeed, mulberry32 } from "../src/game/seed";
import { PAIRS, decorateSession, initTestEngine, newSession } from "./support/engine";
import { faultyFactory } from "./support/faulty";
import { fakeSession } from "./support/fake-session";
import { immediateScheduler, instrumentedFactory, playHuman, randomPreview, realDeps, settle, stallPolicy, stallPreview } from "./support/policies";

beforeAll(async () => {
  await initTestEngine();
});

const NAMES: Record<string, string> = { rush: "Ruée", bulwark: "Rempart" };
const config = (mode: Mode, one: string, two: string, seed: number): GameConfig => ({
  mode,
  deckOne: { id: one, name: NAMES[one] },
  deckTwo: { id: two, name: NAMES[two] },
  seed,
});

/** Collects every distinct snapshot published to subscribers. */
function watch(c: GameController): { snapshots: Snapshot[] } {
  const snapshots: Snapshot[] = [];
  c.subscribe(() => snapshots.push(c.getSnapshot()));
  return { snapshots };
}

describe("phases at the start", () => {
  test("the computer opens (bulwark vs rush): computer-turn first, with an empty legal list and 0 steps", async () => {
    const c = GameController.create(config("computer", "bulwark", "rush", 3), realDeps());
    try {
      c.start();
      const s = c.getSnapshot();
      expect(s.phase).toBe("computer-turn");
      expect(s.computerSteps).toBe(0);
      expect(s.legal).toEqual([]);
      expect(s.views.One?.active).toBe("Two");
      expect(s.views.Two).toBeUndefined();
      const after = await settle(c);
      expect(after.phase).toBe("human-turn");
      expect(after.legal.length).toBeGreaterThan(0);
      expect(after.records.some((r) => r.by === "computer")).toBe(true);
      expect(after.computerSteps).toBeGreaterThan(0);
    } finally {
      c.dispose();
    }
  });

  test("the human opens (rush vs bulwark): human-turn at once", () => {
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps());
    try {
      c.start();
      const s = c.getSnapshot();
      expect(s.phase).toBe("human-turn");
      expect(s.legal.length).toBeGreaterThan(0);
      expect(s.records).toHaveLength(1);
      expect(s.records[0].by).toBe("setup");
      expect(s.records[0].seq).toBe(0);
    } finally {
      c.dispose();
    }
  });

  test.each(["rush", "bulwark"])("mirror %s over 40 seeds: both openings occur and none blocks", async (deck) => {
    const phases = new Set<string>();
    for (let seed = 0; seed < 40; seed++) {
      const c = GameController.create(config("computer", deck, deck, seed), realDeps());
      try {
        c.start();
        phases.add(c.getSnapshot().phase);
        const s = await settle(c);
        expect(s.phase, `seed ${seed}`).toBe("human-turn");
        expect(s.legal.length).toBeGreaterThan(0);
      } finally {
        c.dispose();
      }
    }
    expect(phases).toEqual(new Set(["human-turn", "computer-turn"]));
  });

  test("a failed creation throws the engine's error and leaks nothing", () => {
    const { factory, counters } = instrumentedFactory();
    expect(() => GameController.create(config("computer", "nope", "rush", 1), realDeps({}, factory))).toThrow(EngineError);
    expect(counters.frees).toBe(counters.creations);
  });

  test("the snapshot carries the identity facts", () => {
    const c = GameController.create(config("computer", "rush", "bulwark", 77), realDeps());
    try {
      const s = c.getSnapshot();
      expect(s.mode).toBe("computer");
      expect(s.seed).toBe(77);
      expect(s.dataVersion).toBe("starter@1.0.0");
      expect(s.players.One).toEqual({ id: "rush", name: "Ruée" });
      expect(s.players.Two).toEqual({ id: "bulwark", name: "Rempart" });
    } finally {
      c.dispose();
    }
  });
});

describe("snapshots", () => {
  test("immutable, a new identity at each publication, the same one between publications", () => {
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps());
    try {
      c.start();
      const a = c.getSnapshot();
      expect(c.getSnapshot()).toBe(a);
      expect(Object.isFrozen(a)).toBe(true);
      const { snapshots } = watch(c);
      expect(c.submit(a.legal[0])).toEqual({ ok: true });
      expect(snapshots.length).toBeGreaterThan(0);
      expect(c.getSnapshot()).not.toBe(a);
      expect(a.records).toHaveLength(1);
    } finally {
      c.dispose();
    }
  });

  test("unsubscribe stops notifications", () => {
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps());
    try {
      c.start();
      const fn = vi.fn();
      const off = c.subscribe(fn);
      off();
      c.submit(c.getSnapshot().legal[0]);
      expect(fn).not.toHaveBeenCalled();
    } finally {
      c.dispose();
    }
  });
});

describe("submit", () => {
  test("a preview of the current snapshot is applied and recorded as human", () => {
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3));
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps({ createSession: () => session }));
    try {
      c.start();
      const before = c.getSnapshot();
      const res = c.submit(before.legal[0]);
      expect(res).toEqual({ ok: true });
      expect(probe.calls.apply).toBe(1);
      const s = c.getSnapshot();
      expect(s.records).toHaveLength(2);
      expect(s.records[1]).toMatchObject({ seq: 1, by: "human", actor: "One" });
      expect(s.error).toBeNull();
    } finally {
      c.dispose();
    }
  });

  test("a stale preview (double tap, late render) is ignored silently: no engine call, no error banner", () => {
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3));
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps({ createSession: () => session }));
    try {
      c.start();
      const old = c.getSnapshot();
      expect(c.submit(old.legal[0])).toEqual({ ok: true });
      const applied = probe.calls.apply;
      const errorBefore = c.getSnapshot().error;
      expect(c.submit(old.legal[0])).toEqual({ ok: false, reason: "stale" });
      expect(probe.calls.apply).toBe(applied);
      expect(c.getSnapshot().error).toBe(errorBefore);
      // An equal-looking copy is not the identity of a preview of the current snapshot either.
      const cur = c.getSnapshot().legal[0];
      expect(c.submit(structuredClone(cur))).toEqual({ ok: false, reason: "stale" });
      expect(probe.calls.apply).toBe(applied);
    } finally {
      c.dispose();
    }
  });

  test("outside the human turn: not-your-turn, no engine call", async () => {
    const { session, probe } = decorateSession(newSession("bulwark", "rush", 3));
    const gate: { release: () => void } = { release: () => undefined };
    const blocked = new Promise<void>((r) => (gate.release = r));
    const c = GameController.create(config("computer", "bulwark", "rush", 3), realDeps({ createSession: () => session, scheduler: { yield: () => blocked } }));
    try {
      c.start();
      expect(c.getSnapshot().phase).toBe("computer-turn");
      const fake: ActionPreview = { action: { play: { card: 1 } }, cost: 1, hands_over: false, budget_after: 1, ends_game: null };
      expect(c.submit(fake)).toEqual({ ok: false, reason: "not-your-turn" });
      expect(c.concede()).toEqual({ ok: false, reason: "not-your-turn" });
      expect(probe.calls.apply).toBe(0);
    } finally {
      c.dispose();
      gate.release();
    }
  });

  test("the safety net: an illegal action reaching the list is refused by the engine and changes nothing", () => {
    const illegal: ActionPreview = { action: { play: { card: 999999 } }, cost: 1, hands_over: false, budget_after: 1, ends_game: null };
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3), { injectPreview: illegal });
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps({ createSession: () => session }));
    try {
      c.start();
      const before = c.getSnapshot();
      const injected = before.legal.find((p) => JSON.stringify(p.action) === JSON.stringify(illegal.action))!;
      expect(injected).toBeDefined();
      const res = c.submit(injected);
      expect(res.ok).toBe(false);
      if (res.ok) throw new Error("unreachable");
      expect(res.reason).toBe("refused");
      expect(res.error?.code).toBe("card_not_in_hand");
      const after = c.getSnapshot();
      expect(after.error?.code).toBe("card_not_in_hand");
      expect(after.phase).toBe(before.phase);
      expect(after.views).toEqual(before.views);
      expect(after.legal).toEqual(before.legal);
      expect(after.records).toEqual(before.records);
      expect(probe.calls.apply).toBe(1);
      c.clearError();
      expect(c.getSnapshot().error).toBeNull();
      expect(c.getSnapshot().views).toEqual(before.views);
      // The game goes on normally.
      const ok = c.getSnapshot().legal.find((p) => p !== injected && p.action !== "concede")!;
      expect(c.submit(ok)).toEqual({ ok: true });
    } finally {
      c.dispose();
    }
  });
});

describe("concede and the end of the game", () => {
  test("concede() ends the game: phase over and the view says concede", () => {
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps());
    try {
      c.start();
      expect(c.concede()).toEqual({ ok: true });
      const s = c.getSnapshot();
      expect(s.phase).toBe("over");
      expect(s.views.One?.result).toEqual({ winner: "Two", reason: "concede" });
      expect(s.legal).toEqual([]);
      expect(c.submit({ action: "concede", cost: null, hands_over: false, budget_after: null, ends_game: null })).toEqual({
        ok: false,
        reason: "not-your-turn",
      });
    } finally {
      c.dispose();
    }
  });

  test("the end of the game is never deduced from the events: a view forced to result null keeps human-turn", () => {
    const { session } = decorateSession(newSession("rush", "bulwark", 3), { forceView: (_s, v) => ({ ...v, result: null }) });
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps({ createSession: () => session }));
    try {
      c.start();
      expect(c.concede()).toEqual({ ok: true });
      const s = c.getSnapshot();
      expect(s.records[s.records.length - 1].events.some((e) => "game_won" in e)).toBe(true);
      expect(s.phase).toBe("human-turn");
    } finally {
      c.dispose();
    }
  });
});

describe("legalActionsWithTime budget", () => {
  test("never during the computer's turn, at most once per human publication, never on a plain read", async () => {
    for (const [one, two] of PAIRS) {
      const { session, probe } = decorateSession(newSession(one, two, 5));
      const c = GameController.create(config("computer", one, two, 5), realDeps({ createSession: () => session }));
      try {
        const { snapshots } = watch(c);
        c.start();
        const { snapshot } = await playHuman(c, randomPreview(5));
        for (let i = 0; i < 10; i++) c.getSnapshot();
        expect(snapshot.phase).toBe("over");
        expect(probe.timedCalls.every((t) => t.active === "One")).toBe(true);
        const humanTurnPublications = new Set([c.getSnapshot(), ...snapshots].filter((s) => s.phase === "human-turn" && s.legal.length > 0));
        expect(probe.calls.legalActionsWithTime).toBeLessThanOrEqual(humanTurnPublications.size);
        expect(probe.calls.legalActionsWithTime).toBeGreaterThan(0);
      } finally {
        c.dispose();
      }
    }
  });
});

describe("dispose", () => {
  test("N games make N free(), and nothing reaches the WASM afterwards", async () => {
    const { factory, counters } = instrumentedFactory();
    for (let seed = 0; seed < 12; seed++) {
      const c = GameController.create(config("computer", "rush", "bulwark", seed), realDeps({}, factory));
      c.start();
      c.submit(c.getSnapshot().legal[0]);
      await settle(c);
      c.dispose();
      c.dispose();
      expect(c.submit(c.getSnapshot().legal[0] ?? { action: "concede", cost: null, hands_over: false, budget_after: null, ends_game: null })).toMatchObject({
        ok: false,
      });
    }
    expect(counters.creations).toBe(12);
    expect(counters.frees).toBe(12);
    expect(counters.callsAfterFree).toBe(0);
  });

  test("submit after dispose says disposed, publishes nothing, drops subscribers", () => {
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps());
    c.start();
    const fn = vi.fn();
    c.subscribe(fn);
    const preview = c.getSnapshot().legal[0];
    c.dispose();
    expect(c.submit(preview)).toEqual({ ok: false, reason: "disposed" });
    expect(c.concede()).toEqual({ ok: false, reason: "disposed" });
    expect(fn).not.toHaveBeenCalled();
    c.start();
    c.clearError();
    expect(fn).not.toHaveBeenCalled();
  });

  test("dispose in the middle of the computer's chain: no apply, no legalActions, no publication afterwards", async () => {
    const { session, probe } = decorateSession(newSession("bulwark", "rush", 3));
    let clock = 0;
    const gates: (() => void)[] = [];
    const scheduler = { yield: () => new Promise<void>((r) => gates.push(r)) };
    const c = GameController.create(
      config("computer", "bulwark", "rush", 3),
      realDeps({ createSession: () => session, scheduler, now: () => (clock += 20) }),
    );
    const fn = vi.fn();
    c.subscribe(fn);
    c.start();
    // Release the first yield: the chain plays one action (the slow clock cuts a slice right
    // after it) and waits at the next yield, which stays pending while we dispose.
    await Promise.resolve();
    gates.splice(0).forEach((g) => g());
    for (let i = 0; i < 5; i++) await Promise.resolve();
    expect(gates).toHaveLength(1);
    expect(probe.calls.apply).toBeGreaterThanOrEqual(1);
    expect(c.getSnapshot().phase).toBe("computer-turn");
    const applied = probe.calls.apply;
    const listed = probe.calls.legalActions;
    const published = fn.mock.calls.length;
    c.dispose();
    for (let i = 0; i < 5; i++) {
      gates.splice(0).forEach((g) => g());
      await Promise.resolve();
      await Promise.resolve();
    }
    expect(probe.calls.apply).toBe(applied);
    expect(probe.calls.legalActions).toBe(listed);
    expect(fn.mock.calls.length).toBe(published);
    expect(probe.calls.dispose).toBe(1);
  });
});

describe("determinism", () => {
  const run = async (seed: number) => {
    const c = GameController.create(config("computer", "bulwark", "rush", seed), realDeps());
    try {
      c.start();
      const { snapshot } = await playHuman(c, randomPreview(seed + 1000));
      return JSON.stringify(snapshot.records);
    } finally {
      c.dispose();
    }
  };

  test("same seed and same human choices: identical records, the computer included", async () => {
    expect(await run(11)).toBe(await run(11));
    expect((await run(11)).includes('"by":"computer"')).toBe(true);
  });

  test("another seed: a different game", async () => {
    expect(await run(11)).not.toBe(await run(12));
  });

  test("the computer's seed is derived from the game seed", () => {
    const c = GameController.create(config("computer", "rush", "bulwark", 42), realDeps());
    try {
      expect(c.getSnapshot().aiSeed).toBe(deriveAiSeed(42));
      expect(c.getSnapshot().aiSeed).not.toBe(42);
    } finally {
      c.dispose();
    }
  });
});

describe("a fatal engine failure (carry-over of the #62 review)", () => {
  test("a RuntimeError during the human's apply: phase fatal, then no call reaches the WASM again", () => {
    const { factory, calls } = faultyFactory("apply");
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps({}, factory));
    c.start();
    const preview = c.getSnapshot().legal[0];
    const res = c.submit(preview);
    expect(res.ok).toBe(false);
    expect(c.getSnapshot().phase).toBe("fatal");
    expect(c.getSnapshot().error?.code).toBe("internal_error");
    expect(c.getSnapshot().legal).toEqual([]);
    const reached = calls.length;
    expect(c.submit(preview)).toMatchObject({ ok: false, reason: "not-your-turn" });
    expect(c.concede()).toMatchObject({ ok: false });
    expect(calls.length).toBe(reached);
    expect(() => c.dispose()).not.toThrow();
    expect(calls.filter((x) => x === "free")).toHaveLength(1);
  });

  test("a RuntimeError during the computer's chain is fatal too, free() throwing included", async () => {
    const { factory, calls } = faultyFactory("apply", { failFrom: 2, freeThrows: true });
    const c = GameController.create(config("computer", "bulwark", "rush", 3), realDeps({}, factory));
    c.start();
    const s = await settle(c);
    expect(s.phase).toBe("fatal");
    expect(s.error?.code).toBe("internal_error");
    const reached = calls.length;
    expect(c.concede()).toMatchObject({ ok: false });
    expect(calls.length).toBe(reached);
    expect(() => c.dispose()).not.toThrow();
  });

  test("a RuntimeError while reading the legal list after an action is fatal", () => {
    const { factory } = faultyFactory("legal_actions_with_time", { failFrom: 2 });
    const c = GameController.create(config("computer", "rush", "bulwark", 3), realDeps({}, factory));
    c.start();
    c.submit(c.getSnapshot().legal[0]);
    expect(c.getSnapshot().phase).toBe("fatal");
    c.dispose();
  });
});

describe("two camps", () => {
  test("no policy is ever called, both views are present, nothing is the computer's", async () => {
    const policy = vi.fn(stallPolicy);
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 5));
    const c = GameController.create(config("two-camps", "rush", "bulwark", 5), realDeps({ createSession: () => session, policy }));
    try {
      c.start();
      const s0 = c.getSnapshot();
      expect(s0.humanSeat).toBeNull();
      expect(s0.aiSeed).toBeNull();
      expect(s0.views.One).toBeDefined();
      expect(s0.views.Two).toBeDefined();
      expect(s0.phase).toBe("human-turn");
      const { snapshot } = await playHuman(c, randomPreview(5));
      expect(snapshot.phase).toBe("over");
      expect(policy).not.toHaveBeenCalled();
      expect(snapshot.records.every((r) => r.by !== "computer")).toBe(true);
      expect(probe.calls.legalActions).toBe(0);
      const seats = new Set(probe.timedCalls.map((t) => t.active));
      expect(seats).toEqual(new Set<Seat>(["One", "Two"]));
    } finally {
      c.dispose();
    }
  });

  test("hero_down at random", async () => {
    const c = GameController.create(config("two-camps", "rush", "bulwark", 1), realDeps());
    try {
      c.start();
      const { snapshot } = await playHuman(c, randomPreview(1));
      expect(snapshot.views.One?.result?.reason).toBe("hero_down");
      expect(snapshot.views.Two?.result).toEqual(snapshot.views.One?.result);
    } finally {
      c.dispose();
    }
  });

  test("exhaustion, pinned: rush vs bulwark, seed 2, both camps wear down -> exhaustion after 220 actions", async () => {
    const c = GameController.create(config("two-camps", "rush", "bulwark", 2), realDeps());
    try {
      c.start();
      const { snapshot, humanActions } = await playHuman(c, stallPreview);
      expect(snapshot.phase).toBe("over");
      expect(snapshot.views.One?.result?.reason).toBe("exhaustion");
      expect(humanActions).toBe(220);
      expect(snapshot.records).toHaveLength(221);
    } finally {
      c.dispose();
    }
  });

  test("concede by the camp that holds the main", async () => {
    const c = GameController.create(config("two-camps", "rush", "bulwark", 5), realDeps());
    try {
      c.start();
      const active = c.getSnapshot().views.One!.active;
      expect(c.concede()).toEqual({ ok: true });
      const s = c.getSnapshot();
      expect(s.phase).toBe("over");
      expect(s.views.One?.result).toEqual({ winner: active === "One" ? "Two" : "One", reason: "concede" });
    } finally {
      c.dispose();
    }
  });
});

describe("computer mode", () => {
  test("humanSeat One, and the same pinned exhaustion with human and computer both wearing down", async () => {
    const c = GameController.create(config("computer", "rush", "bulwark", 2), realDeps({ policy: stallPolicy }));
    try {
      expect(c.getSnapshot().humanSeat).toBe("One");
      c.start();
      const { snapshot } = await playHuman(c, stallPreview);
      expect(snapshot.phase).toBe("over");
      expect(snapshot.views.One?.result?.reason).toBe("exhaustion");
      expect(snapshot.records).toHaveLength(221);
      expect(snapshot.views.Two).toBeUndefined();
    } finally {
      c.dispose();
    }
  });
});

describe("the driver on engine doubles", () => {
  test("only concede left: computer_stuck, phase fatal, nothing applied", async () => {
    const { session, state } = fakeSession({ activeAfter: () => "Two", legal: () => ["concede"] });
    const c = GameController.create(config("computer", "rush", "bulwark", 1), realDeps({ createSession: () => session }));
    c.start();
    const s = await settle(c);
    expect(s.phase).toBe("fatal");
    expect(s.error?.code).toBe("computer_stuck");
    expect(state.applied).toBe(0);
    c.dispose();
  });

  test("a game that never ends: computer_chain_limit after exactly 1000 actions, no silent truncation", async () => {
    const { session, state } = fakeSession({ activeAfter: () => "Two" });
    const c = GameController.create(config("computer", "rush", "bulwark", 1), realDeps({ createSession: () => session }));
    c.start();
    const s = await settle(c);
    expect(s.phase).toBe("fatal");
    expect(s.error?.code).toBe("computer_chain_limit");
    expect(state.applied).toBe(1000);
    expect(s.computerSteps).toBe(1000);
    c.dispose();
  });

  test.each([7, 9, 10, 30])("slices: with 5 ms per action a chain of %i actions yields about every third action", async (n) => {
    const { session } = fakeSession({ activeAfter: (applied) => (applied < n ? "Two" : "One") });
    let clock = 0;
    const yields = vi.fn(() => Promise.resolve());
    const c = GameController.create(
      config("computer", "rush", "bulwark", 1),
      realDeps({ createSession: () => session, scheduler: { yield: yields }, now: () => (clock += 5) }),
    );
    const { snapshots } = watch(c);
    c.start();
    const s = await settle(c);
    expect(s.phase).toBe("human-turn");
    expect(s.computerSteps).toBe(n);
    expect(yields.mock.calls.length).toBeGreaterThanOrEqual(Math.floor(n / 3) - 1);
    expect(yields.mock.calls.length).toBeLessThanOrEqual(Math.ceil(n / 3));
    expect(snapshots.length).toBeGreaterThanOrEqual(yields.mock.calls.length);
    c.dispose();
  });

  test("without a slow clock, no slice is cut", async () => {
    const { session } = fakeSession({ activeAfter: (applied) => (applied < 30 ? "Two" : "One") });
    const yields = vi.fn(() => Promise.resolve());
    const c = GameController.create(config("computer", "rush", "bulwark", 1), realDeps({ createSession: () => session, scheduler: { yield: yields } }));
    c.start();
    await settle(c);
    expect(yields.mock.calls.length).toBeLessThanOrEqual(1);
    c.dispose();
  });

  test("concede is never played even if the policy asks for the head of the raw list", async () => {
    const { session, state } = fakeSession({
      activeAfter: (applied) => (applied < 2 ? "Two" : "One"),
      legal: () => ["concede", { play: { card: 1 } }],
    });
    const c = GameController.create(config("computer", "rush", "bulwark", 1), realDeps({ createSession: () => session, policy: () => 0 }));
    c.start();
    const s = await settle(c);
    expect(s.phase).toBe("human-turn");
    expect(state.applied).toBe(2);
    expect(s.records.every((r) => r.events.every((e) => !("game_won" in e)))).toBe(true);
    c.dispose();
  });
});

describe("what the controller writes into its records (mutations 2, 12c, 13a)", () => {
  test("records are completed from the views taken after each action, computer's steps included", async () => {
    let computerPlays = 0;
    let deaths = 0;
    for (const [one, two] of PAIRS) {
      const c = GameController.create(config("computer", one, two, 6), realDeps());
      try {
        c.start();
        const { snapshot } = await playHuman(c, randomPreview(6));
        for (const r of snapshot.records) {
          for (const e of r.events) {
            if ("played" in e) {
              if (e.played.player === "Two") computerPlays += 1;
              expect(typeof r.cards[e.played.card]?.name, `${one}/${two} seq ${r.seq}`).toBe("string");
            }
            if ("died" in e) {
              deaths += 1;
              expect(r.cards[e.died.creature]?.health, `${one}/${two} seq ${r.seq}`).toBe(0);
            }
          }
        }
      } finally {
        c.dispose();
      }
    }
    expect(computerPlays).toBeGreaterThan(0);
    expect(deaths).toBeGreaterThan(0);
  });

  test("dispose while the very first yield is pending: nothing is applied once it resolves", async () => {
    const { session, probe } = decorateSession(newSession("bulwark", "rush", 3));
    const gates: (() => void)[] = [];
    const c = GameController.create(
      config("computer", "bulwark", "rush", 3),
      realDeps({ createSession: () => session, scheduler: { yield: () => new Promise<void>((r) => gates.push(r)) } }),
    );
    c.start();
    expect(gates).toHaveLength(1);
    c.dispose();
    gates.splice(0).forEach((g) => g());
    for (let i = 0; i < 5; i++) await Promise.resolve();
    expect(probe.calls.apply).toBe(0);
    expect(probe.calls.legalActions).toBe(0);
  });

  test("the computer draws its randomness from mulberry32(deriveAiSeed(seed)), one draw per action", async () => {
    const seed = 21;
    const expected = mulberry32(deriveAiSeed(seed));
    const draws: number[] = [];
    const reference: number[] = [];
    const c = GameController.create(
      config("computer", "bulwark", "rush", seed),
      realDeps({
        policy: (actions, rng) => {
          const got = rng();
          draws.push(got);
          reference.push(expected());
          return Math.floor(got * actions.length);
        },
      }),
    );
    try {
      c.start();
      await playHuman(c, randomPreview(seed));
      expect(draws.length).toBeGreaterThan(5);
      expect(draws).toEqual(reference);
    } finally {
      c.dispose();
    }
  });
});

export { immediateScheduler };
