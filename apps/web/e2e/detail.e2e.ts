// #107, the e2e task of story #90: the card detail and the actions taken from it (#104 read-only detail,
// #105 "Jouer" from a hand card, #106 the attack line of an own creature). Production build, real engine,
// real computer; the only thing arranged by the test is WHEN the page's own zero-delay timers fire, in the one
// case that must stand inside the computer's turn (case 6). Everything is read from the DOM (roles, accessible
// names, French texts, boxes): no cost, target or rule is recomputed, a label read BEFORE an action is only
// compared with one read AFTER.
//
// The situations are searched by bounded loops (`playUntil`) that fail loudly, so that a changed starter set or
// seed cannot pass silently; the searches are the ones of `two-camps.e2e.ts` and `cards.e2e.ts`.
import type { Locator, Page } from "@playwright/test";
import {
  SEED,
  activeCamp,
  closeDetail,
  creatureTargets,
  detailSheet,
  attackLine,
  expect,
  expectAccessible,
  expectMobileSound,
  expectNoHorizontalScroll,
  expectTouchTargets,
  handTiles,
  hasAttacker,
  heroTarget,
  isInactive,
  openDetail,
  openHome,
  opponentBoardTiles,
  ownBoardTiles,
  ownHand,
  playButton,
  playUntil,
  readJournal,
  selectAttacker,
  settle,
  startGame,
  tapCard,
  tapEffect,
  test,
  thinking,
  timeBand,
  topBoard,
  bottomBoard,
} from "./support";

const CAMP_1 = "Camp 1 — Ruée";
const CAMP_2 = "Camp 2 — Rempart";
const twoCamps = (page: Page) => startGame(page, { mode: "two-camps", deck: "Ruée", deckTwo: "Rempart", seed: String(SEED) });
const regionOf = (page: Page, camp: string, what: "main" | "plateau" | "héros") => page.getByRole("group", { name: `${camp} : ${what}`, exact: true });
/** A hand card that hands the main over: the tile says so in its accessible name (the engine's preview). */
const passingTile = (page: Page): Locator => ownHand(page).locator('.card[aria-label$="passe la main"]');

/** Two camps: Ruée to play, with a creature that can attack, facing a "Garde du rempart" (provocation). */
async function reachProvocation(page: Page): Promise<void> {
  await playUntil(
    page,
    async (p) =>
      (await activeCamp(p)) === CAMP_1
        ? (await tapCard(p)) || (await tapEffect(p))
        : (await tapCard(p, /Garde du rempart/)) || (await tapEffect(p)) || (await tapCard(p)),
    async (p) => (await activeCamp(p)) === CAMP_1 && (await hasAttacker(p)) && (await topBoard(p).getByText("Provocation").count()) > 0,
    40,
    "Ruée can attack a board with a Garde du rempart",
  );
}

/** Against the computer: both boards hold a creature and the human has the main back. */
async function reachBothBoards(page: Page): Promise<void> {
  await playUntil(
    page,
    async (p) => (await tapCard(p)) || (await tapEffect(p)),
    async (p) => (await opponentBoardTiles(p).count()) > 0 && (await ownBoardTiles(p).count()) > 0,
    30,
    "a creature on each board",
  );
}

/** What the journal says now: a detail that is only opened or closed must leave it as it was. */
const journalNow = async (page: Page): Promise<string> => JSON.stringify(await readJournal(page));

