// The "I play both camps" mode through the interface, on the real WASM engine: the camp holding the
// main is at the bottom, both hands are visible, the camps swap when the main passes, nothing is
// hidden but the decks' content, and no computer is ever involved.
import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { CARDS_JSON, DECKS_JSON } from "../src/data/starter";
import type { CardView, Seat } from "../src/engine/types";
import { isPlay } from "../src/engine/types";
import type { Snapshot } from "../src/game/controller";
import { expectedCardLabel } from "./support/card-label";
import { newSession } from "./support/engine";
import { launch, renderApp, useAppEngine } from "./support/app";
import { playHuman, playUntil, randomPreview, stallPreview } from "./support/policies";
import { inAct, makeCampsController, mountGame, useRealEngine } from "./support/ui";
import { playTile } from "./support/play";

useRealEngine();
useAppEngine();

const CAMP1 = "Camp 1 — Ruée";
const CAMP2 = "Camp 2 — Rempart";
const group = (name: string) => screen.getByRole("group", { name });
const buttonsIn = (name: string) => within(group(name)).queryAllByRole("button") as HTMLButtonElement[];
const baseLabel = (c: CardView) => expectedCardLabel(c, { inHand: true });
const seats: Seat[] = ["One", "Two"];

/** First seed where the camp holding the main (rush, One) has a summon that hands the main over without ending the game. */
function seedWithHandOver(): number {
  for (let seed = 0; seed < 80; seed++) {
    const s = newSession("rush", "bulwark", seed);
    try {
      if (s.legalActionsWithTime().some((p) => isPlay(p.action) && p.hands_over && p.ends_game === null)) return seed;
    } finally {
      s.dispose();
    }
  }
  throw new Error("no seed found");
}

/** The index (in the active camp's hand) of a summon that hands the main over, from the engine previews. */
function handOverIndex(s: Snapshot): number {
  const v = s.views[s.views.One!.active]!;
  const preview = s.legal.find((p) => isPlay(p.action) && p.hands_over && p.ends_game === null)!;
  return v.you.hand.findIndex((c) => isPlay(preview.action) && c.id === preview.action.play.card);
}

