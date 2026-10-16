// The computer player: a policy and the one function that applies it. Pure, no React.
// The driver loop itself lives in the controller (it publishes snapshots and owns the
// cancellation token); this file only decides WHICH action, and refuses to decide badly.
import { EngineError } from "../engine/errors";
import type { Action } from "../engine/types";

/**
 * Receives the legal actions WITHOUT "concede" (filtered by `pickComputerAction`, the same
 * semantics as the simulator's `random_policy`) and returns an index into that list.
 */
export type ComputerPolicy = (actions: readonly Action[], rng: () => number) => number;

/** Safety net, never reached by construction (the engine's potential strictly decreases, <= 566 actions with the starter set). */
export const MAX_COMPUTER_ACTIONS_PER_CHAIN = 1000;
/** A chain of computer actions publishes a snapshot and yields to the browser after this long. */
export const COMPUTER_SLICE_MS = 12;
/** No artificial delay between the computer's actions (may be raised later). */
export const COMPUTER_STEP_DELAY_MS = 0;

/**
 * Uniform choice. `rng()` is in [0, 1) with a granularity of 2^-32 (mulberry32), so the
 * modulo-free `floor(rng() * n)` is biased by at most n / 2^32 per index (n <= 931 legal
 * actions: <= 2.2e-7), which is negligible for a game opponent.
 */
export const randomPolicy: ComputerPolicy = (actions, rng) => Math.floor(rng() * actions.length);

/**
 * The action the computer plays among the raw legal actions. "concede" is removed first and
 * never reaches the policy; if nothing else is left the computer is stuck (explicit error,
 * never a silent concession); an index out of bounds from the policy is a fatal internal error.
 */
export function pickComputerAction(legal: readonly Action[], policy: ComputerPolicy, rng: () => number): Action {
  const playable = legal.filter((a) => a !== "concede");
  if (playable.length === 0) {
    throw new EngineError("computer_stuck", "the computer has no legal action other than conceding");
  }
  const index = policy(playable, rng);
  if (!Number.isInteger(index) || index < 0 || index >= playable.length) {
    throw new EngineError("internal_error", `the computer policy returned ${String(index)} for ${playable.length} actions`);
  }
  return playable[index];
}
