// Cases 4 to 6 of #68: provocation and riposte (two camps bring the situation about), the two-camp
// mode itself, and the three ways a game ends. Production build, real engine, no mock.
//
// Not reachable from the interface, so not tested here: the "action refused" safety net (the
// interface only hands back previews the engine produced; covered by injection in #63, the
// controller, and #64, the banner) and `entropy_unavailable` (Chromium has
// `crypto.getRandomValues` even on a non-secure origin; covered in #64 with a fake `crypto`).
import type { Page } from "@playwright/test";
import {
  SEED,
  activeCamp,
  attackWith,
  bottomBoard,
  creatureTargets,
  endHeading,
  expect,
  expectMobileSound,
  gameRegion,
  handTiles,
  hasAttacker,
  heroTarget,
  isOver,
  openHome,
  ownHand,
  playToEnd,
  playUntil,
  readBlocks,
  readJournal,
  selectAttacker,
  openDetail,
  closeDetail,
  playButton,
  isInactive,
  settle,
  startGame,
  tapCard,
  tapEffect,
  test,
  tile,
  timeBand,
  topBoard,
  wearDownStep,
} from "./support";

const CAMP_1 = "Camp 1 — Ruée";
const CAMP_2 = "Camp 2 — Rempart";
const twoCamps = (page: Page, seed: string | number = SEED) =>
  startGame(page, { mode: "two-camps", deck: "Ruée", deckTwo: "Rempart", seed: String(seed) });

