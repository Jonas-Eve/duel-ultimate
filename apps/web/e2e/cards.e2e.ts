// #95: the look of the compact cards (#87: #92 face and tags, #93 discard, #94 deck chips), on the
// production build, with the real engine and the real computer. Everything is read from the DOM
// (boxes, accessible names, French texts): no cost, target or rule is recomputed here.
//
// The searches below are bounded (they fail loudly if a changed starter set or seed moves the
// situation); they reuse the ones of `two-camps.e2e.ts`.
import type { Locator, Page } from "@playwright/test";
import {
  SEED,
  activeCamp,
  attackers,
  bottomBoard,
  confirmAttack,
  creatureTargets,
  enabledButtons,
  expect,
  expectMobileSound,
  expectNoHorizontalScroll,
  gameRegion,
  heroTarget,
  openHome,
  ownHand,
  playUntil,
  readJournal,
  settle,
  startGame,
  tapCard,
  tapEffect,
  test,
  tile,
  topBoard,
} from "./support";

const CAMP_1 = "Camp 1 — Ruée";
const CAMP_2 = "Camp 2 — Rempart";
const FACE_HEIGHT = 112; // the contract of #92: `.card-face` is `--card-w` x 112 px
const TOLERANCE = 0.5;

const twoCamps = (page: Page) => startGame(page, { mode: "two-camps", deck: "Ruée", deckTwo: "Rempart", seed: String(SEED) });
const regionOf = (page: Page, camp: string, what: "main" | "plateau" | "héros") => page.getByRole("group", { name: `${camp} : ${what}`, exact: true });
const faces = (root: Locator): Locator => root.locator(".card-face");

/** The boxes of the visible faces under `root`. */
async function boxesOf(root: Locator): Promise<{ width: number; height: number }[]> {
  const out: { width: number; height: number }[] = [];
  for (const face of await faces(root).all()) {
    const box = await face.boundingBox();
    expect(box, "a visible face has a box").not.toBeNull();
    out.push({ width: box!.width, height: box!.height });
  }
  return out;
}

/** Every box has the reference size, and the height is the contract's 112 px. */
function expectSameBox(boxes: { width: number; height: number }[], ref: { width: number; height: number }, what: string): void {
  expect(boxes.length, `${what}: at least one face`).toBeGreaterThan(0);
  for (const b of boxes) {
    expect(Math.abs(b.height - FACE_HEIGHT), `${what}: height ${b.height}`).toBeLessThanOrEqual(TOLERANCE);
    expect(Math.abs(b.width - ref.width), `${what}: width ${b.width} against ${ref.width}`).toBeLessThanOrEqual(TOLERANCE);
    expect(Math.abs(b.height - ref.height), `${what}: height ${b.height} against ${ref.height}`).toBeLessThanOrEqual(TOLERANCE);
  }
}

/** The numbers printed on a face (attack, riposte, life) against the ones of the accessible name. */
async function expectValuesMatchName(card: Locator): Promise<void> {
  const name = (await card.getAttribute("aria-label")) ?? "";
  const m = /attaque (\d+), riposte (\d+), vie (\d+)/.exec(name);
  expect(m, `three values in the accessible name: ${name}`).not.toBeNull();
  const shown = (await card.locator(".card-value").allTextContents()).map((t) => t.trim());
  expect(shown, `printed values of ${name}`).toEqual([m![1], m![2], m![3]]);
}

test.describe("1. Taille fixe", () => {
  test("toutes les cartes (main, deux plateaux) ont la même boîte, la main de cinq tient sans déborder", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    await settle(page);

    // Five cards in hand, one face each, none overflowing the row, nor the page.
    const hand = ownHand(page);
    await expect(faces(hand)).toHaveCount(5);
    const handBoxes = await boxesOf(hand);
    const ref = handBoxes[0];
    expectSameBox(handBoxes, ref, "main");
    const row = await hand.evaluate((el) => ({ scroll: el.scrollWidth, client: el.clientWidth }));
    expect(row.scroll, "five compact cards fit a 360 px screen").toBeLessThanOrEqual(row.client);
    await expectNoHorizontalScroll(page);

    // Invoke until both boards hold cards (the computer plays its own turn in between).
    await playUntil(
      page,
      async (p) => {
        const hands = enabledButtons(ownHand(p));
        if ((await hands.count()) > 0) {
          await hands.first().tap();
          return true;
        }
        return tapEffect(p);
      },
      async (p) => (await faces(topBoard(p)).count()) > 0 && (await faces(bottomBoard(p)).count()) > 0,
      30,
      "a creature on each board",
    );
    expectSameBox(await boxesOf(topBoard(page)), ref, "plateau du haut");
    expectSameBox(await boxesOf(bottomBoard(page)), ref, "plateau du bas");
    expectSameBox(await boxesOf(ownHand(page)), ref, "main après invocations");
    await expectNoHorizontalScroll(page);
    await expectMobileSound(page, "partie, cartes compactes sur les deux plateaux");
  });
});

