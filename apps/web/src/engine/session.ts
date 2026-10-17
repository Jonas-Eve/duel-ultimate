import type { Action, ActionPreview, EngineEvent, PlayerView, Seat } from "./types";

/** The subset of the generated `Game` the client uses: lets tests instrument the real engine. */
export interface WasmGameLike {
  start_events(): string;
  view(seat: string): string;
  legal_actions(): string;
  legal_actions_with_time(): string;
  computer_action(seat: string, policy: string, draw: number): string;
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
  /** Raw list, engine order. */
  legalActions(): Action[];
  /** Same length and order as `legalActions()` (human turn only: costs a clone per action). */
  legalActionsWithTime(): ActionPreview[];
  /**
   * The action the computer plays for `seat` (the active seat), chosen by the engine from the
   * single `draw` (a u32) this call is given: the same state, seat and draw always give the same
   * action. Never "concede". Throws `EngineError` (`computer_stuck`, fatal, when nothing else is
   * playable). Like `view`, a local adapter: nothing authenticates the seat.
   */
  computerAction(seat: Seat, draw: number): Action;
  view(seat: Seat): PlayerView;
  /** Throws `EngineError`; the state is untouched on error (guaranteed by the engine). */
  apply(action: Action): EngineEvent[];
  /** Frees the game once; idempotent. */
  dispose(): void;
}