test.describe("1. Ouvrir et fermer", () => {
  test("toucher ouvre le détail ; « Fermer » et Échap le ferment, le focus revient à la carte, rien n'est joué", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    await settle(page);
    const before = { journal: await journalNow(page), band: await timeBand(page).innerText(), hand: await handTiles(page).count() };
    const tile = handTiles(page).first();
    const name = (await tile.locator(".card-name").textContent())!;

    // "Fermer".
    const sheet = await openDetail(page, tile);
    await expect(page.getByRole("dialog", { name })).toBeVisible();
    await expect(sheet).toBeFocused(); // the sheet itself holds the focus, so that a screen reader reads the card
    await closeDetail(page);
    await expect(tile).toBeFocused(); // given back to the tile that opened it

    // Échap, from the keyboard.
    await openDetail(page, tile);
    await page.keyboard.press("Escape");
    await expect(detailSheet(page)).toHaveCount(0);
    await expect(tile).toBeFocused();

    // Opening and closing played nothing: same journal, same time band, same hand.
    expect(await journalNow(page)).toBe(before.journal);
    expect(await timeBand(page).innerText()).toBe(before.band);
    expect(await handTiles(page).count()).toBe(before.hand);
    await expect(page.getByRole("alert")).toHaveCount(0);
  });
});

test.describe("2. Jouer depuis le détail", () => {
  test("avec passation : « passe la main » lu avant, vérifié après", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);
    await playUntil(page, async (p) => tapCard(p), async (p) => (await passingTile(p).count()) > 0, 20, "a hand card that hands the main over");
    const tile = passingTile(page).first();
    const label = (await tile.getAttribute("aria-label"))!;
    const name = (await tile.locator(".card-name").textContent())!;
    const cost = Number(/, coût (\d+),/.exec(label)![1]);

    // Read BEFORE: the sheet announces the cost and the hand-over, and the time the other camp will get.
    const sheet = await openDetail(page, tile);
    await expect(playButton(sheet)).toHaveText(`Jouer (${cost} temps)`);
    const consequence = sheet.locator(".play-consequence");
    await expect(consequence).toContainText(`Coût : ${cost}`);
    const announced = /Passe la main : l'autre camp aura (\d+) de temps/.exec(await consequence.innerText());
    expect(announced, "the consequence names the time the other camp gets").not.toBeNull();
    const camp = await activeCamp(page);
    expect(camp).toBe(CAMP_1);

    // Verified AFTER: the main moved, the other camp holds the announced time, the journal says the same.
    await playButton(sheet).tap();
    await expect(sheet).toHaveCount(0);
    await expect(timeBand(page)).toContainText(`Au tour de ${CAMP_2}`);
    await expect(timeBand(page)).toContainText(`Temps : ${announced![1]}`);
    const mine = (await readJournal(page)).flatMap((b) => b.lines);
    expect(mine).toContain(`${CAMP_1} invoque ${name} (coût ${cost}).`);
    expect(mine.at(-1)).toBe(`La main passe à ${CAMP_2} (temps ${announced![1]}).`);
  });
});

test.describe("3. Attaquer depuis le détail", () => {
  test("face à un provocateur : les seules cibles sont les provocateurs, confirmation puis résultat", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);
    await reachProvocation(page);

    // The attack is a line of the creature's detail: its words and its cost are read before pressing it.
    const sheet = await openDetail(page, ownBoardTiles(page).first());
    const line = attackLine(sheet);
    await expect(line).toHaveAccessibleName(/^Attaque \(\d+\) : inflige \d+ dégâts?, pour \d+ temps$/);
    const cost = /^Attaque \((\d+)\)/.exec((await line.getAttribute("aria-label"))!)![1];
    await line.tap();
    await expect(sheet).toHaveCount(0); // the detail closes: the creature is the attacker, the targets come next
    await expect(page.getByText("Choisis une cible", { exact: true })).toBeVisible();

    // The targets are the provocateurs only; the hero is not available.
    const targets = creatureTargets(page);
    await expect(targets.first()).toContainText("Provocation");
    for (const t of await targets.all()) await expect(t).toContainText("Provocation");
    for (const t of await topBoard(page).locator('button[aria-disabled="true"]').all()) await expect(t).not.toContainText("Provocation");
    await expect(page.getByRole("button", { name: /^Le héros .*, cible non disponible$/ })).toHaveAttribute("aria-disabled", "true");
    await expect(heroTarget(page)).toHaveCount(0);

    // Confirmation: the sheet announces the cost, cancelling changes nothing, confirming attacks.
    await targets.first().tap();
    const confirmation = page.getByRole("dialog", { name: "Confirmer l'attaque" });
    await expect(confirmation).toContainText(/attaque Garde du rempart/);
    await expect(confirmation).toContainText(`Coût d'attaque : ${cost}`);
    await confirmation.getByRole("button", { name: "Annuler" }).tap();
    await expect(confirmation).toHaveCount(0);
    await targets.first().tap();
    await confirmation.getByRole("button", { name: "Confirmer" }).tap();
    await expect(confirmation).toHaveCount(0);
    await settle(page);

    // Result: the journal says who hit the warden, at the announced cost.
    const lines = (await readJournal(page)).flatMap((b) => b.lines).join("\n");
    expect(lines).toMatch(new RegExp(`${CAMP_1} attaque avec .* \\(coût ${cost}\\)\\.`));
    expect(lines).toMatch(/Garde du rempart subit \d+ dégâts? \(PV restants : \d+\)\./);
    await expect(page.getByText("Choisis une cible", { exact: true })).toHaveCount(0);
  });
});

