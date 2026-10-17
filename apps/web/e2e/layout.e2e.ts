// Cases 7 and 8 of #68: the phone (360 px, the default viewport of this project) and a wide
// screen (1280 px). On every screen: no sideways scroll of the page, every control at least
// 44x44 px, axe (WCAG 2.0/2.1/2.2 A and AA, real contrast) without a violation. The silence of the
// page itself (no CSP violation, console error or foreign request: S1 and S3 of #64) is checked
// for every test by the `guard` fixture, and a CSP that blocked the WASM would leave the game
// screen unreachable (the start button stays disabled) so these tests could not get this far.
import {
  SEED,
  expect,
  expectMobileSound,
  expectNoHorizontalScroll,
  gameRegion,
  humanStep,
  openHome,
  ownHand,
  playToEnd,
  selectAttacker,
  settle,
  startGame,
  tapCard,
  test,
  endHeading,
} from "./support";

test.describe("7. Mobile 360 px", () => {
  test("viewport de 360 px", async ({ page }) => {
    await openHome(page);
    expect(page.viewportSize()?.width).toBe(360);
  });

  test("accueil, deux modes", async ({ page }) => {
    await openHome(page);
    await expectMobileSound(page, "accueil, contre l'ordinateur");
    await page.getByRole("radio", { name: "Je joue les deux camps" }).check();
    await expect(page.getByRole("group", { name: "Deck du camp 2" })).toBeVisible();
    await expectMobileSound(page, "accueil, deux camps");
    await page.getByLabel("Graine (optionnelle)").fill("abc");
    await page.getByRole("button", { name: "Lancer la partie" }).tap();
    await expect(page.getByRole("alert")).toBeVisible();
    await expectMobileSound(page, "accueil, graine refusée");
  });

  test("partie après le tour de l'ordinateur, journal, défausse, abandon, écran de fin", async ({ page }) => {
    await openHome(page);
    // The computer (Ruée) opens: the screen the human gets back once its turn is over.
    await startGame(page, { deck: "Rempart", seed: String(SEED) });
    await settle(page);
    await expectMobileSound(page, "partie, après le tour de l'ordinateur");

    // The hand is a row that scrolls inside itself (overflow-x: auto), but five compact cards (#92) fit a 360 px screen: it no longer overflows.
    const row = await ownHand(page).evaluate((el) => ({ scroll: el.scrollWidth, client: el.clientWidth, overflowX: getComputedStyle(el).overflowX }));
    expect(row.overflowX).toBe("auto");
    expect(row.scroll, "a hand of five compact cards fits a 360 px screen without overflowing").toBeLessThanOrEqual(row.client);
    await expectNoHorizontalScroll(page);

    await page.getByRole("button", { name: "Journal" }).tap();
    await expect(page.getByRole("dialog", { name: "Journal" })).toBeVisible();
    await expectMobileSound(page, "tiroir du journal");
    await page.getByRole("dialog", { name: "Journal" }).getByRole("button", { name: "Fermer" }).tap();

    await page.getByRole("group", { name: "Ton héros" }).getByRole("button", { name: /^Défausse/ }).tap();
    await expect(page.getByRole("dialog", { name: /^Défausse/ })).toBeVisible();
    await expectMobileSound(page, "dialogue de défausse");
    await page.getByRole("dialog", { name: /^Défausse/ }).getByRole("button", { name: "Fermer" }).tap();

    await page.getByRole("button", { name: "Abandonner" }).tap();
    await expect(page.getByRole("dialog", { name: "Abandonner la partie ?" })).toBeVisible();
    await expectMobileSound(page, "confirmation d'abandon");
    await page.getByRole("button", { name: "Continuer" }).tap();

    // Leave through the end screen (conceding): its own screen to check.
    await page.getByRole("button", { name: "Abandonner" }).tap();
    await page.getByRole("dialog").getByRole("button", { name: "Abandonner" }).tap();
    await expect(endHeading(page)).toBeVisible();
    await expectMobileSound(page, "écran de fin, abandon");
  });

  test("feuille d'attaque et attaquant sélectionné, puis écran de fin d'un duel entier", async ({ page }) => {
    await openHome(page);
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    expect(await tapCard(page)).toBe(true);
    await settle(page);
    expect(await selectAttacker(page)).toBeGreaterThanOrEqual(0);
    await expect(page.getByRole("button", { name: "Annuler la sélection" })).toBeVisible();
    await expectMobileSound(page, "partie, attaquant sélectionné");
    await page.getByRole("button", { name: /^Attaquer le héros/ }).tap();
    await expect(page.getByRole("dialog", { name: "Confirmer l'attaque" })).toBeVisible();
    await expectMobileSound(page, "feuille d'attaque");
    await page.getByRole("button", { name: "Annuler", exact: true }).tap();
    await page.getByRole("button", { name: "Annuler la sélection" }).tap();

    await playToEnd(page, humanStep, 300);
    await expectMobileSound(page, "écran de fin, duel entier avec son journal");
  });
});