test.describe("2. Contenu", () => {
  test("coût en pastille dans la main seulement, trois valeurs dont la riposte 0, nom accessible complet", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    await settle(page);

    // Hand: one cost pill per card, equal to the cost of the accessible name, shown once.
    const handCards = ownHand(page).locator(".card");
    const n = await handCards.count();
    expect(n).toBe(5);
    for (const card of await handCards.all()) {
      const name = (await card.getAttribute("aria-label")) ?? "";
      const cost = /, coût (\d+),/.exec(name);
      expect(cost, `a cost in the accessible name: ${name}`).not.toBeNull();
      await expect(card.locator(".card-cost")).toHaveCount(1);
      await expect(card.locator(".card-cost")).toHaveText(cost![1]);
      await expectValuesMatchName(card);
      expect(name, "full accessible name").toMatch(/^.+, coût \d+, attaque \d+, riposte \d+, vie \d+/);
    }
    await expect(ownHand(page).locator(".card-cost")).toHaveCount(n);

    // Boards: no cost pill, the three values (riposte even when it is 0), the same accessible name without cost.
    await playUntil(
      page,
      async (p) => (await tapCard(p)) || (await tapEffect(p)),
      async (p) => (await faces(topBoard(p)).count()) > 0 && (await faces(bottomBoard(p)).count()) > 0,
      30,
      "a creature on each board",
    );
    await expect(page.locator(".board .card-cost")).toHaveCount(0);
    const boardCards = page.locator(".board .card");
    let sawRiposteZero = false;
    for (const card of await boardCards.all()) {
      const name = (await card.getAttribute("aria-label")) ?? "";
      expect(name, "board name has no cost").toMatch(/^[^,]+, attaque \d+, riposte \d+, vie \d+/);
      expect(name).not.toContain("coût");
      await expect(card.locator(".card-value")).toHaveCount(3);
      await expectValuesMatchName(card);
      if (/riposte 0,/.test(name)) sawRiposteZero = true;
    }
    expect(sawRiposteZero, "a riposte of 0 is shown as a value on some card").toBe(true);
  });

  test("« Provocation » est écrite sous la carte provocatrice (hors de la face), jamais sous les autres", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);
    await playUntil(
      page,
      async (p) => ((await activeCamp(p)) === CAMP_1 ? (await tapCard(p)) || (await tapEffect(p)) : (await tapCard(p, /Garde du rempart/)) || (await tapEffect(p)) || (await tapCard(p))),
      async (p) => (await page.locator(".board .card-keyword").count()) > 0,
      40,
      "a Garde du rempart is on a board",
    );
    const card = page.locator(".board .card:has(.card-keyword)").first();
    const keyword = card.locator(".card-keyword");
    await expect(keyword).toHaveText("Provocation");
    await expect(card).toHaveAccessibleName(/provocation/);
    // Under the face, inside the interactive element (or the group), not in the fixed box.
    const face = (await card.locator(".card-face").boundingBox())!;
    const tag = (await keyword.boundingBox())!;
    expect(tag.y, "the label sits under the face").toBeGreaterThanOrEqual(face.y + face.height - TOLERANCE);
    await expect(card.locator(".card-face .card-keyword")).toHaveCount(0);
    // No other card claims it: every tile with the word is a card whose name says it too.
    for (const other of await page.locator(".board .card").all()) {
      const has = (await other.locator(".card-keyword").count()) > 0;
      expect(/provocation/.test((await other.getAttribute("aria-label")) ?? ""), "word and name agree").toBe(has);
    }
    await expectMobileSound(page, "deux camps, provocation sous une carte");
  });
});

