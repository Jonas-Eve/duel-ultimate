// Shared helpers of the end-to-end tests. Everything here reads the DOM the player sees (roles,
// accessible names, French texts) and taps; nothing reaches the engine, and no cost, target or
// rule is recomputed: a test only compares a label read BEFORE an action with one read AFTER.
import AxeBuilder from "@axe-core/playwright";
import { expect, test as base, type Locator, type Page } from "@playwright/test";

/** What the page did that it must never do (decisions S1 and S3 of #64, checked on the build). */
export interface Guard {
  /** Anything unexpected: console errors, page errors, CSP violations, requests leaving the origin. */
  problems: string[];
}

/** The fixed seed of the scenarios that do not search for their own (noted here, on purpose). */
export const SEED = 42;

export const test = base.extend<{ guard: Guard }>({
  guard: [
    async ({ page, baseURL }, use, testInfo) => {
      const origin = new URL(baseURL ?? "http://127.0.0.1:4173").origin;
      const problems: string[] = [];
      const notes: string[] = [];
      // Survives navigations (exposed before the first `goto`). Reports `securitypolicyviolation`
      // events: the build's CSP is a <meta> tag, so a blocked WASM or inline script lands here.
      await page.exposeFunction("__reportViolation", (what: string) => {
        problems.push(`csp: ${what}`);
      });
      await page.addInitScript(() => {
        document.addEventListener("securitypolicyviolation", (e) => {
          (window as unknown as { __reportViolation(s: string): void }).__reportViolation(`${e.violatedDirective} ${e.blockedURI}`);
        });
      });
      page.on("console", (m) => {
        notes.push(`[console.${m.type()}] ${m.text()}`);
        if (m.type() === "error") problems.push(`console.error: ${m.text()}`);
      });
      page.on("pageerror", (e) => problems.push(`pageerror: ${e.message}`));
      page.on("request", (r) => {
        const u = new URL(r.url());
        if (u.protocol === "data:" || u.protocol === "blob:") return;
        if (u.origin !== origin) problems.push(`request outside the origin: ${r.url()}`);
      });

      await use({ problems });

      if (testInfo.status !== testInfo.expectedStatus) {
        // The manual e2e workflow archives no artifact: the log is the only diagnosis (see apps/web/README.md).
        console.log(`--- ${testInfo.title}: failed, what the player sees ---`);
        try {
          console.log(await page.locator("main").ariaSnapshot({ timeout: 3000 }));
        } catch {
          console.log("(no snapshot: the page is gone)");
        }
        console.log(`--- console and violations (${notes.length} / ${problems.length}) ---`);
        for (const n of notes) console.log(n);
        for (const p of problems) console.log(p);
      } else {
        expect(problems, "the page must stay silent: no CSP violation, console error or foreign request").toEqual([]);
      }
    },
    { auto: true },
  ],
});
export { expect };

// ---- Home ----------------------------------------------------------------------------------

/** Opens the app and waits until the engine is loaded (the start button is enabled). */
export async function openHome(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Choisis ton deck" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Lancer la partie" })).toBeEnabled();
}

export interface StartOptions {
  /** Against the computer (default) or both camps. */
  mode?: "computer" | "two-camps";
  /** Deck names as shown: "Ruée" or "Rempart". Against the computer: the human's deck. Two camps: camp 1. */
  deck?: string;
  /** Two camps only: camp 2's deck. */
  deckTwo?: string;
  /** Typed in the seed field; empty string leaves it blank. */
  seed?: string;
}

export async function startGame(page: Page, o: StartOptions = {}): Promise<void> {
  const mode = o.mode ?? "computer";
  if (mode === "two-camps") await page.getByRole("radio", { name: "Je joue les deux camps" }).check();
  if (mode === "computer") {
    if (o.deck !== undefined) await page.getByRole("group", { name: "Ton deck" }).getByRole("radio", { name: o.deck }).check();
  } else {
    if (o.deck !== undefined) await page.getByRole("group", { name: "Deck du camp 1" }).getByRole("radio", { name: o.deck }).check();
    if (o.deckTwo !== undefined) await page.getByRole("group", { name: "Deck du camp 2" }).getByRole("radio", { name: o.deckTwo }).check();
  }
  await page.getByLabel("Graine (optionnelle)").fill(o.seed ?? String(SEED));
  await page.getByRole("button", { name: "Lancer la partie" }).tap();
  await expect(page.getByRole("region", { name: "Partie" })).toBeVisible();
}

