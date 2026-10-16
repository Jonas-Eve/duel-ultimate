import type { GameFactory, WasmGameLike } from "../../src/engine/session";
import { testFactory } from "./engine";

export type Method = "start_events" | "view" | "legal_actions" | "legal_actions_with_time" | "apply";

export interface FaultyOptions {
  thrower?: () => never;
  freeThrows?: boolean;
  /** Fail from the n-th call (1-based) of `failing` instead of the first. */
  failFrom?: number;
}

/** The real game, except that `failing` throws a non-JSON RuntimeError (or whatever `thrower` says). */
export function faultyFactory(failing: Method, opts: FaultyOptions = {}) {
  const calls: string[] = [];
  let failingCalls = 0;
  const thrower =
    opts.thrower ??
    (() => {
      throw new WebAssembly.RuntimeError("unreachable");
    });
  const factory: GameFactory = (...args) => {
    const game = testFactory(...args);
    const wrap =
      <A extends unknown[]>(name: Method, fn: (...a: A) => string) =>
      (...a: A): string => {
        calls.push(name);
        if (name === failing) {
          failingCalls += 1;
          if (failingCalls >= (opts.failFrom ?? 1)) thrower();
        }
        return fn(...a);
      };
    const wrapped: WasmGameLike = {
      start_events: wrap("start_events", () => game.start_events()),
      view: wrap("view", (seat: string) => game.view(seat)),
      legal_actions: wrap("legal_actions", () => game.legal_actions()),
      legal_actions_with_time: wrap("legal_actions_with_time", () => game.legal_actions_with_time()),
      apply: wrap("apply", (a: string) => game.apply(a)),
      free: () => {
        calls.push("free");
        game.free();
        if (opts.freeThrows) throw new Error("null pointer passed to rust");
      },
    };
    return wrapped;
  };
  return { factory, calls };
}
