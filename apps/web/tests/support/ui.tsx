// Shared helpers for the interface tests: real WASM engine, a controller built by the test
// (as `App` builds it, in an event handler) and rendered in `GameScreen`.
import { act, cleanup, render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, vi } from "vitest";
import type { GameFactory } from "../../src/engine/session";
import { GameController, type ControllerDeps } from "../../src/game/controller";
import { GameScreen } from "../../src/ui/GameScreen";
import { NAMES, config } from "./config";
import { initTestEngine } from "./engine";
import { realDeps } from "./policies";

export { NAMES, config };

const controllers: GameController[] = [];

/** Loads the real engine once per file; cleans the DOM and frees every controller after each test. */
export function useRealEngine(): void {
  beforeAll(async () => {
    await initTestEngine();
  });
  afterEach(() => {
    cleanup();
    for (const c of controllers.splice(0)) c.dispose();
  });
}

/** A `crypto` double: each call to `getRandomValues` writes the next value (the last one repeats). */
export function fakeCrypto(...values: number[]) {
  let i = 0;
  const getRandomValues = vi.fn(<T extends ArrayBufferView | null>(array: T): T => {
    (array as unknown as Uint32Array)[0] = values[Math.min(i, values.length - 1)];
    i += 1;
    return array;
  });
  return { getRandomValues: getRandomValues as unknown as Crypto["getRandomValues"], spy: getRandomValues };
}

export function makeController(
  one: string,
  two: string,
  seed: number,
  over: Partial<ControllerDeps> = {},
  factory?: GameFactory,
): GameController {
  const c = GameController.create(config(one, two, seed), realDeps(over, factory));
  controllers.push(c);
  return c;
}

/** Two camps: one person plays both seats, no computer. */
export function makeCampsController(one: string, two: string, seed: number, over: Partial<ControllerDeps> = {}, factory?: GameFactory): GameController {
  const c = GameController.create({ ...config(one, two, seed), mode: "two-camps" }, realDeps(over, factory));
  controllers.push(c);
  return c;
}

/** Renders `GameScreen` on a controller built here and starts it, like `App` does. */
export function mountGame(c: GameController) {
  const onReplaySameSeed = vi.fn();
  const onReplayNewSeed = vi.fn();
  const onHome = vi.fn();
  const utils = render(<GameScreen controller={c} onReplaySameSeed={onReplaySameSeed} onReplayNewSeed={onReplayNewSeed} onHome={onHome} />);
  act(() => c.start());
  return { ...utils, user: userEvent.setup(), onReplaySameSeed, onReplayNewSeed, onHome };
}

/** Runs controller-driven steps inside `act` so React sees every publication. */
export async function inAct<T>(fn: () => Promise<T>): Promise<T> {
  let out: T | undefined;
  await act(async () => {
    out = await fn();
  });
  return out as T;
}