// ---- Reading the screen --------------------------------------------------------------------

export const gameRegion = (page: Page): Locator => page.getByRole("region", { name: "Partie" });
export const thinking = (page: Page): Locator => page.getByRole("status").filter({ hasText: "L'ordinateur joue…" });
export const timeBand = (page: Page): Locator => page.getByRole("group", { name: "Temps et jeton" });
export const endHeading = (page: Page): Locator => page.locator("#end-title");
/** The hand of the camp that holds the main (the last hand of the page: two camps show the other camp's above). */
export const ownHand = (page: Page): Locator => page.locator(".hand").last();
/** The two boards: [0] the top one (opponent / the camp that waits), [1] the bottom one (the camp that plays). */
export const topBoard = (page: Page): Locator => page.locator(".board").nth(0);
export const bottomBoard = (page: Page): Locator => page.locator(".board").nth(1);

/** The names of the cards of a row, as printed on the tiles. */
export const cardNames = (row: Locator): Promise<string[]> => row.locator(".card-name").allTextContents();

export const seedShown = async (page: Page): Promise<number> => {
  const text = (await gameRegion(page).getByText(/^Graine : \d+$/).textContent()) ?? "";
  return Number(text.replace("Graine : ", ""));
};

/** Opens the journal drawer and returns its lines grouped by block title; closes it again. */
export async function readJournal(page: Page): Promise<{ title: string; lines: string[] }[]> {
  await page.getByRole("button", { name: "Journal" }).tap();
  const dialog = page.getByRole("dialog", { name: "Journal" });
  await expect(dialog).toBeVisible();
  const blocks = await readBlocks(dialog);
  await dialog.getByRole("button", { name: "Fermer" }).tap();
  await expect(dialog).toBeHidden();
  return blocks;
}

/** The blocks of a `.log-history` inside `root`: heading text, then its lines. */
export function readBlocks(root: Locator): Promise<{ title: string; lines: string[] }[]> {
  return root.locator(".log-block").evaluateAll((els) =>
    els.map((el) => ({
      title: el.querySelector(".log-block-title")?.textContent ?? "",
      lines: [...el.querySelectorAll(".log-lines li")].map((li) => li.textContent ?? ""),
    })),
  );
}

// ---- Waiting and failing loudly --------------------------------------------------------------

/**
 * Waits until the computer's turn is over (or there never was one), and fails at once on what
 * must never appear in a game: the "Partie interrompue" screen or the refused-action banner.
 * (A refused action is unreachable from the interface by construction: covered by injection in
 * #63 and #64, so seeing it here is a defect.)
 */
export async function settle(page: Page): Promise<void> {
  await expect(thinking(page)).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Partie interrompue" })).toHaveCount(0);
  await expect(page.getByRole("alert")).toHaveCount(0);
}

export const isOver = async (page: Page): Promise<boolean> => (await endHeading(page).count()) > 0;

// ---- Bots driven by the DOM ----------------------------------------------------------------------

const effectButtons = (page: Page): Locator => page.locator(".btn-effect:not([disabled])");
export const creatureTargets = (page: Page): Locator => topBoard(page).locator("button.card-target");
export const heroTarget = (page: Page): Locator => page.getByRole("button", { name: /^Attaquer le héros/ });
export const confirmAttack = async (page: Page): Promise<void> => {
  await page.getByRole("dialog", { name: "Confirmer l'attaque" }).getByRole("button", { name: "Confirmer" }).tap();
  await expect(page.getByRole("dialog", { name: "Confirmer l'attaque" })).toHaveCount(0);
};