describe("the screen: both hands visible, the camp holding the main at the bottom", () => {
  test("both hands are named, the active camp plays and the other camp's tiles are not controls", () => {
    const c = makeCampsController("rush", "bulwark", 3);
    mountGame(c);
    const s = c.getSnapshot();
    expect(s.views.One!.active).toBe("One"); // Ruée has the initiative

    const mine = s.views.One!.you.hand;
    const theirs = s.views.Two!.you.hand;
    const bottom = buttonsIn(`${CAMP1} : main`);
    expect(bottom.map((b) => b.getAttribute("aria-label")!.startsWith(`${mine[bottom.indexOf(b)].name}, coût`))).toEqual(mine.map(() => true));
    for (const b of bottom) expect(b.getAttribute("aria-haspopup")).toBe("dialog");

    // The other camp's hand is named in full too; its tiles open a read-only detail (#104), they never play.
    const tiles = buttonsIn(`${CAMP2} : main`);
    expect(tiles.map((t) => t.getAttribute("aria-label"))).toEqual(theirs.map((card) => `${baseLabel(card)}`));
    for (const t of tiles) {
      expect(t.getAttribute("aria-haspopup")).toBe("dialog");
      expect(t.disabled).toBe(false);
    }

    // Heroes and counters of both camps (decks are counts only).
    for (const seat of seats) {
      const side = s.views[seat]!.you;
      const label = seat === "One" ? CAMP1 : CAMP2;
      const hero = within(group(`${label} : héros`));
      expect(hero.getByText(side.hero.name)).toBeTruthy();
      expect(hero.getByText(`Main : ${side.hand.length}`)).toBeTruthy();
      expect(hero.getByText(`Pioche : ${side.deck_count}`)).toBeTruthy();
    }
    expect(within(group("Temps et jeton")).getByText(`Au tour de ${CAMP1}`)).toBeTruthy();
    expect(within(group("Temps et jeton")).getByText(`Temps : ${s.views.One!.budget}`)).toBeTruthy();
  });

  test("the mirror: same names on both sides, told apart by the camp number", () => {
    const c = makeCampsController("rush", "rush", 5);
    mountGame(c);
    expect(group("Camp 1 — Ruée : main")).not.toBe(group("Camp 2 — Ruée : main"));
    expect(group("Camp 1 — Ruée : plateau")).not.toBe(group("Camp 2 — Ruée : plateau"));
    expect(group("Camp 1 — Ruée : héros")).not.toBe(group("Camp 2 — Ruée : héros"));
  });

  test("no computer wording anywhere, the game seed stays shown, no computer seed", () => {
    const c = makeCampsController("rush", "bulwark", 3);
    const { container } = mountGame(c);
    expect(screen.getByText("Graine : 3")).toBeTruthy();
    expect(screen.queryByRole("status")).toBeNull();
    expect(container.textContent).not.toMatch(/ordinateur|À toi|adversaire|Tu /i);
    // The info panel has the data version and no computer seed.
    const info = within(container.querySelector("details") as HTMLElement);
    expect(info.getByText(/Données : starter@1\.1\.0/)).toBeTruthy();
    expect(container.querySelector("details")!.textContent).not.toMatch(/ordinateur|Graine/i);
  });

  test("the contents of the decks are never shown: a card name absent from hands, boards, discards and the log is not in the DOM", async () => {
    const c = makeCampsController("rush", "bulwark", 3);
    const { container } = mountGame(c);
    await inAct(() => playUntil(c, stallPreview, (s) => s.records.length >= 3, 60));
    const s = c.getSnapshot();
    const known = new Set<string>();
    for (const seat of seats) {
      const side = s.views[seat]!.you;
      for (const card of [...side.hand, ...side.board, ...side.discard]) known.add(card.name ?? "");
    }
    for (const r of s.records) for (const note of Object.values(r.cards)) known.add(note.name ?? "");
    const cards = JSON.parse(CARDS_JSON) as { id: string; name: string }[];
    const nameOf = new Map(cards.map((x) => [x.id, x.name]));
    const decks = JSON.parse(DECKS_JSON) as { cards: string[] }[];
    const hidden = new Set(decks.flatMap((d) => d.cards.map((id) => nameOf.get(id)!)).filter((n) => !known.has(n)));
    // The scenario is only meaningful if some name is really unknown at this point.
    expect(hidden.size).toBeGreaterThan(0);
    for (const name of hidden) expect(container.textContent).not.toContain(name);
    for (const name of known) expect(container.textContent).toContain(name);
  });
});