test.describe("4. Détail adverse en lecture seule", () => {
  test("une créature du plateau adverse : aucun bouton hors « Fermer »", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    await reachBothBoards(page);
    const before = await journalNow(page);

    const sheet = await openDetail(page, opponentBoardTiles(page).first());
    await expect(sheet.getByRole("button")).toHaveCount(1);
    await expect(sheet.getByRole("button", { name: "Fermer" })).toBeVisible();
    await expect(playButton(sheet)).toHaveCount(0);
    await expect(attackLine(sheet)).toHaveCount(0);
    // The effect line is there to be read, as plain text: "Attaque (N) : inflige D dégâts".
    await expect(sheet.locator(".card-detail-effect")).toContainText(/^Attaque \(\d+\) : inflige \d+ dégâts?/);
    await closeDetail(page);
    expect(await journalNow(page)).toBe(before);
  });
});

test.describe("5. Choix d'une cible", () => {
  test("toucher une carte adverse pendant le choix la choisit sans ouvrir son détail", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);
    await reachProvocation(page);
    expect(await selectAttacker(page)).toBeGreaterThanOrEqual(0);
    await expect(page.getByText("Choisis une cible", { exact: true })).toBeVisible();

    // A card that is not a target (no provocation): the touch does nothing, no detail opens.
    for (const t of await topBoard(page).locator('button[aria-disabled="true"]').all()) {
      await t.tap({ force: true });
      await expect(page.getByRole("dialog")).toHaveCount(0);
    }
    // A target: the touch chooses it (the confirmation opens), it does not open the card's detail.
    await creatureTargets(page).first().tap();
    await expect(page.getByRole("dialog", { name: "Confirmer l'attaque" })).toBeVisible();
    await expect(detailSheet(page)).toHaveCount(0);
    await page.getByRole("dialog", { name: "Confirmer l'attaque" }).getByRole("button", { name: "Annuler" }).tap();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    // Out of the choice, the same card opens its detail again.
    await page.getByRole("button", { name: "Annuler la sélection" }).tap();
    await openDetail(page, opponentBoardTiles(page).first());
    await closeDetail(page);
  });
});