// ---- The card detail (#104 to #106) ------------------------------------------------------------------
// A card is touched to be read: every tile of a hand or a board opens a detail sheet (a dialog named by the card),
// and the actions live there: "Jouer (N temps)" for a hand card, the line "Attaque (N) : inflige D dégâts" for an
// own board creature. A tile is therefore never a criterion of playability any more: what the engine allows is
// read from the state of those two buttons (`aria-disabled`), nothing is recomputed.

/** The card detail sheet: the dialog that holds a `.card-detail` (the journal, the discard and the attack confirmation are others). */
export const detailSheet = (page: Page): Locator => page.getByRole("dialog").filter({ has: page.locator(".card-detail") });
/** The tiles (compact cards) of the hand of the camp that holds the main, and of each board. */
export const handTiles = (page: Page): Locator => ownHand(page).locator(".card");
export const ownBoardTiles = (page: Page): Locator => bottomBoard(page).locator(".card");
export const opponentBoardTiles = (page: Page): Locator => topBoard(page).locator(".card");
/** "Jouer (N temps)" of a hand card's sheet. */
export const playButton = (sheet: Locator): Locator => sheet.locator(".btn-play");
/** The attack line of an own creature's sheet, a button: "Attaque (N) : inflige D dégâts, pour N temps". */
export const attackLine = (sheet: Locator): Locator => sheet.getByRole("button", { name: /^Attaque \(\d+\) : inflige/ });
/** An action button is inactive (but still focusable) when the engine offers no preview of it. */
export const isInactive = async (button: Locator): Promise<boolean> => (await button.getAttribute("aria-disabled")) === "true";

/** Touches a tile and waits for its detail sheet; returns the sheet. */
export async function openDetail(page: Page, tileLocator: Locator): Promise<Locator> {
  await tileLocator.tap();
  const sheet = detailSheet(page);
  await expect(sheet).toBeVisible();
  return sheet;
}

/** Closes the open detail sheet with its "Fermer" button. */
export async function closeDetail(page: Page): Promise<void> {
  const sheet = detailSheet(page);
  await sheet.getByRole("button", { name: "Fermer" }).tap();
  await expect(sheet).toHaveCount(0);
}

/**
 * Opens the detail of a hand tile and taps "Jouer" if the engine offers it (true, the sheet closes);
 * otherwise closes the sheet and returns false. The state of the button is the only criterion.
 */
export async function playFromDetail(page: Page, tileLocator: Locator): Promise<boolean> {
  return (await playOrExplain(page, tileLocator)) === "played";
}

/**
 * `playFromDetail` that also tells why nothing was played: "board-full" when the sheet says the board
 * is full (the engine's reason, the same for every card of the hand: board limit of the standard rules,
 * so a caller may stop looking at the other cards), "unavailable" otherwise.
 */
async function playOrExplain(page: Page, tileLocator: Locator): Promise<"played" | "board-full" | "unavailable"> {
  const sheet = await openDetail(page, tileLocator);
  const play = playButton(sheet);
  if (await isInactive(play)) {
    const full = /Le plateau est plein/.test((await sheet.textContent()) ?? "");
    await closeDetail(page);
    return full ? "board-full" : "unavailable";
  }
  await play.tap();
  await expect(sheet).toHaveCount(0);
  return "played";
}

/**
 * Opens the detail of the own creature at `index` of the board and taps its attack line if the engine offers
 * it (true: the sheet closes and the creature is selected, "Choisis une cible"); otherwise closes the sheet
 * and returns false.
 */
export async function selectAttackerAt(page: Page, index: number): Promise<boolean> {
  const sheet = await openDetail(page, ownBoardTiles(page).nth(index));
  const line = attackLine(sheet);
  if (await isInactive(line)) {
    await closeDetail(page);
    return false;
  }
  await line.tap();
  await expect(sheet).toHaveCount(0);
  return true;
}

/** Selects the first own creature that can attack, from board position `from`; returns its position, or -1 if none can. */
export async function selectAttacker(page: Page, from = 0): Promise<number> {
  const n = await ownBoardTiles(page).count();
  for (let i = from; i < n; i++) if (await selectAttackerAt(page, i)) return i;
  return -1;
}