test.describe("4. Provocation et riposte", () => {
  test("provocation : seul le provocateur est une cible, le héros redevient ciblable à sa mort", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);

    // Bounded search (<= 40 actions) of the situation: Ruée to play, with a creature that can
    // attack, facing a "Garde du rempart" (provocation). Ruée invokes, Rempart looks for the warden.
    const found = await playUntil(
      page,
      async (p) =>
        (await activeCamp(p)) === CAMP_1
          ? (await tapCard(p)) || (await tapEffect(p))
          : (await tapCard(p, /Garde du rempart/)) || (await tapEffect(p)) || (await tapCard(p)),
      async (p) => (await activeCamp(p)) === CAMP_1 && (await hasAttacker(p)) && (await topBoard(p).getByText("Provocation").count()) > 0,
      40,
      "Ruée can attack a board with a Garde du rempart",
    );
    test.info().annotations.push({ type: "search", description: `${found} actions to reach the provocation situation` });

    // The attacker is selected: the provocateur is the only possible target, the hero is not available.
    expect(await selectAttacker(page)).toBeGreaterThanOrEqual(0);
    const targets = creatureTargets(page);
    await expect(targets.first()).toContainText("Provocation");
    for (const t of await targets.all()) await expect(t).toContainText("Cible possible");
    const unavailable = topBoard(page).locator('button[aria-disabled="true"]');
    for (const t of await unavailable.all()) {
      await expect(t).toContainText("Cible non disponible");
      await expect(t).not.toContainText("Provocation");
    }
    const hero = page.getByRole("button", { name: /^Le héros .*, cible non disponible$/ });
    await expect(hero).toHaveAttribute("aria-disabled", "true");
    await expect(hero).toHaveText("Cible non disponible");
    await expect(heroTarget(page)).toHaveCount(0);
    await hero.tap({ force: true }); // pressing it does nothing
    await expect(page.getByRole("dialog")).toHaveCount(0);

    // Attacking the provocateur: the sheet announces the cost, cancelling changes nothing, confirming attacks.
    await targets.first().tap();
    const sheet = page.getByRole("dialog", { name: "Confirmer l'attaque" });
    await expect(sheet).toContainText(/attaque Garde du rempart/);
    const announced = /Coût d'attaque : (\d+)/.exec(await sheet.innerText());
    expect(announced, "the sheet announces the attack cost").not.toBeNull();
    await sheet.getByRole("button", { name: "Annuler" }).tap();
    await expect(sheet).toHaveCount(0);
    await targets.first().tap();
    await sheet.getByRole("button", { name: "Confirmer" }).tap();
    await expect(sheet).toHaveCount(0);
    await settle(page);
    const first = await readJournal(page);
    const mine = first.filter((b) => b.title === CAMP_1).at(-1)!;
    expect(mine.lines.join("\n")).toMatch(new RegExp(`${CAMP_1} attaque avec .* \\(coût ${announced![1]}\\)\\.`));
    expect(mine.lines.join("\n")).toMatch(/Garde du rempart subit \d+ dégâts? \(PV restants : \d+\)\./);

    // Keep attacking the warden (Rempart only draws), until it dies: then the hero is a target again.
    const warden = (p: Page) => tile(topBoard(p), "Garde du rempart");
    await playUntil(
      page,
      async (p) => {
        if ((await activeCamp(p)) === CAMP_1) {
          if ((await hasAttacker(p)) && (await warden(p).count()) > 0) return (await attackWith(p, "creature")) !== null;
          return (await tapCard(p)) || (await tapEffect(p));
        }
        return (await tapEffect(page)) || (await tapCard(page, /^(?!Garde du rempart)/));
      },
      async (p) => (await activeCamp(p)) === CAMP_1 && (await hasAttacker(p)) && (await warden(p).count()) === 0 && (await topBoard(p).getByText("Provocation").count()) === 0,
      60,
      "the Garde du rempart is dead and Ruée can attack again",
    );
    const journal = (await readJournal(page)).flatMap((b) => b.lines);
    expect(journal).toContain("Garde du rempart meurt.");
    expect(await selectAttacker(page)).toBeGreaterThanOrEqual(0);
    const open = heroTarget(page);
    await expect(open).toHaveText("Cible possible");
    await expect(open).not.toHaveAttribute("aria-disabled", "true");
    await open.tap();
    await expect(page.getByRole("dialog", { name: "Confirmer l'attaque" })).toContainText(/attaque le héros Gardienne du rempart/);
    await page.getByRole("dialog", { name: "Confirmer l'attaque" }).getByRole("button", { name: "Confirmer" }).tap();
    await settle(page);
    expect((await readJournal(page)).flatMap((b) => b.lines).join("\n")).toMatch(/Le héros Gardienne du rempart subit \d+ dégâts? \(PV restants : \d+\)\./);
  });

  test("riposte : « Riposte 1 » sur la tuile, au journal, et la créature détruite est à la défausse", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);

    // Ruée invokes (a Loup hérissé first), Rempart invokes: search the moment Rempart can attack the wolf.
    const wolfOnBoard = (p: Page) => tile(topBoard(p), "Loup hérissé");
    const found = await playUntil(
      page,
      async (p) =>
        (await activeCamp(p)) === CAMP_1
          ? (await tapCard(p, /Loup hérissé/)) || (await tapCard(p)) || (await tapEffect(p))
          : (await tapCard(p)) || (await tapEffect(p)),
      async (p) => (await activeCamp(p)) === CAMP_2 && (await hasAttacker(p)) && (await wolfOnBoard(p).count()) > 0,
      60,
      "Rempart can attack a Loup hérissé of Ruée",
    );
    test.info().annotations.push({ type: "search", description: `${found} actions to reach the riposte situation` });
    await expect(wolfOnBoard(page).first()).toHaveAccessibleName(/riposte 1/);

    expect(await selectAttacker(page)).toBeGreaterThanOrEqual(0);
    const wolf = wolfOnBoard(page).first();
    await expect(wolf).toContainText("Cible possible"); // no provocation on Ruée's side
    await wolf.tap();
    const sheet = page.getByRole("dialog", { name: "Confirmer l'attaque" });
    await expect(sheet).toContainText(/attaque Loup hérissé/);
    await sheet.getByRole("button", { name: "Confirmer" }).tap();
    await settle(page);

    const lines = (await readJournal(page)).flatMap((b) => b.lines);
    expect(lines.join("\n")).toMatch(/subit 1 dégât de riposte \(PV restants : \d+\)\./);
    // The wolf (1 health) is dead: it is in Ruée's public discard, with its riposte.
    expect(lines).toContain("Loup hérissé meurt.");
    await page.getByRole("group", { name: `${CAMP_1} : héros` }).getByRole("button", { name: /^Défausse \(\d+\)$/ }).tap();
    const discard = page.getByRole("dialog", { name: `Défausse : ${CAMP_1} : héros` });
    await expect(discard).toContainText("Loup hérissé");
    await expect(discard.getByRole("group", { name: /Loup hérissé.*riposte 1/ })).toBeVisible();
    await expect(discard).toContainText("Détruite");
    await expectMobileSound(page, "défausse non vide");
    await discard.getByRole("button", { name: "Fermer" }).tap();
    await expect(discard).toHaveCount(0);
  });
});

