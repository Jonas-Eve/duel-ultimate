import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "vitest";
import { EVENT_KINDS, eventKind, type EngineEvent, type Seat } from "../src/engine/types";
import type { StepRecord } from "../src/game/records";
import { formatBlocks, formatRecords, type LogContext } from "../src/ui/log-format";
import { repoRoot, webRoot } from "./support/paths";

const COMPUTER: LogContext = { mode: "computer", youSeat: "One", labels: { One: "Toi", Two: "L'ordinateur" } };
const CAMPS: LogContext = { mode: "two-camps", youSeat: null, labels: { One: "Camp 1 — Ruée", Two: "Camp 2 — Rempart" } };

// Ids chosen so that a raw id leaking into a text is recognisable.
const ID = { a: 9071, b: 9072, c: 9073 };

function rec(events: EngineEvent[], over: Partial<StepRecord> = {}): StepRecord {
  return {
    seq: 1,
    actor: "One",
    by: "human",
    events,
    cards: {
      [ID.a]: { name: "Pillard", health: 2 },
      [ID.b]: { name: "Lancier", health: 0 },
      [ID.c]: { name: "Cavalier", health: 3 },
    },
    hero: {
      One: { name: "Ruée", health: 20, effects: [{ name: "Piocher", cost: 2, effect: { kind: "draw", count: 1 } }] },
      Two: { name: "Rempart", health: 17, effects: [] },
    },
    ...over,
  };
}
const texts = (r: StepRecord, ctx = COMPUTER) => formatRecords([r], ctx).map((l) => l.text);

describe("one line per event kind", () => {
  const SAMPLES: Record<(typeof EVENT_KINDS)[number], { event: EngineEvent; you: string }> = {
    game_started: {
      event: { game_started: { first: "One", budget: 3 } },
      you: "La partie commence : Tu as la main (temps 3).",
    },
    effect_activated: {
      event: { effect_activated: { player: "One", effect: 0, cost: 2 } },
      you: "Tu utilises Piocher (coût 2).",
    },
    drew: { event: { drew: { player: "One", card: ID.a } }, you: "Tu pioches 1 carte : Pillard." },
    played: { event: { played: { player: "One", card: ID.a, cost: 2 } }, you: "Tu invoques Pillard (coût 2)." },
    attacked: {
      event: { attacked: { player: "One", attacker: ID.a, cost: 1 } },
      you: "Tu attaques avec Pillard (coût 1).",
    },
    damaged: {
      event: { damaged: { player: "Two", amount: 3 } },
      you: "Le héros Rempart subit 3 dégâts (PV restants : 17).",
    },
    creature_damaged: {
      event: { creature_damaged: { player: "Two", creature: ID.a, amount: 1 } },
      you: "Pillard subit 1 dégât (PV restants : 2).",
    },
    riposted: {
      event: { riposted: { player: "One", creature: ID.c, amount: 2 } },
      you: "Cavalier subit 2 dégâts de riposte (PV restants : 3).",
    },
    hero_riposted: {
      // The attacker `Cavalier` of One hit the hero of Two (Rempart): the hero hits back.
      event: { hero_riposted: { player: "One", creature: ID.c } },
      you: "Le héros Rempart riposte et détruit Cavalier.",
    },
    died: { event: { died: { player: "Two", creature: ID.b } }, you: "Lancier meurt." },
    token_moved: {
      event: { token_moved: { player: "One", budget: 4 } },
      you: "Tu gardes la main (temps restant 4).",
    },
    active_player_changed: {
      event: { active_player_changed: { player: "Two" } },
      you: "La main passe à L'ordinateur.",
    },
    game_won: {
      event: { game_won: { winner: "One", reason: "hero_down" } },
      you: "Tu gagnes : Le héros Rempart est tombé à 0 PV.",
    },
  };

  test.each(EVENT_KINDS)("%s", (kind) => {
    const s = SAMPLES[kind];
    expect(eventKind(s.event)).toBe(kind);
    expect(texts(rec([s.event]))).toEqual([s.you]);
    // The same event of the other seat is worded in the third person.
    const flipped = JSON.parse(JSON.stringify(s.event).replace(/"One"/g, '"Tmp"').replace(/"Two"/g, '"One"').replace(/"Tmp"/g, '"Two"')) as EngineEvent;
    expect(texts(rec([flipped])).length).toBe(1);
  });

  test("the third person for the other seat", () => {
    expect(texts(rec([{ played: { player: "Two", card: ID.a, cost: 2 } }]))).toEqual(["L'ordinateur invoque Pillard (coût 2)."]);
    expect(texts(rec([{ effect_activated: { player: "Two", effect: 0, cost: 2 } }]))).toEqual(["L'ordinateur utilise un effet (coût 2)."]);
    expect(texts(rec([{ attacked: { player: "Two", attacker: ID.a, cost: 1 } }]))).toEqual(["L'ordinateur attaque avec Pillard (coût 1)."]);
    expect(texts(rec([{ game_won: { winner: "Two", reason: "concede" } }]))).toEqual(["L'ordinateur gagne : Tu as abandonné."]);
  });

  test("a hero effect is named from the record's hero effects", () => {
    expect(texts(rec([{ effect_activated: { player: "One", effect: 0, cost: 2 } }]))).toEqual(["Tu utilises Piocher (coût 2)."]);
  });
});

