import { useEffect, useRef } from "react";
import type { Snapshot } from "../game/controller";
import { toScreenModel } from "../game/screen-model";
import { endReason, fr } from "./fr";
import { LogHistory } from "./LogHistory";
import { formatBlocks, logContext } from "./log-format";
import { outcomeOf } from "./outcome";

interface EndScreenProps {
  snapshot: Snapshot;
  onReplaySameSeed(): void;
  onReplayNewSeed(): void;
  onHome(): void;
}

/** Winner, reason, seeds and the way out. The full log follows (a plain list, not a live region). Focus lands on the title when it appears. */
export function EndScreen({ snapshot, onReplaySameSeed, onReplayNewSeed, onHome }: EndScreenProps) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    title.current?.focus();
  }, []);

  const result = snapshot.views.One?.result ?? null;
  const outcome = outcomeOf(snapshot, toScreenModel(snapshot));
  return (
    <section className="screen end-screen" aria-labelledby="end-title">
      <h2 id="end-title" ref={title} tabIndex={-1}>
        {result === null ? "" : fr.winnerTitle(result.winner, outcome.you)}
      </h2>
      {result !== null && <p>{endReason(result, outcome)}</p>}
      <p>{fr.seed(snapshot.seed)}</p>
      {snapshot.aiSeed !== null && <p>{fr.aiSeed(snapshot.aiSeed)}</p>}
      <section className="end-log" aria-labelledby="end-log-title">
        <h3 id="end-log-title">{fr.logFull}</h3>
        <LogHistory blocks={formatBlocks(snapshot.records, logContext(snapshot))} level={4} />
      </section>
      <div className="stack">
        <button type="button" className="btn btn-primary" onClick={onReplaySameSeed}>
          {fr.replaySame}
        </button>
        <button type="button" className="btn" onClick={onReplayNewSeed}>
          {fr.replayNew}
        </button>
        <button type="button" className="btn" onClick={onHome}>
          {fr.backHome}
        </button>
      </div>
    </section>
  );
}
