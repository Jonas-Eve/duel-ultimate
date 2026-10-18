// #174 (story #127, epic #120): the hero's riposte, the board limit and the reason of an unplayable card, seen
// from the player's seat. The production build sends no rules (`Game.from_data` without `rules_json`), so it plays
// the standard rules (the hero rips the attacker, board limit); nothing here pins them: the cases search their
// situation by a bounded loop that reads the DOM, with the constant seeds written below. Production build (CSP,
// hashed .wasm), real engine, real computer, nothing mocked; the expected texts are literal (fr.ts: `attention`,
// `attackerDestroyed`, `log.heroRiposted`; errors.fr.ts: `board_full`), never imported from the app.
import type { Page } from "@playwright/test";
import {
  BOARD_FULL_TEXT,
  SEED,
  activeCamp,
  attackSheet,
  cancelHeroAttack,
  closeDetail,
  confirmAttack,
  endHeading,
  expect,
  expectAccessible,
  expectNoHorizontalScroll,
  expectTouchTargets,
  handTiles,
  hasAttacker,
  humanStep,
  isInactive,
  isOver,
  openDetail,
  openHome,
  ownBoardTiles,
  peekHeroAttack,
  playButton,
  playToEnd,
  playUntil,
  playUntilBoardFull,
  readBlocks,
  readJournal,
  settle,
  startGame,
  tapCard,
  tapEffect,
  test,
  timeBand,
} from "./support";

// Constant seeds. All the deterministic cases are two camps (Ruée camp 1 opens, Rempart camp 2) and use SEED (42),
// the suite's seed; the whole duels against the computer use COMPUTER_SEED. If a seed ever stops reaching its
// situation, change it here (and say why), never skip a case.
const COMPUTER_SEED = 7;
const CAMP_1 = "Camp 1 — Ruée";
const CAMP_2 = "Camp 2 — Rempart";
const WARNING = "Attention : La créature qui attaque sera détruite.";
const LETHAL = "Met fin à la partie";

const twoCamps = (page: Page) => startGame(page, { mode: "two-camps", deck: "Ruée", deckTwo: "Rempart", seed: String(SEED) });

/** Camp 1 invokes (hero effects only to get cards or the hand back), Rempart only draws: no provocation ever stands in the way. */
const sacrificeStep = async (p: Page): Promise<boolean> =>
  (await activeCamp(p)) === CAMP_1 ? (await tapCard(p)) || (await tapEffect(p)) : (await tapEffect(p)) || (await tapCard(p, /^(?!Garde du rempart)/)) || (await tapCard(p));

/** Both camps invoke as long as they can, then use a hero effect (which ends by handing the hand over). */
const fillStep = async (p: Page): Promise<boolean> => (await tapCard(p)) || (await tapEffect(p));

/** Reaches "camp 1 holds the main and a creature of its own can attack". */
const reachAttacker = (page: Page) =>
  playUntil(page, sacrificeStep, async (p) => (await activeCamp(p)) === CAMP_1 && (await hasAttacker(p)), 40, "Ruée can attack with a creature");

const heroHealth = async (page: Page, camp: string): Promise<number> => {
  const text = (await page.getByRole("group", { name: `${camp} : héros`, exact: true }).textContent()) ?? "";
  const m = /PV (\d+)/.exec(text);
  expect(m, `the hero bar of ${camp} shows its health: ${text}`).not.toBeNull();
  return Number(m![1]);
};

const journalLines = async (page: Page): Promise<string[]> => (await readJournal(page)).flatMap((b) => b.lines);