describe("merges", () => {
  test("consecutive draws of one player are one line, with plural and names", () => {
    const events: EngineEvent[] = [
      { drew: { player: "One", card: ID.a } },
      { drew: { player: "One", card: ID.b } },
      { drew: { player: "One", card: ID.c } },
    ];
    expect(texts(rec(events))).toEqual(["Tu pioches 3 cartes : Pillard, Lancier, Cavalier."]);
  });

  test("draws without names (the computer's) say how many, never which", () => {
    const events: EngineEvent[] = [{ drew: { player: "Two", card: 5001 } }, { drew: { player: "Two", card: 5002 } }];
    expect(texts(rec(events, { actor: "Two", by: "computer", cards: {} }))).toEqual(["L'ordinateur pioche 2 cartes."]);
    expect(texts(rec([events[0]], { actor: "Two", by: "computer", cards: {} }))).toEqual(["L'ordinateur pioche 1 carte."]);
  });

  test("draws of two different players are not merged", () => {
    const events: EngineEvent[] = [{ drew: { player: "One", card: ID.a } }, { drew: { player: "Two", card: ID.b } }];
    expect(texts(rec(events))).toHaveLength(2);
  });

  test("token_moved followed by active_player_changed is 'la main passe', alone it is 'garde la main'", () => {
    expect(
      texts(rec([{ token_moved: { player: "Two", budget: 3 } }, { active_player_changed: { player: "Two" } }])),
    ).toEqual(["La main passe à L'ordinateur (temps 3)."]);
    expect(
      texts(rec([{ token_moved: { player: "One", budget: 3 } }, { active_player_changed: { player: "One" } }])),
    ).toEqual(["La main passe à toi (temps 3)."]);
    expect(texts(rec([{ token_moved: { player: "One", budget: 2 } }]))).toEqual(["Tu gardes la main (temps restant 2)."]);
  });
});

describe("game_won carries the right reason", () => {
  const won = (winner: Seat, reason: "hero_down" | "exhaustion" | "concede", ctx = COMPUTER) => texts(rec([{ game_won: { winner, reason } }]), ctx)[0];
  test("the three reasons, from both sides, none swapped", () => {
    expect(won("One", "hero_down")).toBe("Tu gagnes : Le héros Rempart est tombé à 0 PV.");
    expect(won("Two", "hero_down")).toBe("L'ordinateur gagne : Le héros Ruée est tombé à 0 PV.");
    expect(won("One", "exhaustion")).toBe("Tu gagnes : L'ordinateur n'avait plus aucune action possible (épuisement).");
    expect(won("Two", "exhaustion")).toBe("L'ordinateur gagne : Tu n'avais plus aucune action possible (épuisement).");
    expect(won("One", "concede")).toBe("Tu gagnes : L'ordinateur a abandonné.");
    expect(won("Two", "concede")).toBe("L'ordinateur gagne : Tu as abandonné.");
  });
  test("two camps", () => {
    expect(won("One", "concede", CAMPS)).toBe("Camp 1 — Ruée gagne : Camp 2 — Rempart a abandonné.");
    expect(won("Two", "exhaustion", CAMPS)).toBe("Camp 2 — Rempart gagne : Camp 1 — Ruée n'avait plus aucune action possible (épuisement).");
  });
});

