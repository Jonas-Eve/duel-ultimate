// Playing a card from the detail of my hand (#105): a touch opens the sheet, "Jouer (N temps)" plays,
// "Fermer" and Escape do nothing. Real WASM engine, no mock of the rules.
import { act, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import type { ActionPreview, CardView } from "../src/engine/types";
import { isPlay } from "../src/engine/types";
import type { Snapshot } from "../src/game/controller";
import { messageFor } from "../src/ui/errors.fr";
import { fr } from "../src/ui/fr";
import { decorateSession, newSession, newSessionFromSetup, rulesFactory } from "./support/engine";
import { newKindsSetup } from "./support/new-kinds";
import { playButton } from "./support/play";
import { inAct, makeCampsController, makeController, mountGame, useRealEngine } from "./support/ui";
import { cardView } from "./support/card-view";

useRealEngine();

const hand = () => screen.getByRole("group", { name: "Ta main" });
const tiles = () => within(hand()).getAllByRole("button") as HTMLButtonElement[];
const region = () => screen.getByRole("region", { name: "Partie" });
const previewOf = (s: Snapshot, card: CardView): ActionPreview => s.legal.find((p) => isPlay(p.action) && p.action.play.card === card.id) as ActionPreview;
/** The first card of the opening hand whose play keeps the main (resp. hands it over). */
const firstWhere = (s: Snapshot, keeps: boolean): number =>
  s.views.One!.you.hand.findIndex((c) => {
    const p = previewOf(s, c);
    return p.ends_game === null && p.hands_over !== keeps;
  });

describe("opening a hand card", () => {
  test("a touch opens the detail with Jouer (N temps) and Fermer, and applies nothing", async () => {
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3));
    const c = makeController("rush", "bulwark", 3, { createSession: () => session });
    const { user } = mountGame(c);
    const s = c.getSnapshot();
    const card = s.views.One!.you.hand[0];
    const p = previewOf(s, card);
    const tile = tiles()[0];
    expect(tile.getAttribute("aria-haspopup")).toBe("dialog");
    expect(tile.disabled).toBe(false);

    await user.click(tile);

    const dialog = screen.getByRole("dialog", { name: card.name! });
    expect(within(dialog).getAllByRole("button").map((b) => b.textContent)).toEqual([`Jouer (${p.cost} temps)`, "Fermer"]);
    expect(probe.calls.apply).toBe(0);
    expect(c.getSnapshot().records).toHaveLength(s.records.length);
  });

  test("Escape and Fermer close without any action and give the focus back to the tile", async () => {
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3));
    const c = makeController("rush", "bulwark", 3, { createSession: () => session });
    const { user } = mountGame(c);
    await user.click(tiles()[1]);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(tiles()[1]);

    await user.click(tiles()[1]);
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Fermer" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(tiles()[1]);
    expect(probe.calls.apply).toBe(0);
  });

  test("the flow works with the keyboard alone: Enter on the tile, Tab to Jouer, Enter plays", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    const before = c.getSnapshot();
    tiles()[0].focus();
    await user.keyboard("{Enter}");
    expect(document.activeElement).toBe(screen.getByRole("dialog"));
    // The scrollable effects zone is a tab stop of its own, then comes Jouer.
    for (let i = 0; i < 3 && document.activeElement !== playButton(); i++) await user.tab();
    expect(document.activeElement).toBe(playButton());
    await user.keyboard("{Enter}");
    expect(c.getSnapshot().records.length).toBeGreaterThan(before.records.length);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("Jouer", () => {
  test("plays the previewed action once, closes the sheet, moves the focus to the game region", async () => {
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3));
    const c = makeController("rush", "bulwark", 3, { createSession: () => session });
    const { user } = mountGame(c);
    const s = c.getSnapshot();
    const index = firstWhere(s, true);
    const card = s.views.One!.you.hand[index];
    const p = previewOf(s, card);

    await user.click(tiles()[index]);
    await user.click(playButton());

    expect(probe.applied).toEqual([p.action]);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(tiles()).toHaveLength(s.views.One!.you.hand.length - 1);
    expect(within(screen.getByRole("group", { name: "Temps et jeton" })).getByText(`Temps : ${p.budget_after}`)).toBeTruthy();
    expect(document.activeElement).toBe(region());
  });

  test("a double press plays once and shows no error", async () => {
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3));
    const c = makeController("rush", "bulwark", 3, { createSession: () => session });
    const { user } = mountGame(c);
    await user.click(tiles()[0]);
    await user.dblClick(playButton());
    await waitFor(() => expect(c.getSnapshot().phase).not.toBe("computer-turn"));
    expect(probe.applied.filter((a) => isPlay(a as never))).toHaveLength(1);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  test("the consequence is the preview's, as text: 'keeps the main' read from the engine, spoken form as the description", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    const s = c.getSnapshot();
    const index = firstWhere(s, true);
    const p = previewOf(s, s.views.One!.you.hand[index]);
    await user.click(tiles()[index]);
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(`Coût : ${p.cost}`)).toBeTruthy();
    const said = within(dialog).getByText(`Il te restera ${p.budget_after} de temps`);
    expect(playButton().getAttribute("aria-describedby")).toBe(said.closest("[id]")!.id);
    expect(within(dialog).queryByText(/Passe la main/)).toBeNull();
  });

  test("a hand-over is announced in the sheet, from the preview", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    const s = c.getSnapshot();
    const index = firstWhere(s, false);
    expect(index).toBeGreaterThanOrEqual(0);
    const p = previewOf(s, s.views.One!.you.hand[index]);
    await user.click(tiles()[index]);
    expect(within(screen.getByRole("dialog")).getByText(`Passe la main : l'adversaire aura ${p.budget_after} de temps`)).toBeTruthy();
  });

  test("an end of game is read from the preview too", async () => {
    const phantom: CardView = cardView({ id: 999999, name: "Fantôme", cost: 2, force: 1, attack_cost: 1, health: 1 });
    const injected: ActionPreview = { action: { play: { card: 999999 } }, cost: 2, hands_over: true, budget_after: null, ends_game: { winner: "One", reason: "exhaustion" }, attacker_destroyed: false };
    const { session } = decorateSession(newSession("rush", "bulwark", 3), {
      injectPreview: injected,
      forceView: (seat, v) => (seat === "One" ? { ...v, you: { ...v.you, hand: [...v.you.hand, phantom] } } : v),
    });
    const c = makeController("rush", "bulwark", 3, { createSession: () => session });
    const { user } = mountGame(c);
    await user.click(within(hand()).getByRole("button", { name: /^Fantôme/ }));
    const text = screen.getByRole("dialog").textContent ?? "";
    expect(text).toContain("Passe la main et met fin à la partie : Tu gagnes.");
    expect(playButton().textContent).toBe("Jouer (2 temps)");
  });
});