test.describe("5. Deux camps", () => {
  const regionOf = (page: Page, camp: string, what: "main" | "plateau" | "héros") => page.getByRole("group", { name: `${camp} : ${what}`, exact: true });

  test("deux mains visibles, camp actif en bas, échange à la passation, journal tout nommé", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);
    const game = gameRegion(page);

    // Both hands are on screen; only the camp that holds the main can play its cards.
    await expect(timeBand(page)).toContainText(`Au tour de ${CAMP_1}`);
    await expect(regionOf(page, CAMP_1, "main").locator(".card-name").first()).toBeVisible();
    await expect(regionOf(page, CAMP_2, "main").locator(".card-name").first()).toBeVisible();
    await expect(game.getByText(`Main de ${CAMP_2} (non jouable)`)).toBeAttached();
    // Every tile of both hands opens a detail (a button with a popup); who may play is read from "Jouer" there.
    const otherHand = regionOf(page, CAMP_2, "main");
    await expect(otherHand.locator('button[aria-haspopup="dialog"]')).toHaveCount(await otherHand.locator(".card").count());
    const ownSheet = await openDetail(page, handTiles(page).first());
    expect(await isInactive(playButton(ownSheet)), "the camp that holds the main can play from its hand").toBe(false);
    await closeDetail(page);
    const otherSheet = await openDetail(page, otherHand.locator(".card").first());
    await expect(playButton(otherSheet), "the other camp's hand cannot be played").toHaveCount(0);
    await closeDetail(page);
    expect(await page.locator(".hand").count()).toBe(2);
    await expectMobileSound(page, "deux camps : deux mains, deux plateaux");

    // Camp 1 invokes a creature, then hands the main over.
    expect(await tapCard(page)).toBe(true);
    const played = (await bottomBoard(page).locator(".card-name").allTextContents())[0];
    expect(played).toBeTruthy();
    const y = async (camp: string) => (await regionOf(page, camp, "plateau").boundingBox())!.y;
    expect(await y(CAMP_2), "camp 1 plays at the bottom").toBeLessThan(await y(CAMP_1));
    await page.getByRole("button", { name: /^Renforts, coût \d+, passe la main$/ }).tap(); // a hero effect: still one tap

    // The hand is Camp 2's: its board comes down, Camp 1's goes up with its creature. No device-passing screen.
    await expect(timeBand(page)).toContainText(`Au tour de ${CAMP_2}`);
    await expect(game.locator(".announce")).toHaveText(`Au tour de ${CAMP_2}`);
    await expect(game).toBeFocused();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    expect(await y(CAMP_1), "camp 2 now plays at the bottom").toBeLessThan(await y(CAMP_2));
    await expect(regionOf(page, CAMP_1, "plateau")).toContainText(played);
    // The hands swapped: Camp 2's is playable now, Camp 1's (now the one above) is read-only.
    const nowOwn = await openDetail(page, regionOf(page, CAMP_2, "main").locator(".card").first());
    expect(await isInactive(playButton(nowOwn)), "camp 2 can play from its hand").toBe(false);
    await closeDetail(page);
    const nowOther = await openDetail(page, regionOf(page, CAMP_1, "main").locator(".card").first());
    await expect(playButton(nowOther), "camp 1's hand is read-only now").toHaveCount(0);
    await closeDetail(page);

    // And back again: Camp 2 hands the main over.
    await page.getByRole("button", { name: /^Réquisition, coût \d+, passe la main$/ }).tap();
    await expect(timeBand(page)).toContainText(`Au tour de ${CAMP_1}`);
    expect(await y(CAMP_2), "camp 1 is back at the bottom").toBeLessThan(await y(CAMP_1));

    // The journal names the camps everywhere: no "Tu", no "Toi", no computer.
    const blocks = await readJournal(page);
    const titles = blocks.map((b) => b.title);
    expect(titles).toEqual(expect.arrayContaining(["Début de partie", CAMP_1, CAMP_2]));
    expect(titles).not.toContain("Ton tour");
    expect(titles).not.toContain("Tour de l'ordinateur");
    const lines = blocks.flatMap((b) => b.lines);
    expect(lines.some((l) => l.startsWith(`${CAMP_1} invoque ${played}`))).toBe(true);
    for (const l of lines) {
      expect(l, "no second person or computer in two camps").not.toMatch(/^Tu |\bToi\b|L'ordinateur| tu /);
    }
  });

  test("le miroir est accepté : les deux camps se distinguent par leur numéro", async ({ page }) => {
    await openHome(page);
    await startGame(page, { mode: "two-camps", deck: "Ruée", deckTwo: "Ruée", seed: String(SEED) });
    await expect(regionOf(page, "Camp 1 — Ruée", "main")).toBeVisible();
    await expect(regionOf(page, "Camp 2 — Ruée", "main")).toBeVisible();
    await expect(regionOf(page, "Camp 1 — Ruée", "héros")).toBeVisible();
    await expect(regionOf(page, "Camp 2 — Ruée", "héros")).toBeVisible();
  });

  test("abandon avec confirmation : écran de fin « abandon »", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);
    await page.getByRole("button", { name: "Abandonner" }).tap();
    const dialog = page.getByRole("dialog", { name: "Le camp 1 abandonne ?" });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Continuer" }).tap(); // the safe answer
    await expect(dialog).toHaveCount(0);
    expect(await isOver(page)).toBe(false);
    await page.getByRole("button", { name: "Abandonner" }).tap();
    await dialog.getByRole("button", { name: "Abandonner" }).tap();
    await expect(endHeading(page)).toHaveText(`${CAMP_2} a gagné.`);
    await expect(page.getByText(`${CAMP_1} a abandonné.`, { exact: true })).toBeVisible();
    await expect(page.getByText("Graine de l'ordinateur", { exact: false })).toHaveCount(0);
  });
});

