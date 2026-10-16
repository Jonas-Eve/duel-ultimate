import { useEffect, useRef, useState, useSyncExternalStore, type KeyboardEvent } from "react";
import { isAttackBy, isConcede, type ActionPreview, type Target } from "../engine/types";
import type { GameController, Phase } from "../game/controller";
import { toScreenModel } from "../game/screen-model";
import { ActionBar } from "./ActionBar";
import { AttackSheet } from "./AttackSheet";
import { deriveAttackState, previewOn } from "./attack-state";
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
  // The attack flow's local state. Both are only ever read through `deriveAttackState`/`previewOn`:
  // a value no engine preview backs any more is dropped during render (never by an effect).
  const [selected, setSelected] = useState<number | null>(null);
  const [pending, setPending] = useState<Target | null>(null);
  const [focusToken, setFocusToken] = useState(0);
  const region = useRef<HTMLElement>(null);
  // After an attack is confirmed the focus goes back to the game region: the dialog closes and
  // the tile that opened it may be gone. Runs after the dialog's own focus restoration.
  useEffect(() => {
    if (focusToken > 0) region.current?.focus();
  }, [focusToken]);
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

  const attack = deriveAttackState(selected, snapshot.legal);
  if (selected !== null && attack.selected === null) {
    setSelected(null);
    setPending(null);
  }
  const pendingPreview = pending === null ? undefined : previewOn(attack.attacks, pending);
  if (pending !== null && pendingPreview === undefined) setPending(null);
  const clearAttack = () => {
    setSelected(null);
    setPending(null);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key === "Escape" && attack.selected !== null) clearAttack();
  };
  const hasAttack = (id: number) => snapshot.legal.some((p) => isAttackBy(id)(p.action));
  const attackerName = model.bottom.board.find((c) => c.id === attack.selected)?.name ?? fr.unknownCard;
  const targetName = (t: Target) =>
    t === "hero" ? fr.heroAsTarget(model.top.hero.name) : (model.top.board.find((c) => c.id === t.creature)?.name ?? fr.unknownCard);

  return (
    <section ref={region} tabIndex={-1} className="screen game" aria-label={fr.gameRegion} onKeyDown={onKeyDown}>
      <div className="info-row">
        <span>{fr.seed(snapshot.seed)}</span>
        <details>
          <summary>{fr.gameInfo}</summary>
          <p>{fr.dataVersion(snapshot.dataVersion)}</p>
          {snapshot.aiSeed !== null && <p>{fr.aiSeed(snapshot.aiSeed)}</p>}
        </details>
      </div>
      {snapshot.error !== null && <ErrorBanner error={snapshot.error} onClose={() => controller.clearError()} />}
      <HeroBar
        side={model.top}
        label={fr.opponentHero}
        target={attack.selected === null ? undefined : { possible: attack.heroTargetable, onPress: () => setPending("hero") }}
      />
      <Board
        cards={model.top.board}
        label={fr.opponentBoard}
        tileProps={
          attack.selected === null
            ? undefined
            : (card) =>
                attack.creatureTargets.has(card.id)
                  ? { onPress: () => setPending({ creature: card.id }), mark: "target" }
                  : { onPress: () => undefined, inactive: true, mark: "unavailable" }
        }
      />
      {snapshot.phase === "computer-turn" && <ThinkingIndicator />}
      <TimeBand model={model} />
      {attack.selected !== null && (
        <button type="button" className="btn" onClick={clearAttack}>
          {fr.cancelSelection}
        </button>
      )}
      <Board
        cards={model.bottom.board}
        label={fr.yourBoard}
        tileProps={(card) => {
          const can = hasAttack(card.id);
          const isSelected = attack.selected === card.id;
          return {
            onPress: () => setSelected(isSelected ? null : card.id),
            inactive: !can,
            pressed: can ? isSelected : undefined,
            mark: isSelected ? "selected" : undefined,
          };
        }}
      />
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
      {pending !== null && pendingPreview !== undefined && (
        <AttackSheet
          summary={fr.attackSummary(attackerName, targetName(pending))}
          lines={describeConsequence(pendingPreview, labels)}
          onCancel={() => setPending(null)}
          onConfirm={() => {
            clearAttack();
            setFocusToken((t) => t + 1);
            controller.submit(pendingPreview);
          }}
        />
      )}
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