describe("no preview: Jouer is inactive and says why", () => {
  test("the computer's turn: the detail opens, Jouer is aria-disabled and focusable, with a visible reason, a press does nothing", async () => {
    const gate = { yield: () => new Promise<void>(() => undefined) };
    const { session, probe } = decorateSession(newSession("bulwark", "rush", 3));
    const c = makeController("bulwark", "rush", 3, { scheduler: gate, createSession: () => session });
    const { user } = mountGame(c);
    expect(c.getSnapshot().phase).toBe("computer-turn");
    const card = c.getSnapshot().views.One!.you.hand[0];
    const applied = probe.calls.apply;

    await user.click(tiles()[0]);

    const dialog = screen.getByRole("dialog", { name: card.name! });
    const play = within(dialog).getByRole("button", { name: "Jouer" });
    expect(play.getAttribute("aria-disabled")).toBe("true");
    expect((play as HTMLButtonElement).disabled).toBe(false);
    const reason = within(dialog).getByText(fr.playUnavailableComputer);
    expect(play.getAttribute("aria-describedby")).toBe(reason.id);
    await user.click(play);
    expect(probe.calls.apply).toBe(applied);
    expect(screen.getByRole("dialog")).toBeTruthy();
    play.focus();
    expect(document.activeElement).toBe(play);
    // Reading stays possible: Escape closes.
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("a sheet left open when the main passes turns inactive at once, and its press plays nothing", async () => {
    const gate = { yield: () => new Promise<void>(() => undefined) };
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3));
    const c = makeController("rush", "bulwark", 3, { scheduler: gate, createSession: () => session });
    const { user } = mountGame(c);
    const s = c.getSnapshot();
    const handOver = s.legal.find((p) => isPlay(p.action) && p.hands_over && p.ends_game === null)!;
    const handOverId = isPlay(handOver.action) ? handOver.action.play.card : -1;
    const other = s.views.One!.you.hand.findIndex((card) => card.id !== handOverId);
    await user.click(tiles()[other]);
    expect(playButton().getAttribute("aria-disabled")).toBeNull();

    await act(async () => {
      c.submit(handOver);
    });

    expect(c.getSnapshot().phase).toBe("computer-turn");
    const play = within(screen.getByRole("dialog")).getByRole("button", { name: "Jouer" });
    expect(play.getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByText(fr.playUnavailableComputer)).toBeTruthy();
    const applied = probe.calls.apply;
    await user.click(play);
    expect(probe.calls.apply).toBe(applied);
  });
});

