// Every failure the client can meet, as one shape. The binding throws an `Error` whose
// `message` is the JSON `{"code","message"}`; anything else (a Rust panic, a call after
// `free()`, a non-Error value) is an internal error. The English `message` is for logs and
// tests only: the interface shows the French table of `ui/errors.fr.ts`, never this text.

/** Codes produced by the Rust sources (`rules.rs`, `setup.rs`, `data.rs`, `engine-wasm/src/lib.rs`). */
export const ENGINE_CODES = [
  "game_over",
  "card_not_in_hand",
  "unknown_effect",
  "effect_not_applicable",
  "attacker_not_on_board",
  "target_not_on_board",
  "target_forbidden_by_provocation",
  "board_full",
  "invalid_setup",
  "invalid_json",
  "unknown_deck",
  "input_too_large",
  "invalid_seat",
  "not_active_seat",
  "invalid_policy",
  "computer_stuck",
  "serialization_failed",
] as const;

/** Codes produced by the client itself. */
export const CLIENT_CODES = [
  "invalid_seed",
  "wasm_load_failed",
  "computer_chain_limit",
  "internal_error",
] as const;

export type ErrorCode = (typeof ENGINE_CODES)[number] | (typeof CLIENT_CODES)[number];

const FATAL: readonly string[] = ["internal_error", "serialization_failed", "computer_stuck", "computer_chain_limit"];

/**
 * Fatal: the game cannot go on. Rule errors leave the state intact and creation errors mean
 * the game does not exist, so neither is fatal.
 */
export const isFatalCode = (code: string): boolean => FATAL.includes(code);

export class EngineError extends Error {
  readonly code: string;
  readonly fatal: boolean;
  constructor(code: string, message: string) {
    super(message);
    this.name = "EngineError";
    this.code = code;
    this.fatal = isFatalCode(code);
  }
}

/** Thrown by any method of a disposed session, before anything reaches the WASM. */
export class SessionDisposedError extends Error {
  constructor() {
    super("the game session was disposed");
    this.name = "SessionDisposedError";
  }
}

export function parseEngineError(e: unknown): EngineError {
  if (e instanceof EngineError) return e;
  if (e instanceof Error) {
    try {
      const body: unknown = JSON.parse(e.message);
      if (typeof body === "object" && body !== null) {
        const { code, message } = body as { code?: unknown; message?: unknown };
        if (typeof code === "string" && code !== "" && typeof message === "string") {
          return new EngineError(code, message);
        }
      }
    } catch {
      // Not the binding's JSON: falls through to the internal error.
    }
    return new EngineError("internal_error", e.message);
  }
  return new EngineError("internal_error", String(e));
}
