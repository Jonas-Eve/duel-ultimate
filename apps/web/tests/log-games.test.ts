import { describe, expect, test } from "vitest";
import type { EngineEvent } from "../src/engine/types";
import { GameController } from "../src/game/controller";
import { formatBlocks, formatRecords, logContext } from "../src/ui/log-format";
import { config } from "./support/config";
import { decorateSession, newSession, PAIRS } from "./support/engine";
import { playHuman, randomPreview, realDeps } from "./support/policies";
import { makeController, useRealEngine } from "./support/ui";

useRealEngine();

const SEEDS = 20;

interface Played {
  controller: GameController;
  /** seq of the record -> the computer's real drawn cards, read on the REAL session (never through the controller). */
  drawnByComputer: Map<number, { id: number; name: string }[]>;
}

/** A full computer game. The decorator reads the computer's real hand after each apply, for the test only. */
async function playComputerGame(one: string, two: string, seed: number): Promise<Played> {
  const drawnByComputer = new Map<number, { id: number; name: string }[]>();
  let applies = 0;
  const controller = makeController(one, two, seed, {
    createSession: (r) =>
      decorateSession(newSession(r.deckOne, r.deckTwo, r.seed), {
        afterApply: (inner, events) => {
          applies += 1; // record seq = number of applies so far (seq 0 is the setup)
          const drew = events.filter((e): e is Extract<EngineEvent, { drew: unknown }> => "drew" in e && e.drew.player === "Two");
          if (drew.length === 0) return;
          const hand = inner.view("Two").you.hand;
          drawnByComputer.set(
            applies,
            drew.map((e) => {
              const card = hand.find((c) => c.id === e.drew.card);
              if (card?.name == null) throw new Error("the computer's drawn card is not in its real hand");
              return { id: e.drew.card, name: card.name };
            }),
          );
        },
      }).session,
  });
  controller.start();
  await playHuman(controller, randomPreview(seed));
  return { controller, drawnByComputer };
}