/** An own creature can attack (the sheet is opened and closed again; nothing is selected). */
export async function hasAttacker(page: Page): Promise<boolean> {
  const n = await ownBoardTiles(page).count();
  for (let i = 0; i < n; i++) {
    const sheet = await openDetail(page, ownBoardTiles(page).nth(i));
    const can = !(await isInactive(attackLine(sheet)));
    await closeDetail(page);
    if (can) return true;
  }
  return false;
}

/**
 * Selects the first creature that can attack, then the target (a creature if `prefer` is "creature" and one is
 * possible, else the hero), reads the confirmation sheet and confirms. Returns the text of the sheet (what the
 * engine announced), or null when no creature can attack.
 */
export async function attackWith(page: Page, prefer: "creature" | "hero"): Promise<string | null> {
  if ((await selectAttacker(page)) < 0) return null;
  if (prefer === "creature" && (await creatureTargets(page).count()) > 0) {
    await creatureTargets(page).first().tap();
  } else if (await heroTarget(page).count()) {
    await heroTarget(page).tap();
  } else {
    await creatureTargets(page).first().tap();
  }
  const sheet = await page.getByRole("dialog", { name: "Confirmer l'attaque" }).innerText();
  await confirmAttack(page);
  return sheet;
}

/** A bounded loop's budget error: names what to look at instead of hanging until the test timeout. */
export class BudgetExceeded extends Error {}

/**
 * The human's simple, deterministic policy against the computer: invoke the first playable card,
 * otherwise attack (the hero when possible), otherwise use the first hero effect. Never concedes.
 * One action per call; returns false if nothing could be done.
 */
export async function humanStep(page: Page): Promise<boolean> {
  if (await tapCard(page)) return true;
  if ((await attackWith(page, "hero")) !== null) return true;
  const fx = effectButtons(page);
  if ((await fx.count()) > 0) {
    await fx.first().tap();
    return true;
  }
  return false;
}

/** Plays `step` until the end screen, at most `max` actions; the count is returned. Fails loudly otherwise. */
export async function playToEnd(page: Page, step: (p: Page) => Promise<boolean>, max: number): Promise<number> {
  for (let n = 0; n < max; n++) {
    await settle(page);
    if (await isOver(page)) return n;
    if (!(await step(page))) throw new Error(`the bot found nothing to do after ${n} actions`);
  }
  await settle(page);
  if (await isOver(page)) return max;
  throw new BudgetExceeded(`the game did not end within ${max} actions`);
}

/**
 * The wear-down robot (fact V7: exhaustion never happens at random). In priority order: the
 * first active hero effect, then the first card of the hand of the camp that plays, then an
 * attack on an opposing creature (the next attacker if this one has no creature to hit), and,
 * as a last resort, an attack on the hero. Never "Abandonner". It reads the DOM only.
 */
export async function wearDownStep(page: Page): Promise<boolean> {
  const fx = effectButtons(page);
  if ((await fx.count()) > 0) {
    await fx.first().tap();
    return true;
  }
  if (await tapCard(page)) return true;
  for (let i = await selectAttacker(page); i >= 0; i = await selectAttacker(page, i + 1)) {
    if ((await creatureTargets(page).count()) > 0) {
      await creatureTargets(page).first().tap();
      await confirmAttack(page);
      return true;
    }
    await page.getByRole("button", { name: "Annuler la sélection" }).tap();
  }
  return (await attackWith(page, "hero")) !== null;
}

// ---- Mobile checks ------------------------------------------------------------------------------

/** The page itself never scrolls sideways (card rows scroll inside themselves). */
export async function expectNoHorizontalScroll(page: Page): Promise<void> {
  const { scrollWidth, clientWidth } = await page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    clientWidth: document.documentElement.clientWidth,
  }));
  expect(scrollWidth, "horizontal overflow of the page").toBeLessThanOrEqual(clientWidth);
}

