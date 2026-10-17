// The computer player's limits. Pure, no React. The choice of the action is the engine's
// (`GameSession.computerAction`, one `draw` per action: no policy and no rule lives here); the
// driver loop itself lives in the controller (it publishes snapshots and owns the cancellation
// token).

/** Safety net, never reached by construction (the engine's potential strictly decreases, <= 566 actions with the starter set). */
export const MAX_COMPUTER_ACTIONS_PER_CHAIN = 1000;
/** A chain of computer actions publishes a snapshot and yields to the browser after this long. */
export const COMPUTER_SLICE_MS = 12;
/** No artificial delay between the computer's actions (may be raised later). */
export const COMPUTER_STEP_DELAY_MS = 0;
