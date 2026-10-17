import { useEffect, useRef, useState, useSyncExternalStore, type KeyboardEvent } from "react";
import { isAttackBy, isConcede, type ActionPreview, type Seat, type Target } from "../engine/types";
import type { GameController, Phase } from "../game/controller";
import { toScreenModel } from "../game/screen-model";
import { ActionBar } from "./ActionBar";
import { AttackSheet } from "./AttackSheet";
import { deriveAttackState, previewOn } from "./attack-state";
import { Board } from "./Board";
import { CardDetailSheet } from "./CardDetailSheet";
import { ConfirmDialog } from "./ConfirmDialog";
import { describeConsequence } from "./consequence";
import { EndScreen } from "./EndScreen";
import { ErrorBanner } from "./ErrorBanner";
import { FatalScreen } from "./FatalScreen";
import { fr } from "./fr";
import { Hand, playPreviewOf } from "./Hand";
import { HeroBar } from "./HeroBar";
import { LogBar } from "./LogBar";
import { LogDrawer } from "./LogDrawer";
import { formatBlocks, formatRecords, logContext } from "./log-format";
import { consequenceLabels, outcomeOf } from "./outcome";
import { PlayActions } from "./PlayActions";
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

/**
 * Two camps: "Au tour de {camp}" when the main changes camp (the swap of top and bottom is silent
 * without it). Like the chain announcement it is produced on the transition only and goes away
 * as soon as the next action is recorded. `token` counts the swaps: the screen moves the focus to
 * its region on each one, since the tile that was pressed may have left the hand.
 */
