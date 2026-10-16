import { EngineError, SessionDisposedError, parseEngineError } from "./errors";
import type { GameFactory, GameSession, SessionConfig, WasmGameLike } from "./session";
import type { Action, ActionPreview, EngineEvent, PlayerView, Seat } from "./types";

const U32_MAX = 4294967295;

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
  const release = () => {
    if (disposed) return;
    disposed = true;
    game.free();
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
    return call(fn);
  };

  return {
    startEvents,
    legalActions: () => live(() => JSON.parse(game.legal_actions()) as Action[]),
    legalActionsWithTime: () => live(() => JSON.parse(game.legal_actions_with_time()) as ActionPreview[]),
    view: (seat: Seat) => live(() => JSON.parse(game.view(seat)) as PlayerView),
    apply: (action: Action) => live(() => JSON.parse(game.apply(JSON.stringify(action))) as EngineEvent[]),
    dispose: release,
  };
}