test.describe("6. Tour de l'ordinateur", () => {
  /**
   * The computer's turn lasts a few milliseconds, too short to touch a card in. This holds back the page's own
   * zero-delay timers (the ones the controller yields on between two computer actions) from the moment the test
   * says so until it releases them: the engine, the computer and the interface stay the real ones.
   */
  const holdTimers = (page: Page) =>
    page.addInitScript(() => {
      type W = { __hold: boolean; __held: (() => void)[]; __release(): void };
      const w = window as unknown as W;
      const real = window.setTimeout.bind(window);
      w.__hold = false;
      w.__held = [];
      window.setTimeout = ((fn: TimerHandler, ms?: number, ...args: unknown[]) => {
        if (w.__hold && !ms && typeof fn === "function") {
          w.__held.push(() => real(fn, 0, ...args));
          return 0;
        }
        return real(fn, ms, ...args);
      }) as typeof window.setTimeout;
      w.__release = () => {
        w.__hold = false;
        for (const run of w.__held.splice(0)) run();
      };
    });
  const hold = (page: Page) => page.evaluate(() => ((window as unknown as { __hold: boolean }).__hold = true));
  const release = (page: Page) => page.evaluate(() => (window as unknown as { __release(): void }).__release());

  test("le détail s'ouvre, « Jouer » et l'attaque sont inactifs, puis la main est rendue", async ({ page }) => {
    await holdTimers(page);
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    // A creature on the board first, and a card that hands the main over in the hand.
    await playUntil(
      page,
      async (p) => tapCard(p),
      async (p) => (await ownBoardTiles(p).count()) > 0 && (await passingTile(p).count()) > 0,
      20,
      "a creature on the board and a hand card that hands the main over",
    );
    await hold(page);
    const sheet = await openDetail(page, passingTile(page).first());
    await playButton(sheet).tap();
    await expect(sheet).toHaveCount(0);
    await expect(thinking(page)).toBeVisible(); // the computer's turn is on, and held

    // The hand card opens, "Jouer" is inactive and says why; pressing it plays nothing.
    const handSheet = await openDetail(page, handTiles(page).first());
    const play = playButton(handSheet);
    await expect(play).toHaveAttribute("aria-disabled", "true");
    await expect(handSheet).toContainText("C'est le tour de l'ordinateur");
    await play.tap({ force: true }); // aria-disabled, not disabled: still pressable, and does nothing
    await expect(handSheet).toBeVisible();
    await expect(thinking(page)).toBeVisible();
    await closeDetail(page);

    // So does an own creature: its attack line is inactive.
    const boardSheet = await openDetail(page, ownBoardTiles(page).first());
    expect(await isInactive(attackLine(boardSheet))).toBe(true);
    await expect(boardSheet).toContainText("C'est le tour de l'ordinateur");
    await closeDetail(page);

    // The computer plays on; the human's hand is playable again.
    await release(page);
    await settle(page);
    const back = await openDetail(page, handTiles(page).first());
    expect(await isInactive(playButton(back))).toBe(false);
    await closeDetail(page);
  });
});

test.describe("7. Deux camps", () => {
  test("la main de l'autre camp est en lecture seule ; après la passation, la main échangée ne rouvre pas le détail", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);

    // The other camp's hand reads, it does not play.
    const otherHand = regionOf(page, CAMP_2, "main");
    const read = await openDetail(page, otherHand.locator(".card").first());
    await expect(read.getByRole("button")).toHaveCount(1);
    await expect(playButton(read)).toHaveCount(0);
    await closeDetail(page);

    // Camp 1 plays a card that hands the main over, from its detail.
    await playUntil(page, async (p) => tapCard(p), async (p) => (await passingTile(p).count()) > 0, 20, "a hand card that hands the main over");
    const sheet = await openDetail(page, passingTile(page).first());
    await playButton(sheet).tap();
    await expect(timeBand(page)).toContainText(`Au tour de ${CAMP_2}`);
    // No detail survives the swap (the pressed tile is gone, the hands changed places).
    await expect(page.getByRole("dialog")).toHaveCount(0);

    // Camp 2's hand is the playable one now (it is the one at the bottom), Camp 1's reads only.
    const own = await openDetail(page, regionOf(page, CAMP_2, "main").locator(".card").first());
    expect(await isInactive(playButton(own))).toBe(false);
    await closeDetail(page);
    const other = await openDetail(page, regionOf(page, CAMP_1, "main").locator(".card").first());
    await expect(other.getByRole("button")).toHaveCount(1);
    await expect(playButton(other)).toHaveCount(0);
    await closeDetail(page);
  });
});

