import type { GameFactory } from "./session";

export type WasmImporter = () => Promise<{ default(): Promise<unknown>; Game: { from_data: GameFactory } }>;

export function createEngineLoader(_importer: WasmImporter): () => Promise<GameFactory> {
  return () => Promise.reject(new Error("not implemented"));
}

export const loadEngine = createEngineLoader(() => Promise.reject(new Error("not implemented")));
