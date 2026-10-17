// The JSON contract of the engine as the client sees it, written once from the Rust sources
// (`crates/engine/src/{rules,view,preview}.rs`) and pinned against the real WASM by
// `tests/wasm.contract.test.ts`. Seats are capitalised ("One"/"Two") everywhere, ids are
// numbers, an event is an object with a single snake_case key, an action is an object with a
// single key or the string "concede".

export type Seat = "One" | "Two";
export type WinReason = "hero_down" | "exhaustion" | "concede";
export interface GameResult {
  winner: Seat;
  reason: WinReason;
}

export type Target = "hero" | { creature: number };
export type PlayAction = { play: { card: number } };
export type ActivateAction = { activate: { effect: number } };
export type AttackAction = { attack: { attacker: number; target: Target } };
export type Action = PlayAction | ActivateAction | AttackAction | "concede";

/** One legal action with its time facts; `action` goes back to `apply` unchanged. */
export interface ActionPreview {
  action: Action;
  cost: number | null;
  hands_over: boolean;
  budget_after: number | null;
  ends_game: GameResult | null;
}

export interface HeroEffect {
  name: string;
  cost: number;
  effect: { kind: "draw"; count: number };
}
export interface HeroView {
  name: string;
  health: number;
  max_time: number;
  effects: HeroEffect[];
}
export interface CardView {
  id: number;
  name: string | null;
  cost: number;
  force: number;
  attack_cost: number;
  health: number;
  provocation: boolean;
  riposte: number;
}
export interface OwnSide {
  hero: HeroView;
  hand: CardView[];
  deck_count: number;
  board: CardView[];
  discard: CardView[];
}
/** No `hand` field: the opposing hand content does not exist in the view, by construction. */
export interface OpponentSide {
  hero: HeroView;
  hand_count: number;
  deck_count: number;
  board: CardView[];
  discard: CardView[];
}
export interface PlayerView {
  seat: Seat;
  active: Seat;
  budget: number;
  result: GameResult | null;
  you: OwnSide;
  opponent: OpponentSide;
}

/** The rule options recorded in `game_started` when they differ from the engine default (keys in the order of Rust). No screen sets them. */
export interface RuleSetJson {
  hero_riposte: boolean;
  board_limit: number | null;
}

export type EngineEvent =
  | { game_started: { first: Seat; budget: number; data_versions?: string[]; rules?: RuleSetJson } }
  | { effect_activated: { player: Seat; effect: number; cost: number } }
  | { drew: { player: Seat; card: number } }
  | { played: { player: Seat; card: number; cost: number } }
  | { attacked: { player: Seat; attacker: number; cost: number } }
  | { damaged: { player: Seat; amount: number } }
  | { creature_damaged: { player: Seat; creature: number; amount: number } }
  | { riposted: { player: Seat; creature: number; amount: number } }
  | { died: { player: Seat; creature: number } }
  | { token_moved: { player: Seat; budget: number } }
  | { active_player_changed: { player: Seat } }
  | { game_won: { winner: Seat; reason: WinReason } };

/** The variants of `enum Event`, in declaration order: a new Rust variant breaks the contract test. */
export const EVENT_KINDS = [
  "game_started",
  "effect_activated",
  "drew",
  "played",
  "attacked",
  "damaged",
  "creature_damaged",
  "riposted",
  "died",
  "token_moved",
  "active_player_changed",
  "game_won",
] as const;
export type EventKind = (typeof EVENT_KINDS)[number];

export const isPlay = (a: Action): a is PlayAction => typeof a === "object" && "play" in a;
export const isActivate = (a: Action): a is ActivateAction => typeof a === "object" && "activate" in a;
export const isAttack = (a: Action): a is AttackAction => typeof a === "object" && "attack" in a;
export const isConcede = (a: Action): a is "concede" => a === "concede";

const sameTarget = (a: Target, b: Target): boolean =>
  a === "hero" || b === "hero" ? a === b : a.creature === b.creature;

export const isAttackBy =
  (attacker: number) =>
  (a: Action): a is AttackAction =>
    isAttack(a) && a.attack.attacker === attacker;
export const isAttackOn =
  (target: Target) =>
  (a: Action): a is AttackAction =>
    isAttack(a) && sameTarget(a.attack.target, target);

export const eventKind = (e: EngineEvent): EventKind => Object.keys(e)[0] as EventKind;
