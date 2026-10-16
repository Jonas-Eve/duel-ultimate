import { beforeAll, describe, expect, test } from "vitest";
import type { GameSession } from "../src/engine/session";
import type { EngineEvent, PlayerView, Seat } from "../src/engine/types";
import { eventKind } from "../src/engine/types";
import { captureRecord, type StepRecord } from "../src/game/records";
import { mulberry32 } from "../src/game/seed";
import { PAIRS, initTestEngine, newSession } from "./support/engine";

beforeAll(async () => {
  await initTestEngine();
});

const other = (s: Seat): Seat => (s === "One" ? "Two" : "One");

/** Ids an event cites, written independently of the implementation under test. */
function citedIds(e: EngineEvent): number[] {
  if ("played" in e) return [e.played.card];
  if ("drew" in e) return [e.drew.card];
  if ("attacked" in e) return [e.attacked.attacker];
  if ("creature_damaged" in e) return [e.creature_damaged.creature];
  if ("riposted" in e) return [e.riposted.creature];
  if ("died" in e) return [e.died.creature];
  return [];
}

interface Step {
  record: StepRecord;
  /** The computer's real hand (seat Two) right after the action, read off the real session. */
  handTwo: { id: number; name: string | null }[];
  viewsBefore: { One?: PlayerView; Two?: PlayerView };
}

const viewsFor = (s: GameSession, mode: "computer" | "two-camps") =>
  mode === "computer" ? { One: s.view("One") } : { One: s.view("One"), Two: s.view("Two") };

/** Plays a whole game with a seeded random choice for both seats, recording every step like the controller will. */
function playRecorded(deckOne: string, deckTwo: string, seed: number, mode: "computer" | "two-camps"): Step[] {
  const s = newSession(deckOne, deckTwo, seed);
  const rnd = mulberry32(seed ^ 0x5bd1e995);
  const steps: Step[] = [];
  try {
    let v = s.view("One");
    steps.push({
      record: captureRecord({ seq: 0, actor: v.active, by: "setup", events: [...s.startEvents], views: viewsFor(s, mode) }),
      handTwo: s.view("Two").you.hand.map((c) => ({ id: c.id, name: c.name })),
      viewsBefore: {},
    });
    for (let seq = 1; v.result === null && seq < 2000; seq++) {
      const legal = s.legalActions().filter((a) => a !== "concede");
      const action = legal[Math.floor(rnd() * legal.length)];
      const actor = v.active;
      const viewsBefore = viewsFor(s, mode);
      const events = s.apply(action);
      const by = mode === "computer" && actor === "Two" ? "computer" : "human";
      steps.push({
        record: captureRecord({ seq, actor, by, events, views: viewsFor(s, mode) }),
        handTwo: s.view("Two").you.hand.map((c) => ({ id: c.id, name: c.name })),
        viewsBefore,
      });
      v = s.view("One");
    }
    expect(v.result, "the game must end").not.toBeNull();
    return steps;
  } finally {
    s.dispose();
  }
}

describe("completeness", () => {
  test.each(PAIRS)("two camps %s/%s: every id cited by a step is named in its record", (one, two) => {
    for (let seed = 0; seed < 5; seed++) {
      for (const { record } of playRecorded(one, two, seed, "two-camps")) {
        for (const e of record.events) {
          for (const id of citedIds(e)) {
            expect(record.cards[id], `seed ${seed} seq ${record.seq} ${eventKind(e)} #${id}`).toBeDefined();
            expect(typeof record.cards[id].name).toBe("string");
          }
        }
      }
    }
  });

  test("cards holds only ids cited by the events of the step", () => {
    for (const { record } of playRecorded("rush", "bulwark", 4, "two-camps")) {
      const cited = new Set(record.events.flatMap(citedIds));
      for (const key of Object.keys(record.cards)) expect(cited.has(Number(key))).toBe(true);
    }
  });

  test("the setup record has seq 0, by setup, the actor, the start events and both heroes", () => {
    const [first] = playRecorded("rush", "bulwark", 4, "computer");
    expect(first.record.seq).toBe(0);
    expect(first.record.by).toBe("setup");
    expect(first.record.actor).toBe("One");
    expect(first.record.events.map(eventKind)).toContain("game_started");
    expect(first.record.hero.One.name).toBe("Capitaine d'avant-garde");
    expect(first.record.hero.Two.health).toBeGreaterThan(0);
    expect(first.record.hero.One.effects.map((e) => e.name)).toEqual(["Rappel", "Renforts"]);
  });
});

describe("confidentiality by construction (against the computer)", () => {
  test("20 full games over 4 pairs: nothing of the computer's hand is in a record", () => {
    let games = 0;
    let computerDrews = 0;
    for (const [one, two] of PAIRS) {
      for (let seed = 0; seed < 5; seed++) {
        games += 1;
        for (const { record, handTwo } of playRecorded(one, two, seed, "computer")) {
          const handIds = new Set(handTwo.map((c) => c.id));
          for (const key of Object.keys(record.cards)) {
            expect(handIds.has(Number(key)), `${one}/${two} seed ${seed} seq ${record.seq}: #${key} is in the computer's hand`).toBe(false);
          }
          for (const e of record.events) {
            if ("drew" in e && e.drew.player === "Two") {
              computerDrews += 1;
              expect(record.cards[e.drew.card], "a computer draw has no entry").toBeUndefined();
            }
          }
        }
      }
    }
    expect(games).toBe(20);
    expect(computerDrews).toBeGreaterThan(0);
  });

  test("a card the computer plays is named, because it is now on the public board", () => {
    let played = 0;
    for (const { record } of playRecorded("bulwark", "rush", 3, "computer")) {
      for (const e of record.events) {
        if ("played" in e && e.played.player === "Two") {
          played += 1;
          expect(typeof record.cards[e.played.card]?.name, `seq ${record.seq}`).toBe("string");
        }
      }
    }
    expect(played).toBeGreaterThan(0);
  });

  test("the human's draw is named, the computer's is not (same game)", () => {
    let human = 0;
    for (const { record } of playRecorded("rush", "bulwark", 6, "computer")) {
      for (const e of record.events) {
        if ("drew" in e && e.drew.player === "One") {
          human += 1;
          expect(typeof record.cards[e.drew.card]?.name).toBe("string");
        }
      }
    }
    expect(human).toBeGreaterThan(0);
  });
});

