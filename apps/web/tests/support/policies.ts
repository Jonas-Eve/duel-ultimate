import type { Action, ActionPreview } from "../../src/engine/types";
import { isActivate, isAttack, isPlay } from "../../src/engine/types";
import { GameController, type ChooseAction, type ControllerDeps, type Snapshot } from "../../src/game/controller";
import { mulberry32 } from "../../src/game/seed";
import { instrumentedFactory, newSession } from "./engine";
import type { GameFactory } from "../../src/engine/session";

export const immediateScheduler = { yield: (): Promise<void> => Promise.resolve() };

/**
 * TEST ONLY. The "wear-down" ranking that makes EXHAUSTION observable (it never happens at
 * random: fact V7): hero effect, then summon, then attack a creature, last attack the hero.
 */
export function stallRank(a: Action): number {
  if (isActivate(a)) return 0;
  if (isPlay(a)) return 1;
  if (isAttack(a)) return a.attack.target === "hero" ? 3 : 2;
  throw new Error("concede has no rank: it must be excluded before ranking");
}

/** The index of the first action after a stable sort by rank (concede already excluded). */
function stallIndex(actions: readonly Action[]): number {
  let best = 0;
  for (let i = 1; i < actions.length; i++) if (stallRank(actions[i]) < stallRank(actions[best])) best = i;
  return best;
}

/** TEST ONLY, `chooseAction` form (the test injection point of `ControllerDeps`): the computer wears down too. */
export const stallPolicy: ChooseAction = (session) => {
  const playable = session.legalActions().filter((a) => a !== "concede");
  return playable[stallIndex(playable)];
};

/** Preview form, for the human: sorts `snapshot.legal` AFTER excluding `concede`. Same choice as `stallPolicy`. */
export function stallPreview(s: Snapshot): ActionPreview {
  const playable = s.legal.filter((p) => p.action !== "concede");
  return playable[stallIndex(playable.map((p) => p.action))];
}

/** Seeded uniform human choice, never concede. */
export function randomPreview(seed: number): (s: Snapshot) => ActionPreview {
  const rnd = mulberry32(seed);
  return (s) => {
    const playable = s.legal.filter((p) => p.action !== "concede");
    return playable[Math.floor(rnd() * playable.length)];
  };
}

/** Resolves with the snapshot once the computer is no longer playing. */
export function settle(c: GameController): Promise<Snapshot> {
  if (c.getSnapshot().phase !== "computer-turn") return Promise.resolve(c.getSnapshot());
  return new Promise((resolve) => {
    const off = c.subscribe(() => {
      if (c.getSnapshot().phase !== "computer-turn") {
        off();
        resolve(c.getSnapshot());
      }
    });
  });
}

/**
 * The human (both seats in two camps) plays with `choose` until the game is over or fatal.
 * Returns the number of human actions and the final snapshot.
 */
export async function playHuman(
  c: GameController,
  choose: (s: Snapshot) => ActionPreview,
  maxActions = 3000,
): Promise<{ humanActions: number; snapshot: Snapshot }> {
  let humanActions = 0;
  for (;;) {
    const s = await settle(c);
    if (s.phase !== "human-turn") return { humanActions, snapshot: s };
    if (humanActions >= maxActions) throw new Error("the game does not end");
    const res = c.submit(choose(s));
    if (!res.ok) throw new Error(`submit refused: ${res.reason}`);
    humanActions += 1;
  }
}

/** Deps on the real engine with an immediate scheduler; `over` overrides any of them. */
export function realDeps(over: Partial<ControllerDeps> = {}, factory?: GameFactory): ControllerDeps {
  return {
    createSession: (r) => newSession(r.deckOne, r.deckTwo, r.seed, factory),
    scheduler: immediateScheduler,
    now: () => 0,
    ...over,
  };
}

export { instrumentedFactory };

export interface PlayUntilResult {
  /** Human actions submitted. */
  steps: number;
  snapshot: Snapshot;
}

/**
 * Bounded scenario search: the human (both seats in two camps) plays with `choose` and the
 * computer replies (`settle`) until `predicate(snapshot)` holds. Stops at once, with zero steps,
 * if it already holds. Fails explicitly after `maxSteps` human actions, or if the game ends
 * (over / fatal) before the predicate held. `playHuman` runs to the end of the game and does
 * not fit this use.
 */
export async function playUntil(
  c: GameController,
  choose: (s: Snapshot) => ActionPreview,
  predicate: (s: Snapshot) => boolean,
  maxSteps: number,
): Promise<PlayUntilResult> {
  let steps = 0;
  for (;;) {
    const s = await settle(c);
    if (predicate(s)) return { steps, snapshot: s };
    if (s.phase !== "human-turn") throw new Error(`playUntil: the game ended (${s.phase}) after ${steps} steps before the predicate held`);
    if (steps >= maxSteps) throw new Error(`playUntil: the predicate did not hold within maxSteps = ${maxSteps}`);
    const res = c.submit(choose(s));
    if (!res.ok) throw new Error(`playUntil: submit refused: ${res.reason}`);
    steps += 1;
  }
}

/**
 * A seed for which the computer, opening alone, plays AT LEAST `minSteps` actions in its first
 * chain (the computer's choices are the engine's, so no test may pin a seed by hand: it is
 * searched here, deterministically, on the real engine). Fails explicitly when none is found.
 */
export async function seedWithComputerOpening(one: string, two: string, minSteps: number, tryUpTo = 200): Promise<number> {
  for (let seed = 0; seed < tryUpTo; seed++) {
    const c = GameController.create(
      { mode: "computer", deckOne: { id: one, name: one }, deckTwo: { id: two, name: two }, seed },
      realDeps(),
    );
    try {
      c.start();
      const s = await settle(c);
      if (s.phase === "human-turn" && s.computerSteps >= minSteps) return seed;
    } finally {
      c.dispose();
    }
  }
  throw new Error(`no seed in 0..${tryUpTo} gives ${one}/${two} a computer opening of ${minSteps} actions`);
}

/**
 * A seed for which, once the computer's opening chain is over, the human has an action that
 * keeps the main (no handover, no end of game).
 */
export async function seedWithQuietHumanAction(one: string, two: string, tryUpTo = 200): Promise<number> {
  for (let seed = 0; seed < tryUpTo; seed++) {
    const c = GameController.create(
      { mode: "computer", deckOne: { id: one, name: one }, deckTwo: { id: two, name: two }, seed },
      realDeps(),
    );
    try {
      c.start();
      const s = await settle(c);
      if (s.phase === "human-turn" && s.computerSteps > 0 && s.legal.some((p) => !p.hands_over && p.ends_game === null && p.action !== "concede")) return seed;
    } finally {
      c.dispose();
    }
  }
  throw new Error(`no seed in 0..${tryUpTo} gives ${one}/${two} a quiet human action after the computer's opening`);
}
