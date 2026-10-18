import { EngineError } from "../../src/engine/errors";
import type { GameSession } from "../../src/engine/session";
import type { Action, EngineEvent, GameResult, PlayerView, Seat } from "../../src/engine/types";

export interface FakeOptions {
  /** Who holds the main once `applied` actions went through. */
  activeAfter: (applied: number) => Seat;
  /** The raw legal actions at that point (default: one summon). */
  legal?: (applied: number) => Action[];
  resultAfter?: (applied: number) => GameResult | null;
}

export interface FakeState {
  applied: number;
  legalActionsCalls: number;
  computerActionCalls: number;
  /** Every `draw` the controller handed over. */
  draws: number[];
  disposed: number;
}

const hero = { name: "Héros d'essai", health: 10, max_time: 6, effects: [] };

/** A hand-written engine double for the cases the real engine cannot produce (stuck, endless). */
export function fakeSession(o: FakeOptions): { session: GameSession; state: FakeState } {
  const state: FakeState = { applied: 0, legalActionsCalls: 0, computerActionCalls: 0, draws: [], disposed: 0 };
  const legal = (): Action[] => (o.legal ? o.legal(state.applied) : [{ play: { card: 1 } }]);
  const view = (seat: Seat): PlayerView => ({
    seat,
    active: o.activeAfter(state.applied),
    budget: 3,
    result: o.resultAfter ? o.resultAfter(state.applied) : null,
    you: { hero, hand: [], deck_count: 0, board: [], discard: [] },
    opponent: { hero, hand_count: 0, deck_count: 0, board: [], discard: [] },
  });
  const session: GameSession = {
    startEvents: [{ game_started: { first: o.activeAfter(0), budget: 3 } }],
    legalActions: () => {
      state.legalActionsCalls += 1;
      return legal();
    },
    // What the binding does: never concede, and `computer_stuck` when nothing else is playable.
    computerAction: (_seat, draw) => {
      state.computerActionCalls += 1;
      state.draws.push(draw);
      const playable = legal().filter((a) => a !== "concede");
      if (playable.length === 0) throw new EngineError("computer_stuck", "the computer has no legal action other than conceding");
      return playable[draw % playable.length];
    },
    legalActionsWithTime: () =>
      legal().map((action) => ({ action, cost: 1, hands_over: false, budget_after: 1, ends_game: null, attacker_destroyed: false })),
    view,
    apply: (): EngineEvent[] => {
      state.applied += 1;
      return [{ effect_activated: { player: o.activeAfter(state.applied - 1), effect: 0, cost: 1 } }];
    },
    dispose: () => {
      state.disposed += 1;
    },
  };
  return { session, state };
}
