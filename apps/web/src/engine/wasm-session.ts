import type { GameFactory, GameSession, SessionConfig } from "./session";

export function createWasmSession(_factory: GameFactory, _config: SessionConfig): GameSession {
  throw new Error("not implemented");
}