test.describe("7b. CSP du build (S3 de #64)", () => {
  test("la politique refuse un script en ligne, un gestionnaire en ligne et un attribut style, et le WASM s'exécute quand même", async ({ page, guard }) => {
    await openHome(page); // the engine loaded: the page is playable, so 'wasm-unsafe-eval' lets the WASM run
    const csp = await page.locator('meta[http-equiv="Content-Security-Policy"]').getAttribute("content");
    expect(csp, "the build carries its CSP").not.toBeNull();
    expect(csp).not.toMatch(/unsafe-inline|(?<!wasm-)unsafe-eval/);
    expect(csp).toContain("script-src 'self' 'wasm-unsafe-eval'");

    // Probes: what an injected script, an inline handler and a style attribute would do. All must be
    // blocked. (`eval` cannot be probed from here: code run through the DevTools protocol is not
    // subject to the page's eval restriction, so the policy text above is what guards it.)
    const probe = await page.evaluate(
      () =>
        new Promise<{ violated: string[]; inlineRan: boolean; handlerRan: boolean }>((resolve) => {
          const violated: string[] = [];
          document.addEventListener("securitypolicyviolation", (e) => violated.push(e.violatedDirective));
          const script = document.createElement("script");
          script.textContent = "window.__inlineRan = true";
          document.head.append(script);
          const el = document.createElement("button");
          el.setAttribute("onclick", "window.__handlerRan = true");
          el.setAttribute("style", "color: red");
          document.body.append(el);
          el.click();
          setTimeout(() => resolve({ violated, inlineRan: "__inlineRan" in window, handlerRan: "__handlerRan" in window }), 100);
        }),
    );
    expect(probe.inlineRan, "an inline script must not run").toBe(false);
    expect(probe.handlerRan, "an inline handler must not run").toBe(false);
    expect(probe.violated).toEqual(expect.arrayContaining(["script-src-elem", "script-src-attr", "style-src-attr"]));
    // These violations are the point of this test: the guard must not count them as a defect.
    // (The browser also logs each violation as a console error.)
    expect(guard.problems.every((p) => p.startsWith("csp:") || /Content Security Policy/.test(p))).toBe(true);
    guard.problems.length = 0;
  });
});

test.describe("8. Viewport large 1280 px", () => {
  // Still a touch-capable context (the helpers tap); only the size and the mobile emulation change.
  test.use({ viewport: { width: 1280, height: 800 }, isMobile: false });

  const expectCentredColumn = async (page: import("@playwright/test").Page) => {
    await expectNoHorizontalScroll(page);
    const box = (await page.locator("main").boundingBox())!;
    expect(box.width, "a readable column, not the whole screen").toBeLessThanOrEqual(768);
    expect(Math.abs(box.x + box.width / 2 - 640), "centred").toBeLessThanOrEqual(2);
  };

  test("accueil et partie lisibles, colonne centrée, sans débordement", async ({ page }) => {
    await openHome(page);
    await expectCentredColumn(page);
    await expect(page.getByRole("button", { name: "Lancer la partie" })).toBeInViewport();
    await startGame(page, { deck: "Ruée", seed: String(SEED) });
    await expectCentredColumn(page);
    await expect(ownHand(page).locator(".card-name").first()).toBeVisible();
    await expect(gameRegion(page).getByText(/^Graine : \d+$/)).toBeVisible();
  });
});