/**
 * Every visible control is at least 44x44 CSS px. A radio button is judged by its label (the
 * 22 px box is only the drawing: the whole labelled row, 44 px high, is the tap target).
 */
export async function expectTouchTargets(page: Page): Promise<void> {
  const small = await page.evaluate(() => {
    const out: string[] = [];
    for (const el of document.querySelectorAll<HTMLElement>('button, summary, input, [role="button"]')) {
      const isChoice = el instanceof HTMLInputElement && (el.type === "radio" || el.type === "checkbox");
      const target = isChoice ? (el.closest("label") ?? el) : el;
      const r = target.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) continue; // not rendered
      if (r.width < 44 || r.height < 44) {
        const name = el.getAttribute("aria-label") ?? el.textContent?.trim().slice(0, 40) ?? el.tagName;
        out.push(`${el.tagName.toLowerCase()} "${name}" is ${Math.round(r.width)}x${Math.round(r.height)}`);
      }
    }
    return out;
  });
  expect(small, "touch targets under 44x44").toEqual([]);
}

/** Axe (WCAG 2.0/2.1/2.2 A and AA, real contrast included) finds nothing on the current screen. */
export async function expectAccessible(page: Page): Promise<void> {
  const result = await new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag22aa"]).analyze();
  expect(
    result.violations.map((v) => `${v.id}: ${v.help} (${v.nodes.map((n) => n.target.join(" ")).join(" | ")})`),
    "axe violations",
  ).toEqual([]);
}

/** The three mobile checks, with a label so that a failure says which screen. */
export async function expectMobileSound(page: Page, screen: string): Promise<void> {
  await test.step(`mobile checks: ${screen}`, async () => {
    await expectNoHorizontalScroll(page);
    await expectTouchTargets(page);
    await expectAccessible(page);
  });
}

// ---- Two camps ----------------------------------------------------------------------------------------

/** "Camp 1 — Ruée": the camp that holds the main, read from the time band ("Au tour de ..."). */
export async function activeCamp(page: Page): Promise<string> {
  const text = (await timeBand(page).locator("strong").textContent()) ?? "";
  return text.replace(/^Au tour de /, "");
}

/**
 * Plays the first card of the playing camp's hand whose name matches, from its detail ("Jouer"), or returns
 * false when none can be played (the state of "Jouer" decides: during the computer's turn it is inactive).
 */
export async function tapCard(page: Page, name?: RegExp): Promise<boolean> {
  const tiles = handTiles(page).filter(name === undefined ? {} : { has: page.locator(".card-name").filter({ hasText: name }) });
  const n = await tiles.count();
  for (let i = 0; i < n; i++) {
    const result = await playOrExplain(page, tiles.nth(i));
    if (result === "played") return true;
    if (result === "board-full") return false; // the same reason for every card: skip the others
  }
  return false;
}

/** Taps the first hero effect that can be used, or returns false. */
export async function tapEffect(page: Page): Promise<boolean> {
  const fx = page.locator(".btn-effect:not([disabled])");
  if ((await fx.count()) === 0) return false;
  await fx.first().tap();
  return true;
}

/** A tile of a board, by the name printed on it. */
export const tile = (board: Locator, name: string): Locator => board.locator(".card").filter({ has: board.page().locator(".card-name", { hasText: name }) });

/**
 * Bounded scenario search: `step` plays one action (whatever policy the scenario needs) until
 * `reached` holds, at most `max` actions. Fails loudly, naming the budget, if it never does or
 * if the game ends first, so that a changed starter set or seed cannot pass silently.
 */
export async function playUntil(page: Page, step: (p: Page) => Promise<boolean>, reached: (p: Page) => Promise<boolean>, max: number, what: string): Promise<number> {
  for (let n = 0; n <= max; n++) {
    await settle(page);
    if (await isOver(page)) throw new Error(`the game ended after ${n} actions before reaching: ${what}`);
    if (await reached(page)) return n;
    if (n === max) break;
    if (!(await step(page))) throw new Error(`no action available after ${n} actions, before reaching: ${what}`);
  }
  throw new Error(`not reached within ${max} actions: ${what}`);
}