function useTurnAnnouncement(active: Seat | null, recordCount: number, text: (seat: Seat) => string): { message: string; token: number } {
  const [seen, setSeen] = useState(active);
  const [state, setState] = useState({ message: "", at: -1, token: 0 });
  if (seen !== active) {
    setSeen(active);
    if (seen !== null && active !== null) setState({ message: text(active), at: recordCount, token: state.token + 1 });
  }
  return { message: state.at === recordCount ? state.message : "", token: state.token };
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
  // The read-only card detail: which card (by id) of which zone. Looked up in the current model on every
  // render, and dropped during render when the card is no longer there (never by an effect).
  // The zone is part of the key: after a hand-over in two camps the swapped hand never reopens a card as another zone's.
  const [detail, setDetail] = useState<{ id: number; zone: "opp-board" | "other-hand" | "hand" | "own-board" } | null>(null);
  const region = useRef<HTMLElement>(null);
  // After an attack is confirmed the focus goes back to the game region: the dialog closes and
  // the tile that opened it may be gone. Runs after the dialog's own focus restoration.
  useEffect(() => {
    if (focusToken > 0) region.current?.focus();
  }, [focusToken]);
  const twoCamps = snapshot.mode === "two-camps";
  const chain = useChainAnnouncement(snapshot.phase, snapshot.computerSteps, snapshot.records.length);
  const turn = useTurnAnnouncement(twoCamps ? (snapshot.views.One?.active ?? null) : null, snapshot.records.length, (seat) =>
    fr.turnOf(logContext(snapshot).labels[seat]),
  );
  const announcement = twoCamps ? turn.message : chain;
  useEffect(() => {
    if (turn.token > 0) region.current?.focus();
  }, [turn.token]);

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
  // Names of the regions: fixed against the computer; the camp's own name in two camps, which also
  // tells the two sides of a mirror apart.
  const names = twoCamps
    ? {
        topHero: fr.campRegion(model.top.label, fr.regionHero),
        topBoard: fr.campRegion(model.top.label, fr.regionBoard),
        topHand: fr.campRegion(model.top.label, fr.regionHand),
        bottomHero: fr.campRegion(model.bottom.label, fr.regionHero),
        bottomBoard: fr.campRegion(model.bottom.label, fr.regionBoard),
        bottomHand: fr.campRegion(model.bottom.label, fr.regionHand),
      }
    : {
        topHero: fr.opponentHero,
        topBoard: fr.opponentBoard,
        topHand: "",
        bottomHero: fr.yourHero,
        bottomBoard: fr.yourBoard,
        bottomHand: fr.yourHand,
      };
  const lastLine = formatRecords(snapshot.records.slice(-1), ctx).at(-1);
  const concede = snapshot.legal.find((p) => isConcede(p.action));
  // The interface hands back a preview of the CURRENT snapshot, never an action of its own;
  // a stale one is ignored by the controller (double press), a refused one comes back as `error`.
  const submit = (preview: ActionPreview) => {
    controller.submit(preview);
  };

  const detailSide = detail === null ? undefined : detail.zone === "hand" || detail.zone === "own-board" ? model.bottom : model.top;
  const detailCards =
    detail === null ? [] : detail.zone === "opp-board" ? model.top.board : detail.zone === "own-board" ? model.bottom.board : (detailSide?.hand ?? []);
  const detailCard = detail === null ? undefined : detailCards.find((c) => c.id === detail.id);
  if (detail !== null && detailCard === undefined) setDetail(null);
  // "Jouer": the engine's preview of the CURRENT snapshot, searched on every render; none = inactive, with the reason.
  const detailPlay = detail?.zone === "hand" && detailCard !== undefined ? playPreviewOf(snapshot.legal, detailCard) : undefined;

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
        label={names.topHero}
        target={attack.selected === null ? undefined : { possible: attack.heroTargetable, onPress: () => setPending("hero") }}
      />
      {twoCamps && (
        <>
          <p className="row-caption" aria-hidden="true">
            {fr.otherHandCaption(model.top.label)}
          </p>
          <Hand
            cards={model.top.hand ?? []}
            legal={[]}
            label={names.topHand}
            onOpen={(card) => setDetail({ id: card.id, zone: "other-hand" })}
            emblem={model.top.emblem}
          />
        </>
      )}
      <Board
        cards={model.top.board}
        label={names.topBoard}
        emblem={model.top.emblem}
        tileProps={
          attack.selected === null
            ? (card) => ({ onPress: () => setDetail({ id: card.id, zone: "opp-board" }), opensDialog: true })
            : (card) =>
                attack.creatureTargets.has(card.id)
                  ? { onPress: () => setPending({ creature: card.id }), mark: "target" }
                  : { onPress: () => undefined, inactive: true, mark: "unavailable" }
        }
      />
      {snapshot.phase === "computer-turn" && <ThinkingIndicator />}
      <TimeBand model={model} twoCamps={twoCamps} />
      <p aria-live="polite" className="attack-hint">
        {attack.selected !== null ? fr.chooseTarget : ""}
      </p>
      {attack.selected !== null && (
        <button type="button" className="btn" onClick={clearAttack}>
          {fr.cancelSelection}
        </button>
      )}
      <Board
        cards={model.bottom.board}
        label={names.bottomBoard}
        emblem={model.bottom.emblem}
        // A touch only opens the detail (also while a target is being chosen: it changes the attacker from there).
        tileProps={(card) => ({
          onPress: () => setDetail({ id: card.id, zone: "own-board" }),
          opensDialog: true,
          mark: attack.selected === card.id ? "selected" : undefined,
        })}
      />
      <HeroBar side={model.bottom} label={names.bottomHero} />
      <p aria-live="polite" className="announce">
        {announcement}
      </p>
      <div className="log-row">
        <LogBar line={lastLine} live={humanTurn && announcement === ""} />
        <button type="button" className="btn" onClick={() => setLogOpen(true)}>
          {fr.logButton}
        </button>
      </div>
      <Hand
        cards={model.bottom.hand ?? []}
        legal={snapshot.legal}
        onOpen={(card) => setDetail({ id: card.id, zone: "hand" })}
        label={names.bottomHand}
        emblem={model.bottom.emblem}
      />
      <ActionBar
        hero={model.bottom.hero}
        legal={snapshot.legal}
        enabled={humanTurn}
        labels={labels}
        onActivate={submit}
        onConcede={() => setConfirming(true)}
      />
      {logOpen && <LogDrawer blocks={formatBlocks(snapshot.records, ctx)} onClose={() => setLogOpen(false)} />}
      {detail !== null && detailCard !== undefined && detailSide !== undefined && (
        <CardDetailSheet
          card={detailCard}
          emblem={detailSide.emblem}
          deckName={detailSide.deckName}
          onClose={() => setDetail(null)}
          attack={
            detail.zone === "own-board"
              ? {
                  available: hasAttack(detailCard.id),
                  reason: humanTurn ? fr.attackUnavailable : fr.attackUnavailableComputer,
                  // The detail closes (the focus goes back to the tile), the creature is the attacker: targets next. Idempotent.
                  onAttack: () => {
                    setDetail(null);
                    setSelected(detailCard.id);
                    setPending(null);
                  },
                }
              : undefined
          }
          actions={
            detail.zone === "hand" ? (
              <PlayActions
                preview={detailPlay}
                labels={labels}
                reason={humanTurn ? fr.playUnavailable : fr.playUnavailableComputer}
                onPlay={(preview) => {
                  // The tile leaves the hand: the focus goes to the game region once the sheet has closed.
                  setDetail(null);
                  setFocusToken((t) => t + 1);
                  submit(preview);
                }}
              />
            ) : undefined
          }
        />
      )}
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
          title={twoCamps ? fr.campConcedeTitle(model.bottom.seat === "One" ? 1 : 2) : undefined}
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