describe("names and health are read AFTER the action", () => {
  test("a creature that died in the action is named from the public discard with health 0", () => {
    let deaths = 0;
    for (const [one, two] of PAIRS) {
      for (const { record } of playRecorded(one, two, 2, "two-camps")) {
        for (const e of record.events) {
          if ("died" in e) {
            deaths += 1;
            const note = record.cards[e.died.creature];
            expect(note, `seq ${record.seq}`).toBeDefined();
            expect(note.health).toBe(0);
            expect(typeof note.name).toBe("string");
          }
        }
      }
    }
    expect(deaths).toBeGreaterThan(0);
  });

  test("a damaged survivor shows its remaining health", () => {
    let seen = 0;
    for (const { record } of playRecorded("bulwark", "bulwark", 8, "two-camps")) {
      for (const e of record.events) {
        if ("creature_damaged" in e && !record.events.some((x) => "died" in x && x.died.creature === e.creature_damaged.creature)) {
          seen += 1;
          expect(record.cards[e.creature_damaged.creature].health).toBeGreaterThan(0);
        }
      }
    }
    expect(seen).toBeGreaterThan(0);
  });

  test("a lethal attack on a hero leaves that hero at 0 in the record", () => {
    let lethal = 0;
    for (const [one, two] of PAIRS) {
      const steps = playRecorded(one, two, 1, "two-camps");
      const last = steps[steps.length - 1].record;
      const won = last.events.find((e) => "game_won" in e);
      if (won && "game_won" in won && won.game_won.reason === "hero_down") {
        lethal += 1;
        expect(last.hero[other(won.game_won.winner)].health).toBe(0);
        expect(last.hero[won.game_won.winner].health).toBeGreaterThan(0);
      }
    }
    expect(lethal).toBeGreaterThan(0);
  });
});

describe("heroes and views", () => {
  test("hero is filed by seat from the first view available, in both modes", () => {
    const s = newSession("rush", "bulwark", 2);
    try {
      const one = s.view("One");
      const two = s.view("Two");
      const both = captureRecord({ seq: 0, actor: "One", by: "setup", events: [], views: { One: one, Two: two } });
      const ownly = captureRecord({ seq: 0, actor: "One", by: "setup", events: [], views: { One: one } });
      const onlyTwo = captureRecord({ seq: 0, actor: "One", by: "setup", events: [], views: { Two: two } });
      for (const r of [both, ownly, onlyTwo]) {
        expect(r.hero.One.name).toBe(one.you.hero.name);
        expect(r.hero.Two.name).toBe(two.you.hero.name);
        expect(r.hero.One.health).toBe(one.you.hero.health);
        expect(r.hero.One.effects).toEqual(one.you.hero.effects);
      }
    } finally {
      s.dispose();
    }
  });

  test("with no view at all it throws instead of inventing heroes", () => {
    expect(() => captureRecord({ seq: 0, actor: "One", by: "setup", events: [], views: {} })).toThrow();
  });
});

describe("exhaustive on event kinds", () => {
  test("an unknown event is a loud error, never silently ignored", () => {
    const s = newSession("rush", "bulwark", 2);
    try {
      const unknown = { teleported: { player: "One" } } as unknown as EngineEvent;
      expect(() => captureRecord({ seq: 1, actor: "One", by: "human", events: [unknown], views: { One: s.view("One") } })).toThrow(/teleported/);
    } finally {
      s.dispose();
    }
  });

  test("every known kind is accepted", () => {
    const s = newSession("rush", "bulwark", 2);
    try {
      const v = s.view("One");
      const id = v.you.hand[0].id;
      const events: EngineEvent[] = [
        { game_started: { first: "One", budget: 3 } },
        { effect_activated: { player: "One", effect: 0, cost: 2 } },
        { drew: { player: "One", card: id } },
        { played: { player: "One", card: id, cost: 1 } },
        { attacked: { player: "One", attacker: id, cost: 1 } },
        { damaged: { player: "One", amount: 1 } },
        { creature_damaged: { player: "One", creature: id, amount: 1 } },
        { riposted: { player: "One", creature: id, amount: 1 } },
        { died: { player: "One", creature: id } },
        { token_moved: { player: "One", budget: 1 } },
        { active_player_changed: { player: "Two" } },
        { game_won: { winner: "One", reason: "concede" } },
      ];
      const r = captureRecord({ seq: 1, actor: "One", by: "human", events, views: { One: v } });
      expect(r.cards[id].name).toBe(v.you.hand[0].name);
    } finally {
      s.dispose();
    }
  });
});
