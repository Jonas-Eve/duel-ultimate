import { describe, expect, test } from "vitest";
import type { Action, EngineEvent } from "../src/engine/types";
import { eventKind, isActivate, isAttack, isAttackBy, isAttackOn, isConcede, isPlay } from "../src/engine/types";

const play: Action = { play: { card: 3 } };
const activate: Action = { activate: { effect: 1 } };
const hero: Action = { attack: { attacker: 7, target: "hero" } };
const creature: Action = { attack: { attacker: 8, target: { creature: 4 } } };

describe("action guards", () => {
  test("each guard accepts exactly its own variant", () => {
    const all: Action[] = [play, activate, hero, creature, "concede"];
    expect(all.map(isPlay)).toEqual([true, false, false, false, false]);
    expect(all.map(isActivate)).toEqual([false, true, false, false, false]);
    expect(all.map(isAttack)).toEqual([false, false, true, true, false]);
    expect(all.map(isConcede)).toEqual([false, false, false, false, true]);
  });

  test("isAttackBy matches the attacker id", () => {
    expect([play, hero, creature, "concede" as Action].map(isAttackBy(7))).toEqual([false, true, false, false]);
  });

  test("isAttackOn matches the hero or one creature", () => {
    const all: Action[] = [play, hero, creature, "concede"];
    expect(all.map(isAttackOn("hero"))).toEqual([false, true, false, false]);
    expect(all.map(isAttackOn({ creature: 4 }))).toEqual([false, false, true, false]);
    expect(all.map(isAttackOn({ creature: 5 }))).toEqual([false, false, false, false]);
  });
});

test("eventKind is the single key of the event", () => {
  const e: EngineEvent = { died: { player: "One", creature: 2 } };
  expect(eventKind(e)).toBe("died");
});