test.describe("3. Symboles de deck", () => {
  test("deux camps : Ruée porte l'éclair, Rempart les créneaux, sur la main comme sur le plateau", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);

    const hand1 = regionOf(page, CAMP_1, "main");
    const hand2 = regionOf(page, CAMP_2, "main");
    await expect(faces(hand1)).toHaveCount(await hand1.locator(".card").count());
    await expect(hand1.locator(".card-emblem-bolt")).toHaveCount(await faces(hand1).count());
    await expect(hand1.locator(".card-emblem-battlements")).toHaveCount(0);
    await expect(hand2.locator(".card-emblem-battlements")).toHaveCount(await faces(hand2).count());
    await expect(hand2.locator(".card-emblem-bolt")).toHaveCount(0);

    // The chip is decorative, drawn on the art inside the face, and never changes the accessible name.
    const chip = hand1.locator(".card-emblem-bolt").first();
    await expect(chip).toHaveAttribute("aria-hidden", "true");
    const faceBox = (await hand1.locator(".card-face").first().boundingBox())!;
    const chipBox = (await chip.boundingBox())!;
    expect(chipBox.x).toBeGreaterThanOrEqual(faceBox.x - TOLERANCE);
    expect(chipBox.y).toBeGreaterThanOrEqual(faceBox.y - TOLERANCE);
    expect(chipBox.x + chipBox.width).toBeLessThanOrEqual(faceBox.x + faceBox.width + TOLERANCE);
    expect(chipBox.y + chipBox.height).toBeLessThanOrEqual(faceBox.y + faceBox.height + TOLERANCE);

    // Camp 1 invokes: its creature on the board carries the bolt; then Camp 2's board (after its turn) the battlements.
    expect(await tapCard(page)).toBe(true);
    await playUntil(
      page,
      async (p) => (await activeCamp(p)) === CAMP_1 ? (await tapCard(p)) || (await tapEffect(p)) : (await tapCard(p)) || (await tapEffect(p)),
      async (p) => (await faces(regionOf(p, CAMP_1, "plateau")).count()) > 0 && (await faces(regionOf(p, CAMP_2, "plateau")).count()) > 0,
      30,
      "a creature on each camp's board",
    );
    const board1 = regionOf(page, CAMP_1, "plateau");
    const board2 = regionOf(page, CAMP_2, "plateau");
    await expect(board1.locator(".card-emblem-bolt")).toHaveCount(await faces(board1).count());
    await expect(board1.locator(".card-emblem-battlements")).toHaveCount(0);
    await expect(board2.locator(".card-emblem-battlements")).toHaveCount(await faces(board2).count());
    await expect(board2.locator(".card-emblem-bolt")).toHaveCount(0);
  });

  test("contre l'ordinateur : mon deck et le sien se reconnaissent à leur symbole", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    await settle(page);
    await expect(ownHand(page).locator(".card-emblem-bolt")).toHaveCount(5);
    await playUntil(
      page,
      async (p) => (await tapCard(p)) || (await tapEffect(p)),
      async (p) => (await faces(topBoard(p)).count()) > 0,
      30,
      "a creature of the computer on its board",
    );
    await expect(topBoard(page).locator(".card-emblem-battlements")).toHaveCount(await faces(topBoard(page)).count());
    await expect(topBoard(page).locator(".card-emblem-bolt")).toHaveCount(0);
  });
});

test.describe("4. Interactions inchangées", () => {
  test("invoquer, choisir un attaquant, cible non disponible face à la provocation, feuille de confirmation", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);

    // Invoking moves the card from the hand to the board.
    const handBefore = await enabledButtons(regionOf(page, CAMP_1, "main")).count();
    expect(handBefore).toBeGreaterThan(0);
    const handAll = await regionOf(page, CAMP_1, "main").locator(".card").count();
    expect(await tapCard(page)).toBe(true);
    await expect(regionOf(page, CAMP_1, "main").locator(".card")).toHaveCount(handAll - 1);
    await expect(faces(regionOf(page, CAMP_1, "plateau"))).toHaveCount(1);

    // Search the situation: Ruée to play, a creature able to attack, facing a Garde du rempart.
    await playUntil(
      page,
      async (p) => ((await activeCamp(p)) === CAMP_1 ? (await tapCard(p)) || (await tapEffect(p)) : (await tapCard(p, /Garde du rempart/)) || (await tapEffect(p)) || (await tapCard(p))),
      async (p) => (await activeCamp(p)) === CAMP_1 && (await attackers(p).count()) > 0 && (await topBoard(p).locator(".card-keyword").count()) > 0,
      40,
      "Ruée can attack a board with a Garde du rempart",
    );
    const attacker = attackers(page).first();
    await attacker.tap();
    await expect(attacker).toHaveAttribute("aria-pressed", "true");
    await expect(attacker).toContainText("Sélectionnée");
    await expect(attacker).toHaveAccessibleName(/sélectionnée$/);

    // The hero is not available (provocation), the provocateur is a possible target.
    const hero = page.getByRole("button", { name: /^Le héros .*, cible non disponible$/ });
    await expect(hero).toHaveAttribute("aria-disabled", "true");
    await expect(hero).toHaveText("Cible non disponible");
    await expect(heroTarget(page)).toHaveCount(0);
    const target = creatureTargets(page).first();
    await expect(target).toContainText("Cible possible");
    await expect(target).toContainText("Provocation");
    await expect(target).toHaveAccessibleName(/provocation.*cible possible$/);

    // The confirmation sheet announces the cost; cancelling leaves the board as it was.
    await target.tap();
    const sheet = page.getByRole("dialog", { name: "Confirmer l'attaque" });
    await expect(sheet).toContainText(/Coût d'attaque : \d+/);
    await sheet.getByRole("button", { name: "Annuler" }).tap();
    await expect(sheet).toHaveCount(0);
    await target.tap();
    await confirmAttack(page);
    await settle(page);
    const lines = (await readJournal(page)).flatMap((b) => b.lines).join("\n");
    expect(lines).toMatch(/Garde du rempart subit \d+ dégâts? \(PV restants : \d+\)\./);
  });
});

