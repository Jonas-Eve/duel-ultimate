// Cases 1 to 3 of #68: a whole duel against the computer, the human as the Rempart deck, and the
// time announced before an action against the time it really costs. Production build, real WASM
// engine, real computer: nothing is mocked. Fixed seeds are written next to each scenario; if the
// starter data change and a seed stops producing the situation, the test fails loudly.
import {
  SEED,
  cardNames,
  endHeading,
  expect,
  gameRegion,
  humanStep,
  isOver,
  openHome,
  ownHand,
  playToEnd,
  readBlocks,
  readJournal,
  seedShown,
  settle,
  startGame,
  test,
  thinking,
  timeBand,
} from "./support";

const REMPART_CARDS = ["Piquier", "Chevalier", "Ogre", "Garde du rempart", "Colosse"];
const INVALID_SEED_TEXT = "La graine doit être un nombre entier entre 0 et 4 294 967 295.";
/** A human that only invokes, attacks and draws ends a game well under this (observed: ~11 actions). */
const MAX_HUMAN_ACTIONS = 300;

test.describe("1. Contre l'ordinateur, Ruée", () => {
  test("accueil : version des données, graine invalide refusée, graine vide acceptée", async ({ page }) => {
    await openHome(page);
    await expect(page.getByText("Données : starter@1.0.0", { exact: true })).toBeVisible();

    for (const bad of ["abc", "-1", "1.5", "4294967296", "12 34"]) {
      await page.getByLabel("Graine (optionnelle)").fill(bad);
      await page.getByRole("button", { name: "Lancer la partie" }).tap();
      await expect(page.getByRole("alert")).toHaveText(INVALID_SEED_TEXT);
      await expect(page.getByRole("region", { name: "Partie" })).toHaveCount(0);
      await expect(page.getByLabel("Graine (optionnelle)")).toBeFocused();
    }

    await page.getByLabel("Graine (optionnelle)").fill("");
    await page.getByRole("button", { name: "Lancer la partie" }).tap();
    await expect(gameRegion(page)).toBeVisible();
    expect(await seedShown(page)).toBeGreaterThanOrEqual(0); // a drawn seed is displayed
    await gameRegion(page).getByText("Infos de partie").tap();
    await expect(gameRegion(page).getByText("Données : starter@1.0.0")).toBeVisible();
  });

  test("toutes les informations permanentes, aucune main adverse en clair", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) }); // seed 42: the human (Ruée, initiative 3) opens
    const game = gameRegion(page);

    await expect(game.getByText(`Graine : ${SEED}`, { exact: true })).toBeVisible();
    const mine = page.getByRole("group", { name: "Ton héros" });
    await expect(mine).toContainText("Capitaine d'avant-garde");
    await expect(mine).toContainText(/PV \d+/);
    await expect(mine).toContainText(/Main : \d+/);
    await expect(mine).toContainText(/Pioche : \d+/);
    await expect(mine.getByRole("button", { name: "Défausse (0)" })).toBeVisible();
    const theirs = page.getByRole("group", { name: "Héros adverse" });
    await expect(theirs).toContainText("Gardienne du rempart");
    await expect(theirs).toContainText(/PV \d+/);
    await expect(theirs).toContainText(/Main : \d+/);
    await expect(theirs).toContainText(/Pioche : \d+/);
    await expect(theirs.getByRole("button", { name: "Défausse (0)" })).toBeVisible();

    const time = timeBand(page);
    await expect(time).toContainText("À toi");
    await expect(time).toContainText(/Temps : \d+/);
    await expect(time).toContainText(/max \d+/);
    await expect(page.getByRole("group", { name: "Ton plateau" })).toContainText("Aucune créature");
    await expect(page.getByRole("group", { name: "Plateau adverse" })).toContainText("Aucune créature");
    await expect(page.getByRole("log", { name: "Dernière ligne du journal" })).toContainText("La partie commence");

    // The hand is named, and its size is the one the counter announces.
    const names = await cardNames(ownHand(page));
    expect(names.length).toBeGreaterThan(0);
    await expect(mine).toContainText(`Main : ${names.length}`);
    // The opponent area carries counters only: no hand region, and not one card of its deck is printed.
    await expect(page.getByRole("group", { name: /main/i })).toHaveCount(1);
    const screen = (await page.locator("main").innerText()) + (await page.locator("main").innerHTML());
    for (const name of REMPART_CARDS) expect(screen, `the opponent's card "${name}" must not be shown`).not.toContain(name);
  });

  test("un duel entier, écran de fin, rejouer, accueil", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    const firstHand = await cardNames(ownHand(page));

    await playToEnd(page, humanStep, MAX_HUMAN_ACTIONS);
    // End screen: winner, reason, both seeds, the whole log.
    await expect(endHeading(page)).toBeFocused();
    await expect(endHeading(page)).toHaveText(/^(Tu as gagné !|L'ordinateur a gagné\.)$/);
    const end = page.getByRole("region").filter({ has: endHeading(page) });
    await expect(end.getByText(/^(Le héros .* est tombé à 0 PV\.|.* n'avai(s|t) plus aucune action possible \(épuisement\)\.|.* a(s)? abandonné\.)$/)).toBeVisible();
    await expect(end.getByText(`Graine : ${SEED}`, { exact: true })).toBeVisible();
    await expect(end.getByText(/^Graine de l'ordinateur : \d+$/)).toBeVisible();
    const blocks = await readBlocks(end.getByRole("region", { name: "Journal complet" }));
    expect(blocks[0].title).toBe("Début de partie");
    expect(blocks[0].lines[0]).toMatch(/^La partie commence : /);
    expect(blocks.map((b) => b.title)).toContain("Ton tour");
    const last = blocks.at(-1)!.lines.at(-1)!;
    const youWon = (await endHeading(page).textContent()) === "Tu as gagné !";
    expect(last).toMatch(youWon ? /^Tu gagnes : / : /^L'ordinateur gagne : /);

    // Same seed: the same starting hand.
    await page.getByRole("button", { name: "Rejouer avec la même graine" }).tap();
    await expect(gameRegion(page)).toBeVisible();
    await expect(gameRegion(page).getByText(`Graine : ${SEED}`, { exact: true })).toBeVisible();
    expect(await cardNames(ownHand(page))).toEqual(firstHand);

    // New seed: a new game, another seed.
    await playToEnd(page, humanStep, MAX_HUMAN_ACTIONS);
    await page.getByRole("button", { name: "Rejouer avec une nouvelle graine" }).tap();
    await expect(gameRegion(page)).toBeVisible();
    expect(await seedShown(page)).not.toBe(SEED);
    await expect(page.getByRole("log", { name: "Dernière ligne du journal" })).toContainText("La partie commence");
    expect(await isOver(page)).toBe(false);

    // Home.
    await playToEnd(page, humanStep, MAX_HUMAN_ACTIONS);
    await page.getByRole("button", { name: "Retour à l'accueil" }).tap();
    await expect(page.getByRole("heading", { name: "Choisis ton deck" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Lancer la partie" })).toBeEnabled();
  });
});

test.describe("2. Humain Rempart : l'ordinateur ouvre", () => {
  /** Watches the page from the inside: a poll could miss a 5 ms "L'ordinateur joue…". */
  const watchThinking = (page: import("@playwright/test").Page) =>
    page.addInitScript(() => {
      const seen = { shown: false, activeControls: 0 };
      (window as unknown as { __thinking: typeof seen }).__thinking = seen;
      new MutationObserver(() => {
        if (document.querySelector(".thinking") === null) return;
        seen.shown = true;
        // While the computer plays, no hand card, hero effect nor "Abandonner" may be enabled.
        seen.activeControls += document.querySelectorAll(".hand button:not([disabled]), .actions button:not([disabled])").length;
      }).observe(document, { childList: true, subtree: true, attributes: true });
    });
  const watched = (page: import("@playwright/test").Page) =>
    page.evaluate(() => (window as unknown as { __thinking: { shown: boolean; activeControls: number } }).__thinking);

  test("indicateur vu puis disparu, main rendue, commandes inactives, journal sans nom pioché", async ({ page }) => {
    await watchThinking(page);
    await openHome(page);
    await startGame(page, { deck: "Rempart", seed: String(SEED) }); // the computer (Ruée, initiative 3) opens
    await settle(page);

    const seen = await watched(page);
    expect(seen.shown, "« L'ordinateur joue… » must have been displayed").toBe(true);
    expect(seen.activeControls, "no control may be active during the computer's turn").toBe(0);
    await expect(thinking(page)).toHaveCount(0);
    await expect(gameRegion(page).getByText(/^L'ordinateur a joué \d+ actions?\. À toi de jouer\.$/)).toBeVisible();

    // The human's hand is back, named and playable; the token is theirs.
    expect((await cardNames(ownHand(page))).length).toBeGreaterThan(0);
    await expect(ownHand(page).locator("button:not([disabled])").first()).toBeVisible();
    await expect(page.getByRole("button", { name: "Abandonner" })).toBeEnabled();
    await expect(timeBand(page)).toContainText("À toi");

    // The journal has a segment of the computer, and the computer's draws name no card.
    const blocks = await readJournal(page);
    const titles = blocks.map((b) => b.title);
    expect(titles).toContain("Début de partie");
    expect(titles).toContain("Tour de l'ordinateur");
    const computerLines = blocks.filter((b) => b.title === "Tour de l'ordinateur").flatMap((b) => b.lines);
    const draws = computerLines.filter((l) => l.startsWith("L'ordinateur pioche"));
    expect(draws.length, "seed 42: the computer is expected to draw in its first turn").toBeGreaterThan(0);
    for (const d of draws) expect(d).toMatch(/^L'ordinateur pioche \d+ cartes?\.$/);
  });

  test("l'ordinateur n'abandonne jamais, sur plusieurs parties", async ({ page }) => {
    await watchThinking(page);
    await openHome(page);
    for (const seed of [SEED, 7, 1234]) {
      await startGame(page, { deck: "Rempart", seed: String(seed) });
      await playToEnd(page, humanStep, MAX_HUMAN_ACTIONS);
      await expect(endHeading(page)).toHaveText(/^(Tu as gagné !|L'ordinateur a gagné\.)$/);
      const reason = await page.locator(".end-screen p").first().textContent();
      expect(reason, `seed ${seed}`).not.toMatch(/L'ordinateur a abandonné/);
      await page.getByRole("button", { name: "Retour à l'accueil" }).tap();
      await expect(page.getByRole("heading", { name: "Choisis ton deck" })).toBeVisible();
    }
    expect((await watched(page)).activeControls).toBe(0);
  });
});

test.describe("3. Temps : l'aperçu annoncé est le temps réellement payé", () => {
  test("avant/après : le coût et « passe la main » affichés tiennent, une action qui garde la main et une qui la passe", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });

    let kept = 0;
    let passed = 0;
    const MAX_ACTIONS = 12; // one keeps and one passes within the first two or three actions
    for (let n = 0; n < MAX_ACTIONS && (kept === 0 || passed === 0); n++) {
      await settle(page);
      // The first control the player could press: a card of the hand, else a hero effect.
      const card = ownHand(page).locator("button:not([disabled])").first();
      const isCard = (await card.count()) > 0;
      const control = isCard ? card : page.locator(".btn-effect:not([disabled])").first();
      // A card announces its cost in its accessible name and its time consequence in the legend under it
      // ("Reste N" or "Passe la main", #92); a hero effect still prints both inside its button.
      const text = await control.innerText();
      const name = isCard ? (await control.locator(".card-name").textContent())! : (await control.locator(".effect-name").textContent())!;
      const label = isCard ? ((await control.getAttribute("aria-label")) ?? "") : text;
      const cost = Number(/[Cc]oût (\d+)/.exec(label)![1]);
      const legend = isCard ? ((await control.locator(".card-legend").textContent()) ?? "") : text;
      const keepsMatch = isCard ? /^Reste (\d+)$/.exec(legend) : /Il te restera (\d+) de temps/.exec(text);
      // The time the opponent gets is printed by an effect only; a card's legend says "Passe la main" alone.
      const passMatch = isCard ? (legend === "Passe la main" ? [legend, ""] : null) : /Passe la main : l'adversaire aura (\d+) de temps/.exec(text);
      expect(keepsMatch !== null || passMatch !== null, `a time consequence is announced on "${name}": ${text}`).toBe(true);
      if (isCard) {
        // The pill shows the same cost as the accessible name, which also carries the legend's meaning.
        await expect(control.locator(".card-cost")).toHaveText(String(cost));
        expect(label.toLowerCase()).toContain(keepsMatch !== null ? `il te restera ${keepsMatch[1]} de temps` : "passe la main");
      }

      await control.tap();
      await settle(page);

      const blocks = await readJournal(page);
      const mine = blocks.filter((b) => b.title === "Ton tour").at(-1)!;
      // The cost paid is the cost announced.
      const paid = isCard ? `Tu invoques ${name} (coût ${cost}).` : `Tu utilises ${name} (coût ${cost}).`;
      expect(mine.lines.slice(-8), `cost of "${name}"`).toContain(paid);
      if (keepsMatch !== null) {
        kept += 1;
        const left = keepsMatch[1];
        expect(mine.lines.at(-1)).toBe(`Tu gardes la main (temps restant ${left}).`);
        await expect(timeBand(page)).toContainText("À toi");
        await expect(timeBand(page)).toContainText(`Temps : ${left}`);
      } else {
        passed += 1;
        if (isCard) expect(mine.lines.at(-1)).toMatch(/^La main passe à L'ordinateur \(temps \d+\)\.$/);
        else expect(mine.lines.at(-1)).toBe(`La main passe à L'ordinateur (temps ${passMatch![1]}).`);
        expect(blocks.at(-1)!.title).toBe("Tour de l'ordinateur"); // the computer did take the main
      }
      if (await isOver(page)) break;
    }
    expect(kept, "an action that keeps the main was exercised").toBeGreaterThan(0);
    expect(passed, "an action that passes the main was exercised").toBeGreaterThan(0);
  });
});
