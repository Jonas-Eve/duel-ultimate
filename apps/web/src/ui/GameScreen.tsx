import { useState, useSyncExternalStore } from "react";
import { isConcede, type ActionPreview } from "../engine/types";
import type { GameController, Phase } from "../game/controller";
import { toScreenModel } from "../game/screen-model";
import { ActionBar } from "./ActionBar";
import { Board } from "./Board";
import { ConfirmDialog } from "./ConfirmDialog";
import { describeConsequence } from "./consequence";
import { EndScreen } from "./EndScreen";
import { ErrorBanner } from "./ErrorBanner";
import { FatalScreen } from "./FatalScreen";
import { fr } from "./fr";
import { Hand } from "./Hand";
import { HeroBar } from "./HeroBar";
import { LogBar } from "./LogBar";
import { LogDrawer } from "./LogDrawer";
import { formatBlocks, formatRecords, logContext } from "./log-format";
import { consequenceLabels, outcomeOf } from "./outcome";
import { ThinkingIndicator } from "./ThinkingIndicator";
import { TimeBand } from "./TimeBand";

interface GameScreenProps {
  /** Created by the caller in an event handler (never in an effect), started by it, disposed by it. */
  controller: GameController;
  onReplaySameSeed(): void;
  onReplayNewSeed(): void;
  onHome(): void;
}

/**
 * The text announced when the computer hands the main back. `computerSteps` is only reset when
 * the NEXT chain starts, so it is stale during later human turns: the message is produced on the
 * transition computer-turn -> human-turn only (never from the bare value), and it goes away as
 * soon as the human acts (the record count moves) or the game ends.
 */
function useChainAnnouncement(phase: Phase, computerSteps: number, recordCount: number): string {
  const [seen, setSeen] = useState(phase);
  const [announced, setAnnounced] = useState<{ message: string; at: number } | null>(null);
  if (seen !== phase) {
    setSeen(phase);
    setAnnounced(seen === "computer-turn" && phase === "human-turn" ? { message: fr.computerPlayed(computerSteps), at: recordCount } : null);
  }
  return announced !== null && announced.at === recordCount ? announced.message : "";
}

export function GameScreen({ controller, onReplaySameSeed, onReplayNewSeed, onHome }: GameScreenProps) {
  const snapshot = useSyncExternalStore(controller.subscribe, controller.getSnapshot);
  const [confirming, setConfirming] = useState(false);
  const [logOpen, setLogOpen] = useState(false);
  const announcement = useChainAnnouncement(snapshot.phase, snapshot.computerSteps, snapshot.records.length);

  if (snapshot.phase === "fatal") {
    return <FatalScreen snapshot={snapshot} onReplaySameSeed={onReplaySameSeed} onHome={onHome} />;
  }
  if (snapshot.phase === "over") {
    return <EndScreen snapshot={snapshot} onReplaySameSeed={onReplaySameSeed} onReplayNewSeed={onReplayNewSeed} onHome={onHome} />;
  }

  const model = toScreenModel(snapshot);
  const labels = consequenceLabels(outcomeOf(snapshot, model));
  const humanTurn = snapshot.phase === "human-turn";
  const ctx = logContext(snapshot);
  const lastLine = formatRecords(snapshot.records.slice(-1), ctx).at(-1);
  const concede = snapshot.legal.find((p) => isConcede(p.action));
  // The interface hands back a preview of the CURRENT snapshot, never an action of its own;
  // a stale one is ignored by the controller (double press), a refused one comes back as `error`.
  const submit = (preview: ActionPreview) => {
    controller.submit(preview);
  };

  return (
    <section className="screen game" aria-label={fr.gameRegion}>
      <div className="info-row">
        <span>{fr.seed(snapshot.seed)}</span>
        <details>
          <summary>{fr.gameInfo}</summary>
          <p>{fr.dataVersion(snapshot.dataVersion)}</p>
          {snapshot.aiSeed !== null && <p>{fr.aiSeed(snapshot.aiSeed)}</p>}
        </details>
      </div>
      {snapshot.error !== null && <ErrorBanner error={snapshot.error} onClose={() => controller.clearError()} />}
      <HeroBar side={model.top} label={fr.opponentHero} />
      <Board cards={model.top.board} label={fr.opponentBoard} />
      {snapshot.phase === "computer-turn" && <ThinkingIndicator />}
      <TimeBand model={model} />
      <Board cards={model.bottom.board} label={fr.yourBoard} />
      <HeroBar side={model.bottom} label={fr.yourHero} />
      <p aria-live="polite" className="announce">
        {announcement}
      </p>
      <div className="log-row">
        <LogBar line={lastLine} live={humanTurn && announcement === ""} />
        <button type="button" className="btn" onClick={() => setLogOpen(true)}>
          {fr.logButton}
        </button>
      </div>
      <Hand cards={model.bottom.hand ?? []} legal={snapshot.legal} enabled={humanTurn} labels={labels} onPlay={submit} />
      <ActionBar
        hero={model.bottom.hero}
        legal={snapshot.legal}
        enabled={humanTurn}
        labels={labels}
        onActivate={submit}
        onConcede={() => setConfirming(true)}
      />
      {logOpen && <LogDrawer blocks={formatBlocks(snapshot.records, ctx)} onClose={() => setLogOpen(false)} />}
      {confirming && concede !== undefined && (
        <ConfirmDialog
          lines={describeConsequence(concede, labels)}
          onCancel={() => setConfirming(false)}
          onConfirm={() => {
            setConfirming(false);
            controller.concede();
          }}
        />
      )}
    </section>
  );
}
