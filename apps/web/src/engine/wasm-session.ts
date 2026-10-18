import { EngineError, SessionDisposedError, parseEngineError } from "./errors";
import type { GameFactory, GameSession, SessionConfig, WasmGameLike } from "./session";
import type { Action, ActionPreview, EngineEvent, PlayerView, Seat } from "./types";

const U32_MAX = 4294967295;
/**
 * The computer's policy, a closed name of the binding (never chosen by `game/` or `ui/`): the
 * engine's greedy policy, the same code as the simulator (#126). It ignores `draw`, which is still
 * supplied and validated (contract of the binding, `"random"`, possible noise later).
 */
const COMPUTER_POLICY = "greedy";

/** Runs a WASM call and turns whatever it throws into an `EngineError`. */
function call<T>(fn: () => T): T {
  try {
    return fn();
  } catch (e) {
    throw parseEngineError(e);
  }
}

export function createWasmSession(factory: GameFactory, config: SessionConfig): GameSession {
  // The real gate is `parseSeed`; the binding would silently reduce a value outside u32.
  if (!Number.isInteger(config.seed) || config.seed < 0 || config.seed > U32_MAX) {
    throw new EngineError("invalid_seed", `seed ${String(config.seed)} is not a u32`);
  }
  const versionsJson = JSON.stringify([config.dataVersion]);
  const game: WasmGameLike = call(() =>
    factory(config.cards, config.decks, config.deckOne, config.deckTwo, versionsJson, config.seed),
  );
  let disposed = false;
  // Set by the first fatal error (a non-JSON throw: a Rust panic is a WebAssembly
  // RuntimeError that leaves the instance unusable). From then on nothing reaches the WASM.
  let poisoned: EngineError | null = null;
  const release = () => {
    if (disposed) return;
    disposed = true;
    try {
      game.free();
    } catch {
      // A broken instance may refuse to be freed; this must never mask the error that
      // made us release it, nor make `dispose()` throw.
    }
  };

  let startEvents: EngineEvent[];
  try {
    startEvents = call(() => JSON.parse(game.start_events()) as EngineEvent[]);
  } catch (e) {
    release();
    throw e;
  }

  const live = <T>(fn: () => T): T => {
    if (disposed) throw new SessionDisposedError();
    if (poisoned !== null) {
      throw new EngineError("internal_error", `the game session is dead: ${poisoned.message}`);
    }
    try {
      return call(fn);
    } catch (e) {
      if (e instanceof EngineError && e.fatal) poisoned = e;
      throw e;
    }
  };

  return {
    startEvents,
    legalActions: () => live(() => JSON.parse(game.legal_actions()) as Action[]),
    legalActionsWithTime: () => live(() => JSON.parse(game.legal_actions_with_time()) as ActionPreview[]),
    computerAction: (seat: Seat, draw: number) => {
      // The binding would silently reduce a value outside u32: refuse it here, like the seed.
      if (!Number.isInteger(draw) || draw < 0 || draw > U32_MAX) {
        throw new EngineError("internal_error", `draw ${String(draw)} is not a u32`);
      }
      return live(() => JSON.parse(game.computer_action(seat, COMPUTER_POLICY, draw)) as Action);
    },
    view: (seat: Seat) => live(() => JSON.parse(game.view(seat)) as PlayerView),
    apply: (action: Action) => live(() => JSON.parse(game.apply(JSON.stringify(action))) as EngineEvent[]),
    dispose: release,
  };
}