test.describe("5. Défausse", () => {
  test("une créature détruite : « Détruite », jamais « PV 0 », même boîte que sur le plateau, riposte qui blesse l'attaquant", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);

    // The riposte situation of two-camps.e2e.ts: Rempart can attack a Loup hérissé (riposte 1, life 1) of Ruée.
    const wolfOnBoard = (p: Page) => tile(topBoard(p), "Loup hérissé");
    await playUntil(
      page,
      async (p) => ((await activeCamp(p)) === CAMP_1 ? (await tapCard(p, /Loup hérissé/)) || (await tapCard(p)) || (await tapEffect(p)) : (await tapCard(p)) || (await tapEffect(p))),
      async (p) => (await activeCamp(p)) === CAMP_2 && (await attackers(p).count()) > 0 && (await wolfOnBoard(p).count()) > 0,
      60,
      "Rempart can attack a Loup hérissé of Ruée",
    );
    const boardBox = (await faces(wolfOnBoard(page).first()).first().boundingBox())!;
    const ref = { width: boardBox.width, height: boardBox.height };
    expectSameBox([ref], ref, "loup sur le plateau");

    await attackers(page).first().tap();
    await wolfOnBoard(page).first().tap();
    await confirmAttack(page);
    await settle(page);
    const lines = (await readJournal(page)).flatMap((b) => b.lines);
    expect(lines.join("\n")).toMatch(/subit 1 dégât de riposte \(PV restants : \d+\)\./); // the riposte wounded the attacker
    expect(lines).toContain("Loup hérissé meurt.");

    const discardButton = page.getByRole("group", { name: `${CAMP_1} : héros` }).getByRole("button", { name: /^Défausse \(\d+\)$/ });
    await discardButton.tap();
    const discard = page.getByRole("dialog", { name: `Défausse : ${CAMP_1} : héros` });
    await expect(discard).toBeVisible();
    const dead = discard.getByRole("group", { name: /^Loup hérissé/ });
    await expect(dead).toHaveCount(1);
    await expect(dead).toHaveAccessibleName(/riposte 1, détruite/);
    await expect(dead.locator(".card-legend")).toHaveText("Détruite");
    await expect(dead.locator(".card-emblem-bolt")).toHaveCount(1);
    await expect(dead.locator(".card-cost")).toHaveCount(0);
    const text = await discard.innerText();
    expect(text, "never a life of zero").not.toMatch(/PV\s*0|vie 0/i);
    await expect(dead.locator(".card-value")).toHaveCount(3);
    expect((await dead.locator(".card-value").last().textContent())?.trim(), "the life slot holds no number").toBe("");
    // The same box as on the board.
    expectSameBox(await boxesOf(discard), ref, "défausse");
    await expectMobileSound(page, "dialogue de défausse, créature détruite");
    await discard.getByRole("button", { name: "Fermer" }).tap();
    await expect(discard).toHaveCount(0);
    await expect(gameRegion(page)).toBeVisible();
  });
});

test.describe("6. Mobile", () => {
  test("écran de partie à 360 px, deux camps : axe, cibles de 44 px, pas de défilement de la page", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);
    await expect(page.locator(".hand")).toHaveCount(2);
    await expectMobileSound(page, "deux camps, deux mains");
    await expect(page.locator(".card-face").first()).toBeVisible();
    const dims = await page.evaluate(() => ({ w: window.innerWidth, h: window.innerHeight }));
    expect(dims).toEqual({ w: 360, h: 740 });
  });
});