describe("the swap when the main passes", () => {
  test("top and bottom swap at once, the label and the announcement change, no intermediate step", async () => {
    const seed = seedWithHandOver();
    const c = makeCampsController("rush", "bulwark", seed);
    const { user, container } = mountGame(c);
    const before = c.getSnapshot();
    const index = handOverIndex(before);
    const tile = buttonsIn(`${CAMP1} : main`)[index];
    expect(tile.getAttribute("aria-label")).toContain("passe la main");
    expect(screen.queryAllByText(`Au tour de ${CAMP2}`)).toHaveLength(0);

    await playTile(user, tile);

    // The very first render after the action already shows the new camp holding the main.
    const after = c.getSnapshot();
    expect(after.views.One!.active).toBe("Two");
    expect(after.records.length).toBe(before.records.length + 1);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(container.textContent).not.toMatch(/appareil|passe(z)? (le|l')|prêt|continuer/i);
    expect(screen.queryByRole("status")).toBeNull();

    // The camp 2 hand is now at the bottom and playable; the camp 1 hand is shown, not playable.
    const nowBottom = buttonsIn(`${CAMP2} : main`);
    expect(nowBottom).toHaveLength(after.views.Two!.you.hand.length);
    for (const b of nowBottom) expect(b.getAttribute("aria-haspopup")).toBe("dialog");
    const nowTop = buttonsIn(`${CAMP1} : main`);
    expect(nowTop).toHaveLength(after.views.One!.you.hand.length);
    for (const t of nowTop) expect(t.getAttribute("aria-haspopup")).toBe("dialog");

    // The vertical order really swapped: camp 2's board is below camp 1's.
    const position = (name: string) => group(name).compareDocumentPosition(group(`${CAMP2} : plateau`));
    expect(position(`${CAMP1} : plateau`) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    // The band names the camp holding the main, and a polite live region announces it.
    expect(within(group("Temps et jeton")).getByText(`Au tour de ${CAMP2}`)).toBeTruthy();
    expect(within(group("Temps et jeton")).getByText(`Temps : ${after.views.Two!.budget}`)).toBeTruthy();
    const live = screen.getAllByText(`Au tour de ${CAMP2}`).find((el) => el.closest('[aria-live="polite"]') !== null);
    expect(live).toBeTruthy();

    // Focus is not left on a removed tile: it lands on the game region.
    expect(document.activeElement).toBe(screen.getByRole("region", { name: "Partie" }));
  });

  test("after the swap the concede dialog asks for the camp now at the bottom", async () => {
    const seed = seedWithHandOver();
    const c = makeCampsController("rush", "bulwark", seed);
    const { user } = mountGame(c);
    await playTile(user, buttonsIn(`${CAMP1} : main`)[handOverIndex(c.getSnapshot())]);
    expect(c.getSnapshot().views.One!.active).toBe("Two");
    await user.click(screen.getByRole("button", { name: "Abandonner" }));
    expect(within(screen.getByRole("dialog")).getByRole("heading", { name: "Le camp 2 abandonne ?" })).toBeTruthy();
  });

  test("the active camp's hero effect is playable (a draw, named in the log of a two-camps game)", async () => {
    const c = makeCampsController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    const before = c.getSnapshot().records.length;
    await user.click(within(group("Actions")).getByRole("button", { name: /^Rappel, coût 2/ }));
    expect(c.getSnapshot().records.length).toBe(before + 1);
    expect(c.getSnapshot().records.at(-1)!.events.some((e) => "drew" in e)).toBe(true);
  });

  test("no computer is ever started: the policy is never called over a whole game", async () => {
    const policy = vi.fn(() => 0);
    const c = makeCampsController("rush", "bulwark", 2, { policy });
    mountGame(c);
    const { humanActions, snapshot } = await inAct(() => playHuman(c, randomPreview(1), 400));
    expect(humanActions).toBeGreaterThan(0);
    expect(snapshot.phase).toBe("over");
    expect(policy).not.toHaveBeenCalled();
  });
});

describe("the log names everything", () => {
  test("both camps' draws are named, in blocks titled with the camp", async () => {
    const c = makeCampsController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    await inAct(() =>
      playUntil(
        c,
        stallPreview,
        (s) => {
          const drawers = new Set(s.records.flatMap((r) => r.events.flatMap((e) => ("drew" in e ? [e.drew.player] : []))));
          return drawers.has("One") && drawers.has("Two");
        },
        100,
      ),
    );
    await user.click(screen.getByRole("button", { name: "Journal" }));
    const drawer = within(screen.getByRole("dialog", { name: "Journal" }));
    expect(drawer.getAllByRole("heading", { name: CAMP1 }).length).toBeGreaterThan(0);
    expect(drawer.getAllByRole("heading", { name: CAMP2 }).length).toBeGreaterThan(0);
    for (const label of [CAMP1, CAMP2]) {
      expect(drawer.getAllByText(new RegExp(`^${label} pioche \\d+ cartes? : .+\\.$`)).length).toBeGreaterThan(0);
    }
    expect(drawer.queryByText(/pioche \d+ cartes?\.$/)).toBeNull();
  });
});

describe("the three ends of a two-camps game", () => {
  const title = () => screen.getByRole("heading", { level: 2 });

  test("hero down: the winning camp is named, with the deck, no computer seed", async () => {
    const c = makeCampsController("rush", "bulwark", 2);
    const { container } = mountGame(c);
    const { snapshot } = await inAct(() => playHuman(c, randomPreview(1), 400));
    const result = snapshot.views.One!.result!;
    expect(result.reason).toBe("hero_down");
    const winner = result.winner === "One" ? CAMP1 : CAMP2;
    const loser = result.winner === "One" ? CAMP2 : CAMP1;
    expect(title().textContent).toBe(`${winner} a gagné.`);
    const heroOf = snapshot.views[result.winner === "One" ? "Two" : "One"]!.you.hero.name;
    expect(screen.getByText(`Le héros ${heroOf} est tombé à 0 PV.`)).toBeTruthy();
    expect(screen.getByText("Graine : 2")).toBeTruthy();
    expect(container.textContent).not.toMatch(/Tu as gagné|ordinateur/i);
    expect(screen.getByRole("heading", { name: "Journal complet" })).toBeTruthy();
    expect(screen.getAllByRole("heading", { name: loser }).length).toBeGreaterThan(0);
  });

  // 220 actions rendered through jsdom: ~3.5 s alone in the full suite, 7+ s under CPU contention;
  // the default 5 s timeout flaked in CI (#77).
  test("exhaustion: both camps in wear-down play, the pinned case of #63 (220 actions)", async () => {
    const c = makeCampsController("rush", "bulwark", 2);
    const { container } = mountGame(c);
    const { humanActions, snapshot } = await inAct(() => playHuman(c, stallPreview));
    expect(humanActions).toBe(220);
    const result = snapshot.views.One!.result!;
    expect(result.reason).toBe("exhaustion");
    const [winner, loser] = result.winner === "One" ? [CAMP1, CAMP2] : [CAMP2, CAMP1];
    expect(title().textContent).toBe(`${winner} a gagné.`);
    expect(screen.getByText(`${loser} n'avait plus aucune action possible (épuisement).`)).toBeTruthy();
    expect(container.textContent).not.toMatch(/Tu as gagné|ordinateur|Graine de/i);
    expect(screen.getByText("Graine : 2")).toBeTruthy();
  }, 30_000);

  test("concede: the dialog asks for the camp at the bottom, the other camp wins", async () => {
    const c = makeCampsController("rush", "bulwark", 3);
    const { user, container } = mountGame(c);
    await user.click(screen.getByRole("button", { name: "Abandonner" }));
    const dialog = within(screen.getByRole("dialog"));
    expect(dialog.getByRole("heading", { name: "Le camp 1 abandonne ?" })).toBeTruthy();
    expect(dialog.getByText(/Camp 2 — Rempart gagne/)).toBeTruthy();
    await user.click(dialog.getByRole("button", { name: "Abandonner" }));
    expect(title().textContent).toBe(`${CAMP2} a gagné.`);
    expect(screen.getByText(`${CAMP1} a abandonné.`)).toBeTruthy();
    expect(container.textContent).not.toMatch(/Tu as gagné|ordinateur|Graine de/i);
  });
});

describe("replay through App", () => {
  test("same seed: the same two starting hands, the mode kept, the old instance freed", async () => {
    const { user, counters } = renderApp();
    await launch(user, { twoCamps: true, deckOne: "Ruée", deckTwo: "Rempart", seed: "7" });
    const hands = () => [`${CAMP1} : main`, `${CAMP2} : main`].map((n) => [...group(n).querySelectorAll("[aria-label]")].map((b) => b.getAttribute("aria-label")));
    const first = hands();
    await user.click(screen.getByRole("button", { name: "Abandonner" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Abandonner" }));
    expect(counters).toMatchObject({ creations: 1, frees: 0 });

    await user.click(screen.getByRole("button", { name: "Rejouer avec la même graine" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Abandonner" })).toBeTruthy());
    expect(hands()).toEqual(first);
    expect(screen.getByText("Graine : 7")).toBeTruthy();
    expect(screen.queryByRole("status")).toBeNull();
    expect(counters).toMatchObject({ creations: 2, frees: 1, callsAfterFree: 0 });
  });
});

describe("wording of the consequence lines in two camps", () => {
  test("no 'te' / 'adversaire': the other camp is the other camp", async () => {
    const seed = seedWithHandOver();
    const c = makeCampsController("rush", "bulwark", seed);
    const { user } = mountGame(c);
    const tile = buttonsIn(`${CAMP1} : main`)[handOverIndex(c.getSnapshot())];
    expect(tile.textContent).toContain("Passe la main");
    const notes = buttonsIn(`${CAMP1} : main`).map((b) => `${b.textContent} ${b.getAttribute("aria-label")}`).join(" ");
    expect(notes).not.toMatch(/te restera|adversaire/);
    // The sentence itself is in the sheet, worded for two camps.
    await user.click(tile);
    const sheet = screen.getByRole("dialog").textContent ?? "";
    expect(sheet).toMatch(/Passe la main : l'autre camp aura \d+ de temps/);
    expect(sheet).not.toMatch(/te restera|adversaire/);
  });
});

describe("the other camp's hand is captioned", () => {
  test("a visible caption says it is not playable", () => {
    const c = makeCampsController("rush", "bulwark", 3);
    mountGame(c);
    expect(screen.getByText(`Main de ${CAMP2} (non jouable)`)).toBeTruthy();
  });
});
