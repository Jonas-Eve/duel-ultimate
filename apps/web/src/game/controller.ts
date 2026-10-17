// The local game controller: a state machine plus an immutable snapshot, for both modes
// ("computer" and "two-camps"). No React: the caller (App, #64) creates it in an event handler
// (never in an effect: StrictMode replays effects and a cleanup would free the second mount's
// instance) and renders `getSnapshot()`.
//
// It decides nothing about the rules. "Whose turn", "is the game over", the cost of an action,
// "hands the main over" and "ends the game" all come back from the engine (views, previews);
// the end of the game is never deduced from the events.
import { EngineError, parseEngineError } from "../engine/errors";
import type { GameSession } from "../engine/session";
import type { Action, ActionPreview, EngineEvent, PlayerView, Seat } from "../engine/types";
import { MAX_COMPUTER_ACTIONS_PER_CHAIN, COMPUTER_SLICE_MS, pickComputerAction, randomPolicy, type ComputerPolicy } from "./computer";
import { captureRecord, type StepRecord } from "./records";
import { deriveAiSeed, mulberry32 } from "./seed";

export type Mode = "computer" | "two-camps";
/** The closed set of deck symbols the card face can draw (decorative; the deck's name is what is read out). */
export type DeckEmblem = "bolt" | "battlements";
export interface DeckRef {
  id: string;
  name: string;
  /** Absent = no chip on the card art. */
  emblem?: DeckEmblem;
}
export interface GameConfig {
  mode: Mode;
  deckOne: DeckRef;
  deckTwo: DeckRef;
  /** Already validated as a u32 by the caller. */
  seed: number;
}
/** What the controller asks the factory for: the caller (App, #64) closes over the starter data. */
export interface SessionRequest {
  deckOne: string;
  deckTwo: string;
  seed: number;
}
export interface ControllerDeps {
  createSession(request: SessionRequest): GameSession;
  policy: ComputerPolicy;
  scheduler: { yield(): Promise<void> };
  now(): number;
}
export type Phase = "human-turn" | "computer-turn" | "over" | "fatal";
export interface Snapshot {
  phase: Phase;
  mode: Mode;
  humanSeat: "One" | null;
  seed: number;
  aiSeed: number | null;
  dataVersion: string;
  players: Record<Seat, DeckRef>;
  views: { One?: PlayerView; Two?: PlayerView };
  legal: ActionPreview[];
  records: StepRecord[];
  error: EngineError | null;
  computerSteps: number;
}
export type SubmitResult =
  | { ok: true }
  | { ok: false; reason: "not-your-turn" | "stale" | "disposed" | "refused"; error?: EngineError };

type Views = { One?: PlayerView; Two?: PlayerView };
type Deps = Partial<ControllerDeps> & Pick<ControllerDeps, "createSession">;

const defaultScheduler = { yield: (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0)) };

export class GameController {
  private readonly listeners = new Set<() => void>();
  private readonly policy: ComputerPolicy;
  private readonly scheduler: ControllerDeps["scheduler"];
  private readonly now: () => number;
  private readonly rng: () => number;
  private readonly aiSeed: number | null;
  private readonly players: Record<Seat, DeckRef>;
  private readonly dataVersion: string;

  private views: Views;
  private legal: ActionPreview[] = [];
  private records: StepRecord[];
  private error: EngineError | null = null;
  private fatal = false;
  private computerSteps = 0;
  private snapshot: Snapshot;

  /** Bumped by `dispose`: a driver holding an older token stops at its next check. */
  private generation = 0;
  private disposed = false;
  private started = false;

  /** Creates the session, reads the start events and the views. Throws `EngineError` if that fails (nothing leaks). */
  static create(config: GameConfig, deps: Deps): GameController {
    let session: GameSession;
    try {
      session = deps.createSession({ deckOne: config.deckOne.id, deckTwo: config.deckTwo.id, seed: config.seed });
    } catch (e) {
      throw parseEngineError(e);
    }
    try {
      return new GameController(config, deps, session);
    } catch (e) {
      session.dispose();
      throw parseEngineError(e);
    }
  }

