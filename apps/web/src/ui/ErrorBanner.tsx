import type { EngineError } from "../engine/errors";
import { messageFor } from "./errors.fr";
import { fr } from "./fr";

/** A refused action: the French text of the code, never the engine's English message. */
export function ErrorBanner({ error, onClose }: { error: EngineError; onClose(): void }) {
  return (
    <div role="alert" className="alert">
      <p>{messageFor(error.code)}</p>
      <button type="button" className="btn" onClick={onClose}>
        {fr.closeError}
      </button>
    </div>
  );
}
