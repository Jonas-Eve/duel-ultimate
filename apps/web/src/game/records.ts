// Journal records (stub: the implementation follows the red tests).
import type { EngineEvent, HeroEffect, PlayerView, Seat } from "../engine/types";

export interface CardNote {
  name: string | null;
  health: number;
}
export interface StepRecord {
  seq: number;
  actor: Seat;
  by: "setup" | "human" | "computer";
  events: EngineEvent[];
  cards: Record<number, CardNote>;
  hero: Record<Seat, { name: string; health: number; effects: HeroEffect[] }>;
}
export interface CaptureInput {
  seq: number;
  actor: Seat;
  by: StepRecord["by"];
  events: EngineEvent[];
  views: { One?: PlayerView; Two?: PlayerView };
}

export function captureRecord(_input: CaptureInput): StepRecord {
  throw new Error("not implemented");
}
