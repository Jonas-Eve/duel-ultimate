import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { CARDS_JSON, DATA_VERSION, DECKS_JSON } from "../../src/data/starter";
import { createWasmSession } from "../../src/engine/wasm-session";
import type { GameFactory, GameSession, SessionConfig, WasmGameLike } from "../../src/engine/session";
import type { Action } from "../../src/engine/types";
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
