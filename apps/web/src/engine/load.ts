// The only place (with `wasm-session.ts` for types) that touches the generated package.
import { EngineError } from "./errors";
import type { GameFactory } from "./session";

interface WasmModule {
  default(): Promise<unknown>;
  Game: { from_data: GameFactory };
}
export type WasmImporter = () => Promise<WasmModule>;

/**
 * Loads the engine once: one import, one `init()`, a single shared promise. A failure is
 * forgotten so that "Reload" can try again.
 */
export function createEngineLoader(importer: WasmImporter): () => Promise<GameFactory> {
  let pending: Promise<GameFactory> | null = null;
  return () => {
    if (pending === null) {
      const attempt = (async () => {
        try {
          const m = await importer();
          await m.default();
          const factory: GameFactory = (...args) => m.Game.from_data(...args);
          return factory;
        } catch (e) {
          throw new EngineError("wasm_load_failed", e instanceof Error ? e.message : String(e));
        }
      })();
      pending = attempt;
      attempt.catch(() => {
        if (pending === attempt) pending = null;
      });
    }
    return pending;
  };
}

/** Production loader: `init()` fetches the same-origin `.wasm` itself. */
export const loadEngine = createEngineLoader(() => import("../wasm/duel_engine_wasm"));
