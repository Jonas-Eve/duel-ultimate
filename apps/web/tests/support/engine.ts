import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { CARDS_JSON, DATA_VERSION, DECKS_JSON } from "../../src/data/starter";
import { createWasmSession } from "../../src/engine/wasm-session";
import type { GameFactory, GameSession, SessionConfig, WasmGameLike } from "../../src/engine/session";
import type { Action, ActionPreview, EngineEvent, PlayerView, Seat } from "../../src/engine/types";
import { mulberry32 } from "../../src/game/seed";
import { webRoot } from "./paths";

type Binding = typeof import("../../src/wasm/duel_engine_wasm");
let binding: Binding | null = null;

/** Loads the real generated package synchronously from the bytes on disk (no fetch under jsdom). */
export async function initTestEngine(): Promise<Binding> {
  if (binding === null) {
    const m = await import("../../src/wasm/duel_engine_wasm");
    m.initSync({ module: readFileSync(resolve(webRoot, "src/wasm/duel_engine_wasm_bg.wasm")) });
    binding = m;
  }
  return binding;
}

/** `Game.from_data` of the real package: call `initTestEngine()` first. */
export const testFactory: GameFactory = (...args) => {
  if (binding === null) throw new Error("call initTestEngine() first");
  return binding.Game.from_data(...args);
};

export interface Counters {
  creations: number;
  frees: number;
  /** Calls reaching the WASM object after its `free()` (must stay 0 for a safe wrapper). */
  callsAfterFree: number;
  /** Seeds the factory received. */
  seeds: number[];
}

/** Wraps the real `Game` (not a mock of the engine) to count `free()` and calls after it. */
export function instrumentedFactory(): { factory: GameFactory; counters: Counters } {
  const counters: Counters = { creations: 0, frees: 0, callsAfterFree: 0, seeds: [] };
  const factory: GameFactory = (...args) => {
    counters.seeds.push(args[5]);
    const game = testFactory(...args);
    counters.creations += 1;
    let freed = false;
    const guard =
      <A extends unknown[], R>(fn: (...a: A) => R) =>
      (...a: A): R => {
        if (freed) counters.callsAfterFree += 1;
        return fn(...a);
      };
    const wrapped: WasmGameLike = {
      start_events: guard(() => game.start_events()),
      view: guard((seat: string) => game.view(seat)),
      legal_actions: guard(() => game.legal_actions()),
      legal_actions_with_time: guard(() => game.legal_actions_with_time()),
      computer_action: guard((seat: string, policy: string, draw: number) => game.computer_action(seat, policy, draw)),
      apply: guard((a: string) => game.apply(a)),
      free: () => {
        if (freed) counters.callsAfterFree += 1;
        freed = true;
        counters.frees += 1;
        game.free();
      },
    };
    return wrapped;
  };
  return { factory, counters };
}

export const starterConfig = (deckOne: string, deckTwo: string, seed: number): SessionConfig => ({
  cards: CARDS_JSON,
  decks: DECKS_JSON,
  deckOne,
  deckTwo,
  seed,
  dataVersion: DATA_VERSION,
});

export function newSession(deckOne: string, deckTwo: string, seed: number, factory: GameFactory = testFactory): GameSession {
  return createWasmSession(factory, starterConfig(deckOne, deckTwo, seed));
}

/** The four ordered pairs of the two starter decks. */
export const PAIRS: [string, string][] = [
  ["rush", "bulwark"],
  ["bulwark", "rush"],
  ["rush", "rush"],
  ["bulwark", "bulwark"],
];

/** Uniform random choice among the legal actions except concede, from a seeded generator. */
export function randomPolicy(seed: number): (actions: Action[]) => Action {
  const rnd = mulberry32(seed);
  return (actions) => {
    const playable = actions.filter((a) => a !== "concede");
    return playable[Math.floor(rnd() * playable.length)];
  };
}

export interface SessionProbe {
  calls: { legalActions: number; legalActionsWithTime: number; computerAction: number; apply: number; view: number; dispose: number };
  /** Every `draw` handed to `computerAction`, in order. */
  draws: number[];
  /** Every action handed to `apply` (the injected illegal preview included). */
  applied: Action[];
  /** For each `legalActionsWithTime` call: who held the main at that instant (read off the real session). */
  timedCalls: { active: Seat }[];
}

export interface DecorateOptions {
  /** Appended to every `legalActionsWithTime()` result: the "safety net" test injects an illegal action. */
  injectPreview?: ActionPreview;
  /** Rewrites what `view` returns (for example `result: null` while the events say the game ended). */
  forceView?: (seat: Seat, view: PlayerView) => PlayerView;
  /** Runs right after each successful `apply`, with the real inner session (reads there are not counted). */
  afterApply?: (inner: GameSession, events: EngineEvent[]) => void;
}

/** Wraps a real `GameSession` (not a mock of the engine): counts calls and can alter what it returns. */
export function decorateSession(inner: GameSession, opts: DecorateOptions = {}): { session: GameSession; probe: SessionProbe } {
  const probe: SessionProbe = {
    calls: { legalActions: 0, legalActionsWithTime: 0, computerAction: 0, apply: 0, view: 0, dispose: 0 },
    draws: [],
    applied: [],
    timedCalls: [],
  };
  const session: GameSession = {
    startEvents: inner.startEvents,
    legalActions: () => {
      probe.calls.legalActions += 1;
      return inner.legalActions();
    },
    legalActionsWithTime: () => {
      probe.calls.legalActionsWithTime += 1;
      probe.timedCalls.push({ active: inner.view("One").active });
      const list = inner.legalActionsWithTime();
      return opts.injectPreview ? [...list, opts.injectPreview] : list;
    },
    computerAction: (seat, draw) => {
      probe.calls.computerAction += 1;
      probe.draws.push(draw);
      return inner.computerAction(seat, draw);
    },
    view: (seat) => {
      probe.calls.view += 1;
      const v = inner.view(seat);
      return opts.forceView ? opts.forceView(seat, v) : v;
    },
    apply: (action) => {
      probe.calls.apply += 1;
      probe.applied.push(action);
      const events = inner.apply(action);
      opts.afterApply?.(inner, events);
      return events;
    },
    dispose: () => {
      probe.calls.dispose += 1;
      inner.dispose();
    },
  };
  return { session, probe };
}