  private constructor(
    private readonly config: GameConfig,
    deps: Deps,
    private readonly session: GameSession,
  ) {
    this.policy = deps.policy ?? randomPolicy;
    this.scheduler = deps.scheduler ?? defaultScheduler;
    this.now = deps.now ?? (() => performance.now());
    this.aiSeed = config.mode === "computer" ? deriveAiSeed(config.seed) : null;
    // One generator for the whole game: it advances by one draw per computer action, so the same
    // seed and the same human actions replay the same game, computer included.
    this.rng = mulberry32(this.aiSeed ?? 0);
    this.players = { One: config.deckOne, Two: config.deckTwo };

    const startEvents = [...session.startEvents];
    const started = startEvents.find((e): e is Extract<EngineEvent, { game_started: unknown }> => "game_started" in e);
    this.dataVersion = (started?.game_started.data_versions ?? []).join(", ");

    // Against the computer only the human seat's view is ever requested: the records written
    // from it cannot hold a card of the computer's hand (display choice, not a guarantee).
    this.views = this.readViews();
    const one = this.viewOne();
    this.records = [captureRecord({ seq: 0, actor: one.active, by: "setup", events: startEvents, views: this.views })];
    if (this.currentPhase() === "human-turn") this.legal = session.legalActionsWithTime();
    this.snapshot = this.build();
  }

  /** Publishes the initial state and, if the computer opens, starts its chain. */
  start(): void {
    if (this.disposed || this.started) return;
    this.started = true;
    this.publish();
    this.driveIfComputerTurn();
  }

  // Arrow-function properties, not methods: `useSyncExternalStore(controller.subscribe,
  // controller.getSnapshot)` passes them detached, so they must not depend on `this` at call time.
  readonly getSnapshot = (): Snapshot => this.snapshot;

  readonly subscribe = (fn: () => void): (() => void) => {
    if (this.disposed) return () => undefined;
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  /** The only entry point of human actions: accepts only the identity of a preview of the CURRENT snapshot. */
  submit(preview: ActionPreview): SubmitResult {
    if (this.disposed) return { ok: false, reason: "disposed" };
    if (this.currentPhase() !== "human-turn") return { ok: false, reason: "not-your-turn" };
    if (!this.legal.includes(preview)) return { ok: false, reason: "stale" };

    const actor = this.viewOne().active;
    let events: EngineEvent[];
    try {
      events = this.session.apply(preview.action);
    } catch (e) {
      const error = parseEngineError(e);
      // The engine is the judge: a rule error leaves state and views untouched.
      if (error.fatal) this.fail(error);
      else {
        this.error = error;
        this.publish();
      }
      return { ok: false, reason: "refused", error };
    }
    try {
      this.commit(events, actor, "human");
      this.error = null;
      this.afterAction();
    } catch (e) {
      const error = parseEngineError(e);
      this.fail(error);
      return { ok: false, reason: "refused", error };
    }
    this.publish();
    this.driveIfComputerTurn();
    return { ok: true };
  }

  concede(): SubmitResult {
    if (this.disposed) return { ok: false, reason: "disposed" };
    if (this.currentPhase() !== "human-turn") return { ok: false, reason: "not-your-turn" };
    const preview = this.legal.find((p) => p.action === "concede");
    if (preview === undefined) return { ok: false, reason: "refused" };
    return this.submit(preview);
  }

  clearError(): void {
    if (this.disposed || this.error === null || this.fatal) return;
    this.error = null;
    this.publish();
  }

  /** Cancels the driver, drops subscribers and frees the session once. Idempotent. */
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.generation += 1;
    this.listeners.clear();
    this.session.dispose();
  }

  // ---- internals -------------------------------------------------------------------------

  /** A subscriber may have disposed the controller during `publish()`: then nothing starts (no session call after `dispose`). */
  private driveIfComputerTurn(): void {
    if (this.disposed || this.currentPhase() !== "computer-turn") return;
    void this.drive(this.generation);
  }

