// The computer player's policy (stub: the implementation follows the red tests).
import type { Action } from "../engine/types";

export type ComputerPolicy = (actions: readonly Action[], rng: () => number) => number;

export const MAX_COMPUTER_ACTIONS_PER_CHAIN = 1000;
export const COMPUTER_SLICE_MS = 12;
export const COMPUTER_STEP_DELAY_MS = 0;

export const randomPolicy: ComputerPolicy = () => {
  throw new Error("not implemented");
};

export function pickComputerAction(_legal: readonly Action[], _policy: ComputerPolicy, _rng: () => number): Action {
  throw new Error("not implemented");
}
