import type { Action, ActionPreview } from "../../src/engine/types";
import { isActivate, isAttack, isPlay } from "../../src/engine/types";
import type { ComputerPolicy } from "../../src/game/computer";
import type { ControllerDeps, GameController, Snapshot } from "../../src/game/controller";
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

/** `ComputerPolicy` form: the index of the first action after a stable sort by rank. */
export const stallPolicy: ComputerPolicy = (actions) => {
  let best = 0;
  for (let i = 1; i < actions.length; i++) if (stallRank(actions[i]) < stallRank(actions[best])) best = i;
  return best;
};

/** Preview form, for the human: sorts `snapshot.legal` AFTER excluding `concede`. Same choice as `stallPolicy`. */
export function stallPreview(s: Snapshot): ActionPreview {
  const playable = s.legal.filter((p) => p.action !== "concede");
  return playable[stallPolicy(playable.map((p) => p.action), () => 0)];
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
    policy: (a, rng) => Math.floor(rng() * a.length),
    scheduler: immediateScheduler,
    now: () => 0,
    ...over,
  };
}

export { instrumentedFactory };