  private readViews(): Views {
    return this.config.mode === "computer"
      ? { One: this.session.view("One") }
      : { One: this.session.view("One"), Two: this.session.view("Two") };
  }

  private viewOne(): PlayerView {
    const v = this.views.One;
    if (v === undefined) throw new EngineError("internal_error", "the view of seat One is missing");
    return v;
  }

  /** From the view only: `result` for the end, `active` for the turn. Never from events. */
  private currentPhase(): Phase {
    if (this.fatal) return "fatal";
    const v = this.viewOne();
    if (v.result !== null) return "over";
    if (this.config.mode === "computer" && v.active !== "One") return "computer-turn";
    return "human-turn";
  }

  /** Records the step and reads the views, once the engine accepted the action. */
  private commit(events: EngineEvent[], actor: Seat, by: "human" | "computer"): void {
    const views = this.readViews();
    const record = captureRecord({ seq: this.records.length, actor, by, events, views });
    this.views = views;
    this.records = [...this.records, record];
  }

  /** After a human action: the preview list only on a human turn (one clone per action: never anywhere else). */
  private afterAction(): void {
    const phase = this.currentPhase();
    this.legal = phase === "human-turn" ? this.session.legalActionsWithTime() : [];
    if (phase === "computer-turn") this.computerSteps = 0; // a new chain starts
  }

  private fail(error: EngineError): void {
    this.fatal = true;
    this.error = error;
    this.legal = [];
    this.publish();
  }

  private build(): Snapshot {
    return Object.freeze({
      phase: this.currentPhase(),
      mode: this.config.mode,
      humanSeat: this.config.mode === "computer" ? ("One" as const) : null,
      seed: this.config.seed,
      aiSeed: this.aiSeed,
      dataVersion: this.dataVersion,
      players: Object.freeze({ ...this.players }),
      views: Object.freeze({ ...this.views }),
      legal: Object.freeze([...this.legal]) as ActionPreview[],
      records: Object.freeze([...this.records]) as StepRecord[],
      error: this.error,
      computerSteps: this.computerSteps,
    });
  }

  private publish(): void {
    if (this.disposed) return;
    this.snapshot = this.build();
    for (const fn of [...this.listeners]) fn();
  }

  /**
   * The computer's chain. The first snapshot (computer-turn, 0 steps) is already published, so
   * the indicator shows at once; one yield lets the browser paint it. Then one action at a time
   * (`legalActions` -> policy -> `apply`), a snapshot and a yield whenever a slice of
   * COMPUTER_SLICE_MS elapsed, and a check of the generation token after every await.
   */
  private async drive(token: number): Promise<void> {
    try {
      await this.scheduler.yield();
      if (token !== this.generation) return;
      let sliceStart = this.now();
      for (;;) {
        if (this.computerSteps >= MAX_COMPUTER_ACTIONS_PER_CHAIN) {
          throw new EngineError("computer_chain_limit", `the computer played ${MAX_COMPUTER_ACTIONS_PER_CHAIN} actions in a row`);
        }
        const actor = this.viewOne().active;
        const action: Action = pickComputerAction(this.session.legalActions(), this.policy, this.rng);
        const events = this.session.apply(action);
        this.commit(events, actor, "computer");
        this.computerSteps += 1;

        const phase = this.currentPhase();
        if (phase !== "computer-turn") {
          // End of the chain: the game is over or the main is back with the human.
          this.legal = phase === "human-turn" ? this.session.legalActionsWithTime() : [];
          this.publish();
          return;
        }
        if (this.now() - sliceStart >= COMPUTER_SLICE_MS) {
          this.publish();
          await this.scheduler.yield();
          if (token !== this.generation) return;
          sliceStart = this.now();
        }
      }
    } catch (e) {
      if (token !== this.generation) return;
      this.fail(parseEngineError(e));
    }
  }
}