test.describe("6. Les trois fins", () => {
  test("hero_down : une attaque létale sur le héros", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);

    // Ruée attacks the hero as soon as it can, and invokes otherwise; Rempart only draws, so that no
    // provocation stands in the way. Bounded: the game must end by a lethal attack within 200 actions.
    let lethalSheet = "";
    const total = await playToEnd(
      page,
      async (p) => {
        if ((await activeCamp(p)) === CAMP_1) {
          if (await hasAttacker(p)) {
            const sheet = await attackWith(p, "hero");
            if (sheet?.includes("Met fin à la partie")) lethalSheet = sheet;
            return true;
          }
          return (await tapCard(p)) || (await tapEffect(p));
        }
        return (await tapEffect(p)) || (await tapCard(p, /^(?!Garde du rempart)/)) || (await tapCard(p));
      },
      200,
    );
    test.info().annotations.push({ type: "actions", description: `${total} actions to a lethal attack` });
    expect(lethalSheet, "the lethal attack was announced as ending the game before it was confirmed").toMatch(new RegExp(`Met fin à la partie : ${CAMP_1} gagne\\.`));
    await expect(endHeading(page)).toHaveText(`${CAMP_1} a gagné.`);
    await expect(page.getByText("Le héros Gardienne du rempart est tombé à 0 PV.", { exact: true }).first()).toBeVisible();
  });

  test("concede : l'abandon d'un camp", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);
    expect(await tapCard(page)).toBe(true);
    await page.getByRole("button", { name: "Abandonner" }).tap();
    await page.getByRole("dialog", { name: "Le camp 1 abandonne ?" }).getByRole("button", { name: "Abandonner" }).tap();
    await expect(endHeading(page)).toHaveText(`${CAMP_2} a gagné.`);
    await expect(page.getByText(`${CAMP_1} a abandonné.`, { exact: true })).toBeVisible();
  });

  // Exhaustion never happens at random (fact V7): the wear-down robot drives the interface, both
  // camps, reading the DOM only. Pinned since #172: Rempart against Rempart (camp 1 and camp 2
  // both play Rempart), seed EXHAUSTION_SEED. Under the standard rules (hero riposte, board limit
  // 5) Ruée and Rempart no longer end by exhaustion with this robot (the hero destroys the
  // attackers that hit it, and the game ends on the hero first: 1200 games searched), while
  // the Rempart mirror does, for every one of 150 seeds tried.
  const EXHAUSTION_SEED = 125;
  const REMPART_1 = "Camp 1 — Rempart";
  const REMPART_2 = "Camp 2 — Rempart";
  test("exhaustion : le robot d'usure épuise un camp", async ({ page }) => {
    test.setTimeout(180_000);
    await openHome(page);
    await startGame(page, { mode: "two-camps", deck: "Rempart", deckTwo: "Rempart", seed: String(EXHAUSTION_SEED) });
    const started = Date.now();
    const steps = await playToEnd(page, wearDownStep, 700);
    const seconds = Math.round((Date.now() - started) / 1000);
    test.info().annotations.push({ type: "exhaustion", description: `${steps} actions through the interface in ${seconds} s` });
    console.log(`exhaustion: ${steps} interface actions in ${seconds} s`);

    const title = (await endHeading(page).textContent()) ?? "";
    const winner = [REMPART_1, REMPART_2].find((c) => title === `${c} a gagné.`);
    expect(winner, `the title names a camp: ${title}`).toBeDefined();
    const loser = winner === REMPART_1 ? REMPART_2 : REMPART_1;
    await expect(page.getByText(`${loser} n'avait plus aucune action possible (épuisement).`, { exact: true })).toBeVisible();
    const blocks = await readBlocks(page.getByRole("region", { name: "Journal complet" }));
    expect(blocks.at(-1)!.lines.at(-1)).toBe(`${winner} gagne : ${loser} n'avait plus aucune action possible (épuisement).`);
  });
});