// A full board (#171): the engine says why a card cannot be played, the sheet shows that reason. One creature
// of a board limited to one is enough; the client never compares a board with a limit.
describe("a full board: Jouer is inactive and says why, in the engine's words", () => {
  const LIMIT_ONE = rulesFactory('{"hero_riposte":false,"board_limit":1}');
  const boardFull = messageFor("board_full");

  /** The human summons a card that keeps the main: back on its turn with a full board and cards in hand. */
  async function fullBoard(c: ReturnType<typeof makeController>) {
    const s = c.getSnapshot();
    const card = s.views.One!.you.hand[firstWhere(s, true)];
    await inAct(async () => {
      c.submit(previewOf(s, card));
    });
    expect(c.getSnapshot().views.One!.you.board).toHaveLength(1);
    expect(c.getSnapshot().views.One!.you.hand.length).toBeGreaterThan(0);
  }

  test("the engine's code is on the card, and its French text is the visible reason, linked to the focusable inactive button", async () => {
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3, LIMIT_ONE));
    const c = makeController("rush", "bulwark", 3, { createSession: () => session });
    const { user } = mountGame(c);
    await fullBoard(c);
    const card = c.getSnapshot().views.One!.you.hand[0];
    expect(card.unavailable).toBe("board_full");
    const applied = probe.calls.apply;

    await user.click(tiles()[0]);

    const dialog = screen.getByRole("dialog", { name: card.name! });
    const play = within(dialog).getByRole("button", { name: "Jouer" });
    expect(play.getAttribute("aria-disabled")).toBe("true");
    expect((play as HTMLButtonElement).disabled).toBe(false);
    const reason = within(dialog).getByText(boardFull);
    expect(play.getAttribute("aria-describedby")).toBe(reason.id);
    expect(within(dialog).queryByText(fr.playUnavailable)).toBeNull();
    play.focus();
    expect(document.activeElement).toBe(play);
    await user.click(play);
    expect(probe.calls.apply).toBe(applied);
    expect(screen.getByRole("dialog")).toBeTruthy();
  });

  test("a card that can be played stays playable while the board has room", async () => {
    const roomy = rulesFactory('{"hero_riposte":false,"board_limit":2}');
    const c = makeController("rush", "bulwark", 3, {}, roomy);
    const { user } = mountGame(c);
    await fullBoard(c);
    expect(c.getSnapshot().views.One!.you.hand[0].unavailable).toBeUndefined();
    await user.click(tiles()[0]);
    expect(playButton().getAttribute("aria-disabled")).toBeNull();
  });

  test("during the computer's turn the engine's reason comes before 'the computer is playing'", async () => {
    const gate = { yield: () => new Promise<void>(() => undefined) };
    const c = makeController("rush", "bulwark", 3, { scheduler: gate }, LIMIT_ONE);
    const { user } = mountGame(c);
    const s = c.getSnapshot();
    const card = s.views.One!.you.hand[firstWhere(s, false)];
    await inAct(async () => {
      c.submit(previewOf(s, card));
    });
    expect(c.getSnapshot().phase).toBe("computer-turn");

    await user.click(tiles()[0]);

    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(boardFull)).toBeTruthy();
    expect(within(dialog).queryByText(fr.playUnavailableComputer)).toBeNull();
    expect(within(dialog).getByRole("button", { name: "Jouer" }).getAttribute("aria-disabled")).toBe("true");
  });

  test("two camps: the hand of the other camp stays read-only, whatever the engine says of it", async () => {
    const c = makeCampsController("rush", "bulwark", 3, {}, LIMIT_ONE);
    const { user } = mountGame(c);
    await fullBoard(c);
    const top = within(screen.getByRole("group", { name: "Camp 2 — Rempart : main" })).getAllByRole("button");
    await user.click(top[0]);
    expect(within(screen.getByRole("dialog")).queryByRole("button", { name: /^Jouer/ })).toBeNull();
  });
});