describe("no raw identifier, no undefined", () => {
  test("a missing name reads 'une carte', everywhere a card is cited", () => {
    const events: EngineEvent[] = [
      { played: { player: "Two", card: 7771, cost: 2 } },
      { attacked: { player: "Two", attacker: 7771, cost: 1 } },
      { creature_damaged: { player: "One", creature: 7772, amount: 1 } },
      { riposted: { player: "Two", creature: 7771, amount: 1 } },
      { hero_riposted: { player: "Two", creature: 7771 } },
      { died: { player: "One", creature: 7772 } },
      { drew: { player: "One", card: 7773 } },
    ];
    const all = texts(rec(events, { cards: {} }));
    expect(all).toEqual([
      "L'ordinateur invoque une carte (coût 2).",
      "L'ordinateur attaque avec une carte (coût 1).",
      "Une carte subit 1 dégât (PV restants : 0).",
      "Une carte subit 1 dégât de riposte (PV restants : 0).",
      "Le héros Ruée riposte et détruit une carte.",
      "Une carte meurt.",
      "Tu pioches 1 carte.",
    ]);
    for (const t of all) {
      expect(t).not.toMatch(/#\d/);
      expect(t).not.toMatch(/777\d/);
      expect(t).not.toMatch(/undefined|NaN|null/);
    }
  });

  test("a hero effect index out of range does not show an index", () => {
    expect(texts(rec([{ effect_activated: { player: "One", effect: 4, cost: 2 } }]))[0]).toBe("Tu utilises un effet (coût 2).");
  });
});

describe("French text only", () => {
  test("no engine key leaks into a line", () => {
    const events: EngineEvent[] = EVENT_KINDS.map((k) => {
      switch (k) {
        case "game_started":
          return { game_started: { first: "One", budget: 3, data_versions: ["starter@1.1.0"] } };
        case "effect_activated":
          return { effect_activated: { player: "One", effect: 0, cost: 2 } };
        case "drew":
          return { drew: { player: "One", card: ID.a } };
        case "played":
          return { played: { player: "One", card: ID.a, cost: 2 } };
        case "attacked":
          return { attacked: { player: "One", attacker: ID.a, cost: 1 } };
        case "damaged":
          return { damaged: { player: "One", amount: 1 } };
        case "creature_damaged":
          return { creature_damaged: { player: "One", creature: ID.a, amount: 1 } };
        case "riposted":
          return { riposted: { player: "One", creature: ID.a, amount: 1 } };
        case "hero_riposted":
          return { hero_riposted: { player: "One", creature: ID.a } };
        case "died":
          return { died: { player: "One", creature: ID.a } };
        case "token_moved":
          return { token_moved: { player: "One", budget: 1 } };
        case "active_player_changed":
          return { active_player_changed: { player: "One" } };
        case "game_won":
          return { game_won: { winner: "One", reason: "concede" } };
      }
    });
    for (const ctx of [COMPUTER, CAMPS]) {
      const all = texts(rec(events), ctx).join("\n");
      for (const word of ["hero_down", "exhaustion", "concede", "token_moved", "game_started", "creature_damaged", "starter@"]) {
        expect(all).not.toContain(word);
      }
    }
  });
});

describe("both contexts", () => {
  test("two camps: third person for both seats and every card named", () => {
    const events: EngineEvent[] = [
      { drew: { player: "Two", card: ID.a } },
      { played: { player: "One", card: ID.b, cost: 2 } },
      { token_moved: { player: "Two", budget: 3 } },
      { active_player_changed: { player: "Two" } },
    ];
    expect(texts(rec(events), CAMPS)).toEqual([
      "Camp 2 — Rempart pioche 1 carte : Pillard.",
      "Camp 1 — Ruée invoque Lancier (coût 2).",
      "La main passe à Camp 2 — Rempart (temps 3).",
    ]);
  });

  test("lines carry seq, segment, actor, a tone and a unique id", () => {
    const r = rec([{ played: { player: "One", card: ID.a, cost: 2 } }, { damaged: { player: "Two", amount: 3 } }, { died: { player: "Two", creature: ID.b } }], {
      seq: 7,
    });
    const lines = formatRecords([r], COMPUTER);
    expect(lines.map((l) => l.tone)).toEqual(["info", "damage", "death"]);
    expect(lines.every((l) => l.seq === 7 && l.segment === "human" && l.actor === "One")).toBe(true);
    expect(new Set(lines.map((l) => l.id)).size).toBe(3);
  });
});

describe("blocks", () => {
  const human = (seq: number, actor: Seat) => rec([{ played: { player: actor, card: ID.a, cost: 1 } }], { seq, actor, by: "human" });
  const computer = (seq: number) => rec([{ played: { player: "Two", card: ID.a, cost: 1 } }], { seq, actor: "Two", by: "computer" });
  const setup = rec([{ game_started: { first: "One", budget: 3 } }], { seq: 0, by: "setup" });

  test("against the computer: start, your turn, the computer's chain (one block), your turn", () => {
    const blocks = formatBlocks([setup, human(1, "One"), human(2, "One"), computer(3), computer(4), human(5, "One")], COMPUTER);
    expect(blocks.map((b) => b.title)).toEqual(["Début de partie", "Ton tour", "Tour de l'ordinateur", "Ton tour"]);
    expect(blocks.map((b) => b.lines.length)).toEqual([1, 2, 2, 1]);
    expect(blocks[2].segment).toBe("computer");
  });

  test("two camps: by is always 'human', only the actor splits the blocks", () => {
    const blocks = formatBlocks([setup, human(1, "One"), human(2, "One"), human(3, "Two"), human(4, "One")], CAMPS);
    expect(blocks.map((b) => b.title)).toEqual(["Début de partie", "Camp 1 — Ruée", "Camp 2 — Rempart", "Camp 1 — Ruée"]);
    expect(blocks.map((b) => b.lines.length)).toEqual([1, 2, 1, 1]);
  });

  test("the blocks hold exactly the lines of formatRecords", () => {
    const rs = [setup, human(1, "One"), computer(2)];
    expect(formatBlocks(rs, COMPUTER).flatMap((b) => b.lines)).toEqual(formatRecords(rs, COMPUTER));
  });
});

describe("exhaustiveness against the Rust enum", () => {
  const rust = readFileSync(join(repoRoot, "crates/engine/src/rules.rs"), "utf8");
  const body = /pub enum Event \{([\s\S]*?)\n\}/.exec(rust)?.[1] ?? "";
  const snake = (s: string) => s.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
  // Variants sit at one indent level: `    Name {` or `    Name,`.
  const variants = [...body.matchAll(/^ {4}([A-Z][A-Za-z]+)\s*[{,]/gm)].map((m) => snake(m[1]));
  const source = readFileSync(join(webRoot, "src/ui/log-format.ts"), "utf8");

  test("the Rust enum is parsed", () => {
    expect(variants.length).toBeGreaterThanOrEqual(12);
    expect([...variants].sort()).toEqual([...EVENT_KINDS].sort());
  });

  test("every variant has a branch of the formatter", () => {
    for (const v of variants) expect(source, `no case for ${v}`).toContain(`case "${v}":`);
  });

  test("an unknown event is a loud error, not a silent line", () => {
    const bad = { teleported: { player: "One" } } as unknown as EngineEvent;
    expect(() => formatRecords([rec([bad])], COMPUTER)).toThrow(/unknown engine event/);
  });
});