test.describe("règles : riposte du héros et limite de plateau", () => {
  test("1. attaque non létale du héros : l'avertissement, puis l'attaquant détruit est à la défausse et au journal", async ({ page }) => {
    await openHome(page);
    await twoCamps(page);
    await reachAttacker(page);

    // What the tile says BEFORE the attack: the name and the force (the "attaque N" of its label).
    const tileLabel = (await ownBoardTiles(page).first().getAttribute("aria-label")) ?? "";
    const m = /^(.+?), attaque (\d+),/.exec(tileLabel);
    expect(m, `the tile label gives name and force: ${tileLabel}`).not.toBeNull();
    const [name, force] = [m![1], Number(m![2])];
    const heroBefore = await heroHealth(page, CAMP_2);

    const sheet = await peekHeroAttack(page);
    expect(sheet).not.toBeNull();
    expect(sheet).toContain(WARNING);
    expect(sheet).toMatch(/Coût d'attaque : \d+/);
    expect(sheet).toMatch(/passe la main|restera/);
    expect(sheet).not.toContain(LETHAL);
    await confirmAttack(page);
    await settle(page);

    // The creature left the board of its owner (the camp may have changed: look at the journal for the order).
    const lines = await journalLines(page);
    const rip = lines.indexOf(`Le héros Gardienne du rempart riposte et détruit ${name}.`);
    expect(rip, `the hero's riposte is in the journal: ${lines.slice(-8).join(" / ")}`).toBeGreaterThanOrEqual(0);
    expect(lines[rip + 1], "then the death, right after").toBe(`${name} meurt.`);
    expect(await heroHealth(page, CAMP_2), "the hero lost the force read on the tile").toBe(heroBefore - force);

    // In the public discard of its camp: destroyed, never "PV 0".
    await page.getByRole("group", { name: `${CAMP_1} : héros`, exact: true }).getByRole("button", { name: /^Défausse \(\d+\)$/ }).tap();
    const discard = page.getByRole("dialog", { name: `Défausse : ${CAMP_1} : héros` });
    await expect(discard.getByRole("group", { name: new RegExp(`^${name},`) })).toBeVisible();
    await expect(discard).toContainText("Détruite");
    await expect(discard).not.toContainText("PV 0");
    await discard.getByRole("button", { name: "Fermer" }).tap();
    await expect(discard).toHaveCount(0);
  });

  test("2. attaque létale : pas d'avertissement, fin de partie, l'attaquant n'est pas détruit", async ({ page }) => {
    test.setTimeout(120_000);
    await openHome(page);
    await twoCamps(page);

    // Sacrifices (each non lethal attack destroys its attacker) until the sheet announces the end of the game.
    let drawer: string[] = [];
    let last = "";
    for (let n = 0; n < 150; n++) {
      await playUntil(page, sacrificeStep, async (p) => (await activeCamp(p)) === CAMP_1 && (await hasAttacker(p)), 60, "Ruée can attack the hero");
      const sheet = (await peekHeroAttack(page))!;
      if (sheet.includes(LETHAL)) {
        last = sheet;
        await cancelHeroAttack(page); // nothing changed: read the drawer's journal, then attack again
        drawer = await journalLines(page);
        await peekHeroAttack(page);
        break;
      }
      expect(sheet, "a non lethal attack warns").toContain(WARNING);
      await confirmAttack(page);
    }
    expect(last, "the loop reached a lethal attack (150 sacrifices at most)").not.toBe("");
    expect(last).not.toContain("Attention");
    expect(last).toContain(`Met fin à la partie : ${CAMP_1} gagne.`);
    await confirmAttack(page);

    await expect(endHeading(page)).toHaveText(`${CAMP_1} a gagné.`);
    await expect(page.getByText("Le héros Gardienne du rempart est tombé à 0 PV.", { exact: true }).first()).toBeVisible();
    const full = await readBlocks(page.getByRole("region", { name: "Journal complet" }));
    const all = full.flatMap((b) => b.lines);
    // The end screen's log is the drawer's log (read just before the last attack) plus the lethal attack's block.
    expect(all.slice(0, drawer.length)).toEqual(drawer);
    const lethalBlock = all.slice(drawer.length);
    expect(lethalBlock.length).toBeGreaterThan(0);
    expect(lethalBlock.join("\n")).not.toMatch(/riposte|meurt/);
    expect(full.at(-1)!.lines.join("\n")).not.toMatch(/riposte|meurt/);
  });

  test("3. plateau plein : « Jouer » grisé et expliqué, une carte jouable ailleurs reste jouable", async ({ page }) => {
    test.setTimeout(120_000);
    await openHome(page);
    await twoCamps(page);
    // Camp 1 invokes; Rempart draws first so that its own board is not the one that fills.
    await playUntilBoardFull(page, async (p) => ((await activeCamp(p)) === CAMP_1 ? fillStep(p) : (await tapEffect(p)) || (await tapCard(p))), 200, CAMP_1);

    const boardBefore = await ownBoardTiles(page).count();
    const bandBefore = await timeBand(page).innerText();
    const journalBefore = await journalLines(page);

    const sheet = await openDetail(page, handTiles(page).first());
    const play = playButton(sheet);
    await expect(play).toHaveAttribute("aria-disabled", "true");
    const reason = sheet.getByText(BOARD_FULL_TEXT, { exact: true });
    await expect(reason).toBeVisible();
    expect(BOARD_FULL_TEXT).not.toMatch(/\d/); // the sentence has no number: the limit is the engine's business
    const reasonId = await reason.getAttribute("id");
    expect(reasonId).toBeTruthy();
    await expect(play).toHaveAttribute("aria-describedby", reasonId!);
    await play.focus();
    await expect(play).toBeFocused();
    await play.tap({ force: true }); // aria-disabled: Playwright waits for "enabled" otherwise; the press does nothing
    await expect(sheet).toBeVisible();
    await expect(play).toHaveAttribute("aria-disabled", "true");
    // Every card of the hand has the same reason.
    await closeDetail(page);
    const count = await handTiles(page).count();
    for (let i = 0; i < count; i++) {
      const s = await openDetail(page, handTiles(page).nth(i));
      expect(await isInactive(playButton(s)), `card ${i} of the hand is not playable`).toBe(true);
      await expect(s).toContainText(BOARD_FULL_TEXT);
      if (i === 0) await page.keyboard.press("Escape");
      else await closeDetail(page);
      await expect(s).toHaveCount(0);
    }
    expect(await ownBoardTiles(page).count()).toBe(boardBefore);
    expect(await timeBand(page).innerText()).toBe(bandBefore);
    expect(await journalLines(page)).toEqual(journalBefore);

    // Hand over (hero effects only), then Rempart, whose board is not full, plays its card from the detail.
    for (let n = 0; n < 30 && (await activeCamp(page)) === CAMP_1; n++) {
      await settle(page);
      expect(await tapEffect(page), "a hero effect hands the main over").toBe(true);
    }
    expect(await activeCamp(page)).toBe(CAMP_2);
    const rempartCards = () => page.getByRole("group", { name: `${CAMP_2} : plateau`, exact: true }).locator(".card").count(); // by name: the boards swap when the hand changes
    const rempartBoard = await rempartCards();
    expect(await tapCard(page), "a card playable elsewhere is still playable").toBe(true);
    expect(await rempartCards()).toBe(rempartBoard + 1);
  });

  test("4. deux camps : mêmes textes quel que soit le camp, la main de l'autre camp reste en lecture seule", async ({ page }) => {
    test.setTimeout(150_000);
    await openHome(page);
    await twoCamps(page);

    const seen: { camp: string; warning: string[]; full: string }[] = [];
    for (const camp of [CAMP_1, CAMP_2]) {
      await playUntilBoardFull(page, fillStep, 250, camp);
      const sheet = await openDetail(page, handTiles(page).first());
      const full = (await sheet.locator(".play-reason").innerText()).trim();
      // The other camp's hand is above: read only, whichever camp that is.
      await closeDetail(page);
      const otherHand = page.locator(".hand").first().locator(".card").first();
      const other = await openDetail(page, otherHand);
      await expect(playButton(other)).toHaveCount(0);
      await closeDetail(page);
      // The attack on the hero, if a creature can: the warning, read and cancelled.
      const text = await peekHeroAttack(page);
      expect(text, `${camp} has a creature that can attack the hero`).not.toBeNull();
      const warning = text!.split("\n").filter((l) => l.includes("Attention"));
      await cancelHeroAttack(page);
      seen.push({ camp, warning, full });
    }
    expect(seen[0].full).toBe(BOARD_FULL_TEXT);
    expect(seen[1].full).toBe(seen[0].full);
    expect(seen[0].warning).toEqual([WARNING]);
    expect(seen[1].warning).toEqual(seen[0].warning);
    expect(await isOver(page)).toBe(false);
  });

  for (const deck of ["Ruée", "Rempart"] as const) {
    test(`5. contre l'ordinateur (${deck}) : une partie entière sans pose refusée ni abandon`, async ({ page }) => {
      test.setTimeout(120_000);
      await openHome(page);
      await startGame(page, { deck, seed: String(COMPUTER_SEED) });
      // humanStep plays through the detail sheets: it only taps what the engine offers; `settle` fails on any alert.
      await playToEnd(page, humanStep, 400);
      await expect(endHeading(page)).toHaveText(/^(Tu as gagné !|L'ordinateur a gagné\.)$/);
      const reason = (await page.locator(".end-screen p").first().textContent()) ?? "";
      expect(reason).toMatch(/^(Le héros .* est tombé à 0 PV\.|.* n'avai(s|t) plus aucune action possible \(épuisement\)\.)$/);
      const lines = (await readBlocks(page.getByRole("region", { name: "Journal complet" }))).flatMap((b) => b.lines);
      expect(lines.filter((l) => /abandonn/.test(l)), "nobody conceded").toEqual([]);
      await expect(page.getByRole("alert")).toHaveCount(0);
      await expect(page.getByText(BOARD_FULL_TEXT)).toHaveCount(0);
      await expectNoHorizontalScroll(page);
    });
  }

  test.describe("6. 360 x 640", () => {
    test.use({ viewport: { width: 360, height: 640 } });

    test("la feuille d'attaque avec avertissement et le détail grisé : texte et bordure, 44 px, axe", async ({ page }) => {
      test.setTimeout(150_000);
      await openHome(page);
      await twoCamps(page);
      await reachAttacker(page);
      expect(await peekHeroAttack(page)).toContain(WARNING);
      await expectNoHorizontalScroll(page);
      // Signalled by text AND by a border (read from the computed style, not from a colour).
      const warning = attackSheet(page).locator(".attack-warning");
      await expect(warning).toContainText("Attention :");
      const border = await warning.evaluate((el) => parseFloat(getComputedStyle(el).borderInlineStartWidth));
      expect(border, "the warning has a side border of at least 3 px").toBeGreaterThanOrEqual(3);
      for (const name of ["Annuler", "Confirmer"]) {
        const box = await attackSheet(page).getByRole("button", { name }).boundingBox();
        expect(box!.height, `${name} height`).toBeGreaterThanOrEqual(44);
        expect(box!.width, `${name} width`).toBeGreaterThanOrEqual(44);
      }
      await expectTouchTargets(page);
      await expectAccessible(page);
      await cancelHeroAttack(page);

      // The unplayable card: the greyed "Jouer" is still a 44 px target, the detail has no violation.
      await playUntilBoardFull(page, fillStep, 200, CAMP_1);
      const sheet = await openDetail(page, handTiles(page).first());
      await expect(playButton(sheet)).toHaveAttribute("aria-disabled", "true");
      await expect(sheet).toContainText(BOARD_FULL_TEXT);
      const box = await playButton(sheet).boundingBox();
      expect(box!.height).toBeGreaterThanOrEqual(44);
      await expectNoHorizontalScroll(page);
      await expectTouchTargets(page);
      await expectAccessible(page);
    });
  });
});

