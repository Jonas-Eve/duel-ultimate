// The screen model (stub: the implementation follows the red tests).
import type { CardView, HeroView, Seat } from "../engine/types";
import type { Snapshot } from "./controller";

export interface SideModel {
  seat: Seat;
  label: string;
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

export function toScreenModel(_snapshot: Snapshot): ScreenModel {
  throw new Error("not implemented");
}
