import type { Action, ActionPreview, EngineEvent, PlayerView, Seat } from "./types";

/** The subset of the generated `Game` the client uses: lets tests instrument the real engine. */
export interface WasmGameLike {
  start_events(): string;
  view(seat: string): string;
  legal_actions(): string;
  legal_actions_with_time(): string;
  apply(action_json: string): string;
  free(): void;
}

/** `Game.from_data` in production. `versionsJson` is a JSON string of an array, not an array. */
export type GameFactory = (
  cards: string,
  decks: string,
  deckOne: string,
  deckTwo: string,
  versionsJson: string,
  seed: number,
) => WasmGameLike;

export interface SessionConfig {
  cards: string;
  decks: string;
  deckOne: string;
  deckTwo: string;
  seed: number;
  dataVersion: string;
}

/**
 * The only access to the engine. The client never builds an action: it finds it in
 * `legalActionsWithTime()` and hands it back to `apply` unchanged. Nothing here
 * authenticates a seat (neither does the binding): hiding the computer's hand is a display
 * choice, not a guarantee.
 */
export interface GameSession {
  readonly startEvents: readonly EngineEvent[];
  /** Raw list, engine order (for the computer). */
  legalActions(): Action[];
  /** Same length and order as `legalActions()` (human turn only: costs a clone per action). */
  legalActionsWithTime(): ActionPreview[];
  view(seat: Seat): PlayerView;
  /** Throws `EngineError`; the state is untouched on error (guaranteed by the engine). */
  apply(action: Action): EngineEvent[];
  /** Frees the game once; idempotent. */
  dispose(): void;
}
