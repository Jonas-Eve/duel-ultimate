// #162 (story #126): the greedy computer, seen from the player's seat. A whole duel from the launch
// button to the end screen, the readability of the computer's turn (one single announcement at the end
// of its chain), determinism per seed, "Rejouer", a 360 x 640 screen and no client-side randomness.
// Production build (CSP, hashed .wasm), real WASM engine, real computer: nothing is mocked. Fixed seeds
// are written next to each scenario; the cases read labels of the DOM and never recompute a rule or a
// cost. Out of scope here, covered by the CLI tests of #160: the simulator's noise, `--policy`, the
// report and its reference measure.
import type { Page } from "@playwright/test";
import {
  SEED,
  endHeading,
  expect,
  expectNoHorizontalScroll,
  gameRegion,
  humanStep,
  openHome,
  ownHand,
  cardNames,
  playToEnd,
  readBlocks,
  readJournal,
  seedShown,
  settle,
  startGame,
  test,
  thinking,
} from "./support";

/** A human that only invokes, attacks and draws ends a game well under this (observed: ~11 actions). */
const MAX_HUMAN_ACTIONS = 300;
/** The seeds of the whole-duel cases (42 is the fixed seed of the suite, 7 and 1234 as in computer.e2e.ts). */
const SEEDS = [SEED, 7, 1234];
const DECKS = ["Ruée", "Rempart"] as const;
const ANNOUNCEMENT = /^L'ordinateur a joué (\d+) actions?\. À toi de jouer\.$/;
/** The lines that stand for one action of the computer (an effect, a card played, an attack); draws and hand changes are consequences. */
const COMPUTER_ACTION_LINE = /^L'ordinateur (invoque|utilise|attaque avec) /;

const endReason = (page: Page) => page.locator(".end-screen p").first().textContent();

/** The whole log of the end screen, as blocks of lines. */
const endJournal = (page: Page) => readBlocks(page.getByRole("region", { name: "Journal complet" }));