test.describe("8. Mise en page à 360 × 640", () => {
  test.use({ viewport: { width: 360, height: 640 } });

  /** The card's box (520 px) and its art (230 px) do not depend on what the sheet carries. */
  async function expectFixedCard(sheet: Locator, what: string): Promise<void> {
    const card = (await sheet.locator(".card-detail").boundingBox())!;
    const art = (await sheet.locator(".card-detail-art").boundingBox())!;
    expect(card.height, `${what}: card height`).toBeCloseTo(520, 0);
    expect(art.height, `${what}: art height`).toBeCloseTo(230, 0);
  }

  /** "Fermer" is 48 px at least and can be reached by scrolling the dialog (the card itself never shrinks). */
  async function expectCloseReachable(page: Page, sheet: Locator, what: string): Promise<void> {
    const close = sheet.getByRole("button", { name: "Fermer" });
    expect((await close.boundingBox())!.height, `${what}: "Fermer" height`).toBeGreaterThanOrEqual(48);
    await sheet.evaluate((el) => (el.scrollTop = el.scrollHeight));
    await expect(close, `${what}: "Fermer" after scrolling the dialog`).toBeInViewport({ ratio: 1 });
    const viewport = page.viewportSize()!;
    const box = (await close.boundingBox())!;
    expect(box.y + box.height, `${what}: inside the screen`).toBeLessThanOrEqual(viewport.height);
  }

  test("carte et illustration fixes avec ou sans action, « Fermer » atteignable, ligne d'effet de 44 px, zone d'effets défilante", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    await reachBothBoards(page);

    // 1. A hand card, with "Jouer" (the tallest sheet): the dialog scrolls.
    const withPlay = await openDetail(page, handTiles(page).first());
    await expectFixedCard(withPlay, "main avec Jouer");
    await expect(playButton(withPlay)).toBeVisible();
    const scrolls = await withPlay.evaluate((el) => ({ scroll: el.scrollHeight, client: el.clientHeight, overflowY: getComputedStyle(el).overflowY }));
    expect(scrolls.overflowY, "the dialog scrolls").toBe("auto");
    expect(scrolls.scroll, "the sheet with actions is taller than the 640 px screen").toBeGreaterThan(scrolls.client);
    await expectCloseReachable(page, withPlay, "main avec Jouer");
    await expectMobileSound(page, "détail d'une carte de la main, avec Jouer");
    await closeDetail(page);

    // 2. An opponent's creature, read-only.
    const readOnly = await openDetail(page, opponentBoardTiles(page).first());
    await expectFixedCard(readOnly, "plateau adverse");
    await expect(readOnly.getByRole("button")).toHaveCount(1);
    await expectCloseReachable(page, readOnly, "plateau adverse");
    await expectMobileSound(page, "détail d'une créature adverse, lecture seule");
    await closeDetail(page);

    // 3. An own creature: the attack line is a button of 44 px at least; the effects zone scrolls inside itself.
    const withAttack = await openDetail(page, bottomBoard(page).locator(".card").first());
    await expectFixedCard(withAttack, "plateau avec attaque");
    const line = attackLine(withAttack);
    expect((await line.boundingBox())!.height, "attack line height").toBeGreaterThanOrEqual(44);
    const zone = withAttack.getByRole("region", { name: "Effets" });
    expect(await zone.evaluate((el) => getComputedStyle(el).overflowY), "the effects zone scrolls inside itself").toBe("auto");
    await expect(zone).toHaveAttribute("tabindex", "0"); // scrollable-region-focusable
    await expectCloseReachable(page, withAttack, "plateau avec attaque");
    await expectMobileSound(page, "détail d'une créature à moi, ligne d'attaque");
    await closeDetail(page);

    // The same sheets, once more, with the checks that need the dialog to be scrolled to the bottom: still sound.
    await openDetail(page, handTiles(page).first());
    await detailSheet(page).evaluate((el) => (el.scrollTop = el.scrollHeight));
    await expectNoHorizontalScroll(page);
    await expectTouchTargets(page);
    await expectAccessible(page);
    await closeDetail(page);
  });
});
