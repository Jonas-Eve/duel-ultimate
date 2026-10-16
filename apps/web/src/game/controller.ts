// The local game controller (stub: the implementation follows the red tests).
import type { EngineError } from "../engine/errors";
import type { GameSession } from "../engine/session";
import type { ActionPreview, PlayerView, Seat } from "../engine/types";
import type { ComputerPolicy } from "./computer";
import type { StepRecord } from "./records";

export type Mode = "computer" | "two-camps";
export interface DeckRef {
  id: string;
  name: string;
}
export interface GameConfig {
  mode: Mode;
  deckOne: DeckRef;
  deckTwo: DeckRef;
  /** Already validated as a u32 by the caller. */
  seed: number;
}
/** What the controller asks the factory for: the caller (App, #64) closes over the starter data. */
export interface SessionRequest {
  deckOne: string;
  deckTwo: string;
  seed: number;
}
export interface ControllerDeps {
  createSession(request: SessionRequest): GameSession;
  policy: ComputerPolicy;
  scheduler: { yield(): Promise<void> };
  now(): number;
}
export type Phase = "human-turn" | "computer-turn" | "over" | "fatal";
export interface Snapshot {
  phase: Phase;
  mode: Mode;
  humanSeat: "One" | null;
  seed: number;
  aiSeed: number | null;
  dataVersion: string;
  players: Record<Seat, DeckRef>;
  views: { One?: PlayerView; Two?: PlayerView };
  legal: ActionPreview[];
  records: StepRecord[];
  error: EngineError | null;
  computerSteps: number;
}
export type SubmitResult =
  | { ok: true }
  | { ok: false; reason: "not-your-turn" | "stale" | "disposed" | "refused"; error?: EngineError };

export class GameController {
  static create(_config: GameConfig, _deps: Partial<ControllerDeps> & Pick<ControllerDeps, "createSession">): GameController {
    throw new Error("not implemented");
  }
  start(): void {
    throw new Error("not implemented");
  }
  getSnapshot(): Snapshot {
    throw new Error("not implemented");
  }
  subscribe(_fn: () => void): () => void {
    throw new Error("not implemented");
  }
  submit(_preview: ActionPreview): SubmitResult {
    throw new Error("not implemented");
  }
  concede(): SubmitResult {
    throw new Error("not implemented");
  }
  clearError(): void {
    throw new Error("not implemented");
  }
  dispose(): void {
    throw new Error("not implemented");
  }
}