/** Plays one game to its end screen and returns what the player is shown there. */
async function playOneGame(page: Page): Promise<{ heading: string; reason: string; blocks: { title: string; lines: string[] }[] }> {
  await playToEnd(page, humanStep, MAX_HUMAN_ACTIONS);
  await expect(endHeading(page)).toHaveText(/^(Tu as gagné !|L'ordinateur a gagné\.)$/);
  return {
    heading: (await endHeading(page).textContent()) ?? "",
    reason: (await endReason(page)) ?? "",
    blocks: await endJournal(page),
  };
}

/**
 * Watches the page from the inside (a poll could miss a state of a few ms): the `aria-live` values of
 * the log bar while "L'ordinateur joue…" is displayed, the announcements (the polite `.announce`
 * paragraph) that appear while it is, and every distinct announcement text in order.
 */
const watchAnnouncements = (page: Page) =>
  page.addInitScript(() => {
    const seen = {
      thinkingShown: false,
      liveWhileThinking: [] as string[],
      announcedWhileThinking: 0,
      liveWhenAnnounced: [] as string[],
      announcements: [] as string[],
    };
    (window as unknown as { __watch: typeof seen }).__watch = seen;
    let last = "";
    new MutationObserver(() => {
      const isThinking = document.querySelector(".thinking") !== null;
      const live = document.querySelector('[role="log"]')?.getAttribute("aria-live") ?? "";
      const text = document.querySelector(".announce")?.textContent?.trim() ?? "";
      if (isThinking) {
        seen.thinkingShown = true;
        if (!seen.liveWhileThinking.includes(live)) seen.liveWhileThinking.push(live);
        if (text !== "") seen.announcedWhileThinking += 1;
      }
      if (text !== last) {
        if (text !== "") {
          seen.announcements.push(text);
          seen.liveWhenAnnounced.push(live);
        }
        last = text;
      }
    }).observe(document, { childList: true, subtree: true, attributes: true, characterData: true });
  });

interface Watched {
  thinkingShown: boolean;
  liveWhileThinking: string[];
  announcedWhileThinking: number;
  liveWhenAnnounced: string[];
  announcements: string[];
}
const watched = (page: Page) => page.evaluate(() => (window as unknown as { __watch: Watched }).__watch);

test.describe("1. Une partie entière contre l'ordinateur gourmand", () => {
  for (const deck of DECKS) {
    for (const seed of SEEDS) {
      // Ruée: the human opens at seed 42. Rempart: the computer (Ruée, initiative 3) opens at seed 42.
      test(`${deck}, graine ${seed} : jusqu'à l'écran de fin, sans blocage ni abandon de l'ordinateur`, async ({ page }) => {
        await openHome(page);
        await startGame(page, { deck, seed: String(seed) });
        const game = await playOneGame(page); // `settle` runs at every step: no "Partie interrompue", no alert
        await expect(endHeading(page)).toBeFocused();

        // A winner, a valid reason, and the computer never conceded.
        expect(game.reason, `seed ${seed}`).toMatch(
          /^(Le héros .* est tombé à 0 PV\.|.* n'avai(s|t) plus aucune action possible \(épuisement\)\.|Tu as abandonné\.)$/,
        );
        expect(game.reason).not.toMatch(/L'ordinateur a abandonné/);
        const lines = game.blocks.flatMap((b) => b.lines);
        expect(lines.filter((l) => /^L'ordinateur a abandonné/.test(l)), "the computer never concedes").toEqual([]);
        expect(lines.at(-1)).toMatch(game.heading === "Tu as gagné !" ? /^Tu gagnes : / : /^L'ordinateur gagne : /);
        // The computer did play: its segments exist, and each holds at least one action.
        const computerBlocks = game.blocks.filter((b) => b.title === "Tour de l'ordinateur");
        expect(computerBlocks.length, "the computer took its turns").toBeGreaterThan(0);
        for (const b of computerBlocks) expect(b.lines.some((l) => COMPUTER_ACTION_LINE.test(l))).toBe(true);
      });
    }
  }
});

test.describe("2. Le tour de l'ordinateur reste lisible : une seule annonce, à la fin de la chaîne", () => {
  for (const seed of SEEDS) {
    test(`Rempart, graine ${seed} : région vive coupée pendant la chaîne, une annonce par tour, N exact`, async ({ page }) => {
      await watchAnnouncements(page);
      await openHome(page);
      await startGame(page, { deck: "Rempart", seed: String(seed) }); // the computer opens
      const game = await playOneGame(page);
      const seen = await watched(page);

      expect(seen.thinkingShown, "« L'ordinateur joue… » must have been displayed").toBe(true);
      // During the chain the live region is off, and nothing is announced before its end.
      expect(seen.liveWhileThinking, "the log region is not live while the computer plays").toEqual(["off"]);
      expect(seen.announcedWhileThinking, "no announcement before the end of the chain").toBe(0);

      // One announcement per chain that handed the main back; a chain that ended the game has none.
      const chains = game.blocks.filter((b) => b.title === "Tour de l'ordinateur");
      const endedInChain = game.blocks.at(-1)!.title === "Tour de l'ordinateur";
      expect(seen.announcements.length, "one announcement per chain, never two").toBe(endedInChain ? chains.length - 1 : chains.length);
      // The announced count is the number of action lines the journal holds for that chain (read, not recomputed).
      seen.announcements.forEach((text, i) => {
        const m = ANNOUNCEMENT.exec(text);
        expect(m, `announcement ${i}: ${text}`).not.toBeNull();
        const actions = chains[i].lines.filter((l) => COMPUTER_ACTION_LINE.test(l)).length;
        expect(Number(m![1]), `chain ${i} of seed ${seed}`).toBe(actions);
      });
    });
  }

  test("Rempart, graine 42 : l'annonce est lue dans la région de partie, puis disparaît à l'action suivante", async ({ page }) => {
    await watchAnnouncements(page);
    await openHome(page);
    await startGame(page, { deck: "Rempart", seed: String(SEED) });
    await settle(page);
    const announcement = gameRegion(page).getByText(ANNOUNCEMENT);
    await expect(announcement).toHaveCount(1);
    // The log region took over again once the announcement was cleared by the human's action.
    expect(await humanStep(page)).toBe(true);
    await settle(page);
    await expect(page.getByRole("log", { name: "Dernière ligne du journal" })).toHaveAttribute("aria-live", /^(polite|off)$/);
    const seen = await watched(page);
    expect(seen.liveWhenAnnounced[0], "the log region is off while the announcement is read").toBe("off");
  });
});

test.describe("3. Déterminisme : même graine, mêmes choix humains, même journal", () => {
  for (const deck of DECKS) {
    test(`${deck}, graine ${SEED} : deux parties successives, journal complet et vainqueur identiques`, async ({ page }) => {
      await openHome(page);
      await startGame(page, { deck, seed: String(SEED) });
      const first = await playOneGame(page);

      await page.getByRole("button", { name: "Rejouer avec la même graine" }).tap();
      await expect(gameRegion(page)).toBeVisible();
      await expect(gameRegion(page).getByText(`Graine : ${SEED}`, { exact: true })).toBeVisible();
      const second = await playOneGame(page);

      expect(second.heading).toBe(first.heading);
      expect(second.reason).toBe(first.reason);
      expect(second.blocks, "the whole log is the same").toEqual(first.blocks);
    });
  }
});

test.describe("4. Rejouer et retour à l'accueil", () => {
  test("Ruée puis Rempart : chaque partie relancée est jouable", async ({ page }) => {
    await watchAnnouncements(page);
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    await playOneGame(page);

    // "Rejouer" (same seed): a playable game, the hand named, the human opens.
    await page.getByRole("button", { name: "Rejouer avec la même graine" }).tap();
    await expect(gameRegion(page)).toBeVisible();
    await settle(page);
    expect(await seedShown(page)).toBe(SEED);
    expect((await cardNames(ownHand(page))).length).toBeGreaterThan(0);
    expect(await humanStep(page)).toBe(true);
    await settle(page);
    await playOneGame(page);

    // "Rejouer" (new seed): another game, playable.
    await page.getByRole("button", { name: "Rejouer avec une nouvelle graine" }).tap();
    await expect(gameRegion(page)).toBeVisible();
    await settle(page);
    expect(await seedShown(page)).not.toBe(SEED);
    expect(await humanStep(page)).toBe(true);
    await settle(page);
    await playOneGame(page);

    // Home, then the other deck: the computer opens, plays, and hands the main back.
    await page.getByRole("button", { name: "Retour à l'accueil" }).tap();
    await expect(page.getByRole("heading", { name: "Choisis ton deck" })).toBeVisible();
    await startGame(page, { deck: "Rempart", seed: String(SEED) });
    await settle(page);
    await expect(gameRegion(page).getByText(ANNOUNCEMENT)).toBeVisible();
    expect((await cardNames(ownHand(page))).length).toBeGreaterThan(0);
    expect(await humanStep(page)).toBe(true);
    await settle(page);
    expect(await endHeading(page).count()).toBe(0);
    expect((await watched(page)).thinkingShown).toBe(true);
  });
});

test.describe("5. 360 x 640", () => {
  test("Rempart, graine 42 : indicateur vu puis disparu, journal ouvrable, aucun défilement horizontal", async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 640 });
    await watchAnnouncements(page);
    await openHome(page);
    await startGame(page, { deck: "Rempart", seed: String(SEED) });
    await settle(page);
    await expect(thinking(page)).toHaveCount(0);
    expect((await watched(page)).thinkingShown, "« L'ordinateur joue… » must have been displayed").toBe(true);
    await expect(gameRegion(page).getByText(ANNOUNCEMENT)).toBeVisible();
    await expectNoHorizontalScroll(page);

    const blocks = await readJournal(page);
    expect(blocks.map((b) => b.title)).toContain("Tour de l'ordinateur");
    await expectNoHorizontalScroll(page);

    // A few more rounds: after each human action the computer answers and the page stays within its width.
    for (let i = 0; i < 6; i++) {
      if (!(await humanStep(page))) break;
      await settle(page);
      await expectNoHorizontalScroll(page);
      if (await endHeading(page).count()) break;
    }
    if ((await endHeading(page).count()) === 0) await readJournal(page); // the drawer is still openable after several rounds
    await expectNoHorizontalScroll(page);
  });
});

test.describe("6. Aucun hasard côté client", () => {
  test("graine manuelle : Math.random n'est jamais appelé, ni par la page ni par l'ordinateur", async ({ page }) => {
    await page.addInitScript(() => {
      const spy = { calls: 0 };
      (window as unknown as { __random: typeof spy }).__random = spy;
      Math.random = () => {
        spy.calls += 1;
        return 0.5;
      };
    });
    const randomCalls = () => page.evaluate(() => (window as unknown as { __random: { calls: number } }).__random.calls);
    await openHome(page);
    // React DOM itself draws two random suffixes for its internal property names when the bundle loads
    // (not a game matter): the baseline is taken once the home screen is up, the game must add nothing.
    const atHome = await randomCalls();
    await startGame(page, { deck: "Rempart", seed: String(SEED) }); // the computer opens: its chain runs
    for (let i = 0; i < 12; i++) {
      await settle(page);
      if (await endHeading(page).count()) break;
      if (!(await humanStep(page))) break;
    }
    await settle(page);
    expect(await randomCalls(), "Math.random was called during a game with a manual seed").toBe(atHome);
  });
});