describe("full games against the computer (real WASM, random computer)", () => {
  test(`${SEEDS} seeds x 4 pairs: every line is built, the computer's hand is never named`, async () => {
    let computerDrawLines = 0;
    let namedAtPlayOnly = 0;
    let ripostes = 0;
    let heroRipostesSeen = 0;
    let humanDrawLines = 0;
    let chains = 0;

    for (const [one, two] of PAIRS) {
      for (let seed = 0; seed < SEEDS; seed++) {
        const { controller, drawnByComputer } = await playComputerGame(one, two, seed);
        const snap = controller.getSnapshot();
        const ctx = logContext(snap);
        const where = `${one} vs ${two}, seed ${seed}`;
        const lines = formatRecords(snap.records, ctx); // must not throw
        // The screen formats again at every render: a second pass must give the same text (no memory of names between passes).
        expect(formatRecords(snap.records, ctx), where).toEqual(lines);
        expect(lines.length, where).toBeGreaterThan(snap.records.length - 1);
        for (const l of lines) {
          expect(l.text, where).not.toMatch(/undefined|NaN|#\d/);
        }

        const drawnReal = new Map<number, string>();
        for (const record of snap.records) {
          const recLines = lines.filter((l) => l.seq === record.seq);
          const drewEvents = record.events.filter((e): e is Extract<EngineEvent, { drew: unknown }> => "drew" in e);
          const computerDrew = drewEvents.filter((e) => e.drew.player === "Two");

          // 1. The computer's draws say how many, never which.
          const drewLines = recLines.filter((l) => l.text.startsWith("L'ordinateur pioche"));
          let counted = 0;
          for (const l of drewLines) {
            const m = /^L'ordinateur pioche (\d+) cartes?\.$/.exec(l.text);
            expect(m, `${where}: ${l.text}`).not.toBeNull();
            counted += Number(m?.[1]);
            computerDrawLines += 1;
          }
          expect(counted, where).toBe(computerDrew.length);

          // 2. Against the REAL hand read on the inner session: none of those names is in the record's lines.
          const real = drawnByComputer.get(record.seq) ?? [];
          expect(real.length, where).toBe(computerDrew.length);
          for (const card of real) {
            drawnReal.set(card.id, card.name);
            for (const l of recLines) expect(l.text, `${where}: ${card.name} named at its draw`).not.toContain(card.name);
          }

          // 3. The human's draws are named.
          for (const l of recLines.filter((x) => x.text.startsWith("Tu pioches"))) {
            expect(l.text, where).toMatch(/ : /);
            humanDrawLines += 1;
          }

          // 4. A drawn card is named when it is summoned later, at the summoning only.
          for (const e of record.events) {
            if ("played" in e && e.played.player === "Two" && drawnReal.has(e.played.card)) {
              const line = recLines.find((l) => l.text.startsWith("L'ordinateur invoque"));
              expect(line?.text, where).toContain(drawnReal.get(e.played.card));
              namedAtPlayOnly += 1;
            }
          }

          // 5. A riposte line exists exactly when the engine emitted a riposte (never for riposte 0).
          // The match is on the damage wording: a hero named "... garde" would contain "de riposte".
          const riposteEvents = record.events.filter((e) => "riposted" in e).length;
          ripostes += riposteEvents;
          expect(recLines.filter((l) => /dégâts? de riposte/.test(l.text)).length, where).toBe(riposteEvents);
          // 5b. Likewise one line per riposte of the hero (#170), which destroys, without damage.
          const heroRipostes = record.events.filter((e) => "hero_riposted" in e).length;
          heroRipostesSeen += heroRipostes;
          expect(recLines.filter((l) => /riposte et détruit/.test(l.text)).length, where).toBe(heroRipostes);
        }

        // 6. The computer's chains are titled and grouped: one block per run of computer records.
        const runs = snap.records.filter((r, i) => r.by === "computer" && (i === 0 || snap.records[i - 1].by !== "computer")).length;
        const blocks = formatBlocks(snap.records, ctx);
        const computerBlocks = blocks.filter((b) => b.segment === "computer");
        expect(computerBlocks.length, where).toBe(runs);
        for (const b of computerBlocks) expect(b.title).toBe("Tour de l'ordinateur");
        for (const b of blocks) expect(b.lines.length, where).toBeGreaterThan(0);
        chains += runs;
        expect(blocks.flatMap((b) => b.lines), where).toEqual(lines);
      }
    }
    // The assertions above must have had something to bite on.
    expect(computerDrawLines).toBeGreaterThan(0);
    expect(humanDrawLines).toBeGreaterThan(0);
    expect(namedAtPlayOnly).toBeGreaterThan(0);
    expect(chains).toBeGreaterThan(0);
    // Riposte scenario by bounded search over the 80 games (Loup hérissé is the only riposte >= 1 card).
    expect(ripostes, "no riposte seen in 80 games: widen the search").toBeGreaterThan(0);
    expect(heroRipostesSeen, "no hero riposte seen in 80 games").toBeGreaterThan(0);
  }, 120_000);
});

describe("full games in two camps: everything is named", () => {
  test("4 pairs x 5 seeds: no 'une carte', every draw lists its cards, both seats in the third person", async () => {
    let draws = 0;
    for (const [one, two] of PAIRS) {
      for (let seed = 0; seed < 5; seed++) {
        const c = GameController.create({ ...config(one, two, seed), mode: "two-camps" }, realDeps());
        try {
          c.start();
          const { snapshot } = await playHuman(c, randomPreview(seed));
          const ctx = logContext(snapshot);
          expect(ctx.youSeat).toBeNull();
          const lines = formatRecords(snapshot.records, ctx);
          const all = lines.map((l) => l.text);
          for (const t of all) {
            expect(t).not.toMatch(/une carte|Une carte|undefined|#\d|\bTu\b|L'ordinateur/);
          }
          for (const t of all.filter((x) => / pioche /.test(x))) {
            expect(t).toMatch(/ pioche \d+ cartes? : /);
            draws += 1;
          }
          expect(all.some((t) => t.startsWith("Camp 1 — "))).toBe(true);
          expect(all.some((t) => t.startsWith("Camp 2 — "))).toBe(true);
          const titles = formatBlocks(snapshot.records, ctx).map((b) => b.title);
          expect(titles).toContain("Début de partie");
          expect(titles.some((t) => t.startsWith("Camp 1"))).toBe(true);
          expect(titles.some((t) => t.startsWith("Camp 2"))).toBe(true);
        } finally {
          c.dispose();
        }
      }
    }
    expect(draws).toBeGreaterThan(0);
  }, 120_000);
});
