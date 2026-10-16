// Starting a game from the form's inputs (stub: the implementation follows the red tests).
import type { EngineError } from "../engine/errors";
import type { DeckRef, GameController, ControllerDeps, Mode } from "./controller";

export interface StartInput {
  mode: Mode;
  deckOne: DeckRef;
  deckTwo: DeckRef;
  /** What the player typed; empty means "draw one for me". */
  seedText: string;
}
export type StartResult =
  | { ok: true; controller: GameController }
  | { ok: false; reason: "entropy_unavailable" | "invalid_seed" | "engine"; error: EngineError | null };

export function startGame(
  _input: StartInput,
  _deps: Partial<ControllerDeps> & Pick<ControllerDeps, "createSession">,
  _crypto?: Pick<Crypto, "getRandomValues"> | null,
): StartResult {
  throw new Error("not implemented");
}
