// The screen model: pure, no React. It only arranges what the engine's views already say
// (top/bottom, visible or counted hands); budget and active seat are read, never computed.
import { EngineError } from "../engine/errors";
import type { CardView, HeroView, PlayerView, Seat } from "../engine/types";
import type { DeckEmblem, Snapshot } from "./controller";

export interface SideModel {
  seat: Seat;
  label: string;
  /** The name of this side's deck (`snapshot.players[seat].name`), spoken by the deck symbol of the card detail. */
  deckName: string;
  /** The deck symbol of this side's seat (read from the snapshot's decks), undefined = none. */
  emblem: DeckEmblem | undefined;
  hero: HeroView;
  handCount: number;
  hand: CardView[] | null;
  deckCount: number;
  board: CardView[];
  discard: CardView[];
  active: boolean;
}
export interface ScreenModel {
  top: SideModel;
  bottom: SideModel;
  budget: number;
  active: Seat;
  bottomIsActive: boolean;
}

const other = (s: Seat): Seat => (s === "One" ? "Two" : "One");

function required(view: PlayerView | undefined, seat: Seat): PlayerView {
  if (view === undefined) throw new EngineError("internal_error", `the view of seat ${seat} is missing`);
  return view;
}

/** A side from the `you` block of that seat's own view: its hand is visible, its deck is a count. */
function ownSide(view: PlayerView, label: string, deckName: string, emblem: DeckEmblem | undefined, activeSeat: Seat): SideModel {
  const you = view.you;
  return {
    seat: view.seat,
    label,
    deckName,
    emblem,
    hero: you.hero,
    handCount: you.hand.length,
    hand: you.hand,
    deckCount: you.deck_count,
    board: you.board,
    discard: you.discard,
    active: view.seat === activeSeat,
  };
}

/**
 * Against the computer: the human (seat One) is always at the bottom with a visible hand, the
 * computer on top with `hand: null` and only a count. Two camps: the camp that holds the main is
 * at the bottom, both hands are visible (they share the screen), decks are counts only.
 */
export function toScreenModel(snapshot: Snapshot): ScreenModel {
  const one = required(snapshot.views.One, "One");
  const active = one.active;
  const deck = (seat: Seat) => snapshot.players[seat].name;
  const emblem = (seat: Seat) => snapshot.players[seat].emblem;

  if (snapshot.mode === "computer") {
    const opp = one.opponent;
    const top: SideModel = {
      seat: "Two",
      label: `Ordinateur — ${deck("Two")}`,
      deckName: deck("Two"),
      emblem: emblem("Two"),
      hero: opp.hero,
      handCount: opp.hand_count,
      hand: null,
      deckCount: opp.deck_count,
      board: opp.board,
      discard: opp.discard,
      active: active === "Two",
    };
    const bottom = ownSide(one, `Toi — ${deck("One")}`, deck("One"), emblem("One"), active);
    return { top, bottom, budget: one.budget, active, bottomIsActive: active === "One" };
  }

  const camp = (seat: Seat) => `Camp ${seat === "One" ? 1 : 2} — ${deck(seat)}`;
  const bottomView = required(snapshot.views[active], active);
  const topView = required(snapshot.views[other(active)], other(active));
  return {
    top: ownSide(topView, camp(topView.seat), deck(topView.seat), emblem(topView.seat), active),
    bottom: ownSide(bottomView, camp(bottomView.seat), deck(bottomView.seat), emblem(bottomView.seat), active),
    budget: bottomView.budget,
    active,
    bottomIsActive: true,
  };
}
