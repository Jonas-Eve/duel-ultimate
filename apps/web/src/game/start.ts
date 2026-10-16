// Starting a game from the form's inputs. Never throws: every way it can fail is a result the
// screen can show. In particular `randomSeed` throws when `crypto.getRandomValues` is missing
// (it never falls back to Math.random): that becomes `entropy_unavailable`, and the interface
// then offers the manual-seed path (a typed seed needs no entropy).
import { EngineError, parseEngineError } from "../engine/errors";
import { GameController, type ControllerDeps, type DeckRef, type Mode } from "./controller";
import { parseSeed, randomSeed } from "./seed";

export interface StartInput {
  mode: Mode;
  deckOne: DeckRef;
  deckTwo: DeckRef;
  /** What the player typed; empty means "draw one for me". */
  seedText: string;
}
export type StartResult =
  | { ok: true; controller: GameController }
  /** `error` is null for `entropy_unavailable` (no engine error: show "type a seed instead"). */
  | { ok: false; reason: "entropy_unavailable" | "invalid_seed" | "engine"; error: EngineError | null };

export function startGame(
  input: StartInput,
  deps: Partial<ControllerDeps> & Pick<ControllerDeps, "createSession">,
  crypto?: Pick<Crypto, "getRandomValues"> | null,
): StartResult {
  let seed: number;
  const typed = parseSeed(input.seedText);
  if (typed.ok) {
    seed = typed.value;
  } else if (typed.reason === "empty") {
    try {
      seed = randomSeed(crypto);
    } catch {
      return { ok: false, reason: "entropy_unavailable", error: null };
    }
  } else {
    return { ok: false, reason: "invalid_seed", error: new EngineError("invalid_seed", `seed text is ${typed.reason}`) };
  }
  try {
    const controller = GameController.create({ mode: input.mode, deckOne: input.deckOne, deckTwo: input.deckTwo, seed }, deps);
    return { ok: true, controller };
  } catch (e) {
    return { ok: false, reason: "engine", error: parseEngineError(e) };
  }
}
