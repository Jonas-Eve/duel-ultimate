// Journal records: pure, no React. One record per step (the setup, then every action), made of
// what the engine returned: the events (ids only), plus the names and current health of the
// cards those events cite, read from the views taken AFTER the action.
//
// Confidentiality is by construction, not by an `if`: a record is written from the views the
// caller hands in, and against the computer the caller hands in only the human seat's view,
// whose `opponent` side has no hand at all. The id of a card the computer drew therefore has no
// entry in `cards`, and the log formatter writes "draws a card" because the name is missing.
// This is a display choice, not a security guarantee (see apps/web/README.md).
import type { CardView, EngineEvent, HeroEffect, PlayerView, Seat } from "../engine/types";

export interface CardNote {
  name: string | null;
  /** Current health after the action (0 for a card in the discard). */
  health: number;
}
export interface HeroNote {
  name: string;
  health: number;
  effects: HeroEffect[];
}
export interface StepRecord {
  /** 0 = the start, then 1, 2... */
  seq: number;
  actor: Seat;
  by: "setup" | "human" | "computer";
  events: EngineEvent[];
  /** Id -> name and health, for each id cited by the events AND visible in the views provided. */
  cards: Record<number, CardNote>;
  hero: Record<Seat, HeroNote>;
}
export interface CaptureInput {
  seq: number;
  actor: Seat;
  by: StepRecord["by"];
  events: EngineEvent[];
  views: { One?: PlayerView; Two?: PlayerView };
}

const never = (x: never): never => {
  throw new Error(`unknown engine event: ${JSON.stringify(x)}`);
};

/**
 * Ids of the cards an event cites. Exhaustive on the event kinds: an unknown event is a loud
 * error, never silently ignored (a new Rust variant must be handled here on purpose).
 */
export function citedCardIds(event: EngineEvent): number[] {
  const e = event;
  if ("game_started" in e) return [];
  if ("effect_activated" in e) return [];
  if ("drew" in e) return [e.drew.card];
  if ("played" in e) return [e.played.card];
  if ("attacked" in e) return [e.attacked.attacker];
  if ("damaged" in e) return [];
  if ("creature_damaged" in e) return [e.creature_damaged.creature];
  if ("riposted" in e) return [e.riposted.creature];
  if ("hero_riposted" in e) return [e.hero_riposted.creature];
  if ("died" in e) return [e.died.creature];
  if ("token_moved" in e) return [];
  if ("active_player_changed" in e) return [];
  if ("game_won" in e) return [];
  return never(e);
}

const opposite = (s: Seat): Seat => (s === "One" ? "Two" : "One");
const note = (c: CardView): CardNote => ({ name: c.name, health: c.health });

export function captureRecord(input: CaptureInput): StepRecord {
  const { seq, actor, by, events, views } = input;
  const provided = [views.One, views.Two].filter((v): v is PlayerView => v !== undefined);
  if (provided.length === 0) throw new Error("captureRecord needs at least one view");

  // Only zones that exist in a view: the opposing hand does not (`OpponentSide` has no `hand`).
  const visible = new Map<number, CardView>();
  for (const v of provided) {
    for (const c of [...v.you.hand, ...v.you.board, ...v.you.discard, ...v.opponent.board, ...v.opponent.discard]) {
      visible.set(c.id, c);
    }
  }

  const cards: Record<number, CardNote> = {};
  for (const event of events) {
    for (const id of citedCardIds(event)) {
      const card = visible.get(id);
      if (card !== undefined) cards[id] = note(card);
    }
  }

  const first = provided[0];
  const heroOf = (h: PlayerView["you"]["hero"]): HeroNote => ({ name: h.name, health: h.health, effects: h.effects });
  const hero = {
    [first.seat]: heroOf(first.you.hero),
    [opposite(first.seat)]: heroOf(first.opponent.hero),
  } as Record<Seat, HeroNote>;

  return { seq, actor, by, events, cards, hero };
}