describe("the deck symbol on the sheet of my own hand", () => {
  test("the sheet carries my deck's symbol and name", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    await user.click(tiles()[0]);
    expect(within(screen.getByRole("dialog")).getByRole("img", { name: "Deck Ruée" })).toBeTruthy();
  });
});

describe("two camps", () => {
  const CAMP1 = "Camp 1 — Ruée";
  const bottom = () => within(screen.getByRole("group", { name: `${CAMP1} : main` })).getAllByRole("button") as HTMLButtonElement[];

  test("a sheet opened from the camp holding the main is dropped when the main passes, never reopened as a read-only hand", async () => {
    let seed = -1;
    for (let k = 0; k < 80 && seed < 0; k++) {
      const probe = newSession("rush", "bulwark", k);
      try {
        const n = probe.view("One").you.hand.length;
        const ho = probe.legalActionsWithTime().filter((p) => isPlay(p.action) && p.hands_over && p.ends_game === null);
        if (n >= 2 && ho.length > 0) seed = k;
      } finally {
        probe.dispose();
      }
    }
    expect(seed).toBeGreaterThanOrEqual(0);
    const c = makeCampsController("rush", "bulwark", seed);
    const { user } = mountGame(c);
    const s = c.getSnapshot();
    const handOver = s.legal.find((p) => isPlay(p.action) && p.hands_over && p.ends_game === null)!;
    const handOverId = isPlay(handOver.action) ? handOver.action.play.card : -1;
    const other = s.views.One!.you.hand.findIndex((card) => card.id !== handOverId);
    await user.click(bottom()[other]);
    expect(screen.getByRole("dialog")).toBeTruthy();

    await inAct(async () => {
      c.submit(handOver);
    });

    expect(c.getSnapshot().views.One!.active).toBe("Two");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("the camp now holding the main can play from its own sheet, with the two-camps wording", async () => {
    const c = makeCampsController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    const s = c.getSnapshot();
    const handOver = s.legal.find((p) => isPlay(p.action) && p.hands_over && p.ends_game === null);
    expect(handOver).toBeDefined();
    await inAct(async () => {
      c.submit(handOver!);
    });
    const two = within(screen.getByRole("group", { name: "Camp 2 — Rempart : main" })).getAllByRole("button");
    const before = c.getSnapshot();
    await user.click(two[0]);
    expect(screen.getByRole("dialog").textContent).not.toMatch(/te restera|adversaire/);
    await user.click(playButton());
    expect(c.getSnapshot().records.length).toBeGreaterThan(before.records.length);
  });
});

describe("a spell in hand (#210)", () => {
  test("a spell in hand opens its sheet and Play says why it cannot be played", async () => {
    const { session, probe } = decorateSession(newSessionFromSetup(newKindsSetup(), 3));
    const c = makeController("rush", "bulwark", 3, { createSession: () => session });
    const { user } = mountGame(c);
    const tile = within(hand()).getByRole("button", { name: "zz-spell, coût 1, sort" });
    await user.click(tile);
    const dialog = screen.getByRole("dialog", { name: "zz-spell" });
    const play = within(dialog).getByRole("button", { name: "Jouer" });
    expect(play.getAttribute("aria-disabled")).toBe("true");
    play.focus();
    expect(document.activeElement).toBe(play);
    const reason = dialog.querySelector(".play-reason")!;
    expect(reason.textContent).toBe(messageFor("unsupported_effect"));
    expect(reason.textContent).toBe("Cette carte utilise un effet que le jeu ne gère pas encore.");
    expect(play.getAttribute("aria-describedby")).toBe(reason.id);
    await user.click(play);
    expect(probe.calls.apply).toBe(0);
    expect(screen.getByRole("dialog")).toBe(dialog);
  });
});
