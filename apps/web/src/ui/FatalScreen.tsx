import { useEffect, useRef } from "react";
import type { Snapshot } from "../game/controller";
import { messageFor } from "./errors.fr";
import { fr } from "./fr";

interface FatalScreenProps {
  snapshot: Snapshot;
  onReplaySameSeed(): void;
  onHome(): void;
}

/** The game cannot go on (dead engine instance, computer stuck...): no control of the game, only the exits. */
export function FatalScreen({ snapshot, onReplaySameSeed, onHome }: FatalScreenProps) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    title.current?.focus();
  }, []);
  return (
    <section className="screen" aria-labelledby="fatal-title">
      <h2 id="fatal-title" ref={title} tabIndex={-1}>
        {fr.fatalTitle}
      </h2>
      <p role="alert">{messageFor(snapshot.error?.code ?? "internal_error")}</p>
      <div className="stack">
        <button type="button" className="btn btn-primary" onClick={onReplaySameSeed}>
          {fr.replaySame}
        </button>
        <button type="button" className="btn" onClick={onHome}>
          {fr.backHome}
        </button>
      </div>
    </section>
  );
}
