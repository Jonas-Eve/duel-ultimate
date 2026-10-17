// The read-only card detail (#104): the sheet itself, then its wiring on the real WASM engine
// (opponent board and the other camp's hand open it; nothing is ever submitted by it).
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test, vi } from "vitest";
import type { CardView } from "../src/engine/types";
import { isAttackBy } from "../src/engine/types";
import type { Snapshot } from "../src/game/controller";
import { CardDetailSheet } from "../src/ui/CardDetailSheet";
import { expectedCardLabel } from "./support/card-label";
import { selectAttacker } from "./support/play";
import { playUntil, randomPreview } from "./support/policies";
import { makeCampsController, makeController, mountGame, useRealEngine } from "./support/ui";

const card = (over: Partial<CardView> = {}): CardView => ({
  id: 7,
  name: "Garde du rempart",
  cost: 3,
  force: 1,
  attack_cost: 2,
  health: 6,
  provocation: false,
  riposte: 0,
  ...over,
});

const PROVOCATION = "Provocation : tant qu'elle est en jeu, l'adversaire ne peut attaquer qu'une créature qui provoque ; son héros n'est pas ciblable.";

/** The text of the elements a node points to by `aria-describedby` (what a screen reader announces after the name). */
function described(el: Element): string {
  const ids = (el.getAttribute("aria-describedby") ?? "").split(/\s+/).filter(Boolean);
  return ids
    .map((id) => {
      const target = document.getElementById(id);
      return target?.getAttribute("aria-label") ?? target?.textContent ?? "";
    })
    .join(" ");
}

function sheet(over: Partial<CardView> = {}, onClose = vi.fn()) {
  const user = userEvent.setup();
  const utils = render(<CardDetailSheet card={card(over)} emblem="battlements" deckName="Rempart" onClose={onClose} />);
  return { user, onClose, dialog: screen.getByRole("dialog"), ...utils };
}

describe("the sheet", () => {
  afterEach(cleanup);

  test("a modal dialog named by the card's name, focus on the sheet itself, no visible duplicate heading", () => {
    const { dialog } = sheet();
    expect(screen.getByRole("dialog", { name: "Garde du rempart" })).toBe(dialog);
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(document.activeElement).toBe(dialog);
    expect(dialog.querySelector(".dialog-title")!.classList.contains("sr-only")).toBe(true);
    expect(dialog.querySelectorAll(".card-detail-name")).toHaveLength(1);
  });

  test("shows the cost big with its hourglass, and the three values with captions, riposte 0 included", () => {
    const { dialog } = sheet({ riposte: 0 });
    const cost = dialog.querySelector(".card-detail-cost")!;
    expect(cost.textContent).toBe("Coût 3 temps");
    expect(cost.querySelector("svg")).not.toBeNull();
    const values = [...dialog.querySelectorAll(".card-detail-value")].map((v) => v.textContent);
    expect(values).toEqual(["Attaque1", "Riposte0", "Vie6"]);
  });

  test("the life is the current life alone, never 'N / max'", () => {
    const { dialog } = sheet({ health: 4 });
    expect(dialog.textContent).not.toMatch(/\d\s*\/\s*\d/);
  });

  test("the deck symbol is an image named by the deck", () => {
    sheet();
    const symbol = screen.getByRole("img", { name: "Deck Rempart" });
    expect(symbol.classList.contains("card-emblem")).toBe(true);
    expect(symbol.closest(".card-detail-head")).not.toBeNull();
  });

  test("the attack is one effect line: visible text plus a spoken addition, whose whole text is the accessible name", () => {
    const { dialog } = sheet({ force: 2, attack_cost: 3 });
    const lines = dialog.querySelectorAll(".card-detail-effect");
    expect(lines).toHaveLength(1);
    expect(lines[0].textContent).toBe("Attaque (3) : inflige 2 dégâts, pour 3 temps");
    const spoken = lines[0].querySelector(".sr-only")!;
    expect(spoken.textContent).toBe(", pour 3 temps");
    // The visible part is exactly the label minus the spoken addition (label in name).
    expect(lines[0].textContent!.replace(spoken.textContent!, "")).toBe("Attaque (3) : inflige 2 dégâts");
    expect(lines[0].querySelector("[aria-hidden='true']")).not.toBeNull(); // the hourglass
  });

  test("a passive effect is explained in French; none without the keyword", () => {
    const { dialog, unmount } = sheet({ provocation: true });
    expect(dialog.querySelector(".card-detail-passive")!.textContent).toBe(PROVOCATION);
    unmount();
    const plain = sheet();
    expect(plain.dialog.querySelector(".card-detail-passive")).toBeNull();
  });

  test("what the sheet announces after its name: cost, values (riposte 0 too), each effect line, each passive effect", () => {
    const { dialog, unmount } = sheet();
    expect(described(dialog)).toBe("Coût 3 temps. Attaque 1, riposte 0, vie 6. Attaque (2) : inflige 1 dégât, pour 2 temps. Deck Rempart");
    unmount();
    const withKeyword = sheet({ provocation: true, riposte: 2 });
    expect(described(withKeyword.dialog)).toBe(
      `Coût 3 temps. Attaque 1, riposte 2, vie 6. Attaque (2) : inflige 1 dégât, pour 2 temps. ${PROVOCATION} Deck Rempart`,
    );
  });

  test("read-only: 'Fermer' is the only button", () => {
    const { dialog } = sheet({ provocation: true });
    const buttons = within(dialog).getAllByRole("button");
    expect(buttons.map((b) => b.textContent)).toEqual(["Fermer"]);
  });

  test("the effects zone scrolls and is focusable, with an accessible name", () => {
    const { dialog } = sheet();
    const zone = dialog.querySelector(".card-detail-effects")!;
    expect(zone.getAttribute("tabindex")).toBe("0");
    expect(zone.getAttribute("role")).toBe("region");
    expect(zone.getAttribute("aria-label")).toBe("Effets");
  });

  test("the art slot is a fixed placeholder, decorative", () => {
    const { dialog } = sheet();
    const art = dialog.querySelector(".card-detail-art")!;
    expect(art).not.toBeNull();
    expect(art.querySelector("svg")!.getAttribute("aria-hidden")).toBe("true");
  });

  test("'Fermer' and Escape close", async () => {
    const a = sheet();
    await a.user.click(screen.getByRole("button", { name: "Fermer" }));
    expect(a.onClose).toHaveBeenCalledTimes(1);
    a.unmount();
    const b = sheet();
    await b.user.keyboard("{Escape}");
    expect(b.onClose).toHaveBeenCalledTimes(1);
  });

  test("Shift+Tab from the sheet itself goes to the last control; Tab never leaves", async () => {
    const { user, dialog } = sheet();
    expect(document.activeElement).toBe(dialog);
    await user.tab({ shift: true });
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Fermer" }));
    for (let i = 0; i < 5; i++) {
      await user.tab();
      expect(dialog.contains(document.activeElement)).toBe(true);
    }
  });

  test("an actions slot is rendered above 'Fermer' when given (for the next tasks)", () => {
    render(<CardDetailSheet card={card()} deckName="Rempart" onClose={() => undefined} actions={<button type="button">Jouer</button>} />);
    const buttons = screen.getAllByRole("button").map((b) => b.textContent);
    expect(buttons).toEqual(["Jouer", "Fermer"]);
  });
});

describe("on the real engine", () => {
  useRealEngine();

  const oppBoard = () => screen.getByRole("group", { name: "Plateau adverse" });

  async function withOpponentCreature() {
    const c = makeController("rush", "bulwark", 3);
    await playUntil(c, randomPreview(3), (s) => (s.views.One?.opponent.board.length ?? 0) >= 1 && (s.views.One?.you.board.length ?? 0) >= 1 && s.phase === "human-turn", 60);
    return c;
  }

  test("an opponent board tile is a button with a popup, its accessible name unchanged; a touch opens the read-only sheet and submits nothing", async () => {
    const c = await withOpponentCreature();
    const submit = vi.spyOn(c, "submit");
    const { user } = mountGame(c);
    const before = c.getSnapshot();
    const theirs = before.views.One!.opponent.board;
    const tiles = within(oppBoard()).getAllByRole("button");
    expect(tiles.map((t) => t.getAttribute("aria-label"))).toEqual(theirs.map((x) => expectedCardLabel(x)));
    for (const t of tiles) expect(t.getAttribute("aria-haspopup")).toBe("dialog");

    await user.click(tiles[0]);
    const dialog = screen.getByRole("dialog", { name: theirs[0].name! });
    expect(within(dialog).getAllByRole("button").map((b) => b.textContent)).toEqual(["Fermer"]);
    expect(screen.getByRole("img", { name: "Deck Rempart" })).toBeTruthy();
    expect(submit).not.toHaveBeenCalled();
    expect(c.getSnapshot().records.length).toBe(before.records.length);
  });

  test("Fermer and Escape close it and the focus returns to the tile", async () => {
    const c = await withOpponentCreature();
    const { user } = mountGame(c);
    const tile = () => within(oppBoard()).getAllByRole("button")[0];
    await user.click(tile());
    await user.click(screen.getByRole("button", { name: "Fermer" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(tile());
    await user.click(tile());
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(tile());
  });

  test("the sheet shows the card's own values, from the view", async () => {
    const c = await withOpponentCreature();
    const { user } = mountGame(c);
    const first = c.getSnapshot().views.One!.opponent.board[0];
    await user.click(within(oppBoard()).getAllByRole("button")[0]);
    const dialog = screen.getByRole("dialog");
    expect([...dialog.querySelectorAll(".card-detail-value")].map((v) => v.textContent)).toEqual([`Attaque${first.force}`, `Riposte${first.riposte}`, `Vie${first.health}`]);
    expect(dialog.querySelector(".card-detail-effect")!.textContent).toContain(`(${first.attack_cost})`);
  });

  test("during the choice of a target, a touch on an opposing card still chooses it: the detail never opens", async () => {
    const c = await withOpponentCreature();
    const { user } = mountGame(c);
    const s: Snapshot = c.getSnapshot();
    const mine = s.views.One!.you.board;
    const attacker = mine.findIndex((x) => s.legal.some((p) => isAttackBy(x.id)(p.action)));
    expect(attacker).toBeGreaterThanOrEqual(0);
    await selectAttacker(user, within(screen.getByRole("group", { name: "Ton plateau" })).getAllByRole("button")[attacker]);
    const targets = within(oppBoard())
      .getAllByRole("button")
      .filter((b) => b.getAttribute("aria-disabled") !== "true");
    expect(targets.length).toBeGreaterThan(0);
    await user.click(targets[0]);
    expect(screen.getByRole("dialog", { name: "Confirmer l'attaque" })).toBeTruthy();
    expect(screen.queryByRole("img", { name: /^Deck / })).toBeNull();
  });

  test("two camps: the other camp's hand tiles are popup buttons, never playable; a touch opens the same sheet with the cost", async () => {
    const c = makeCampsController("rush", "bulwark", 3);
    const submit = vi.spyOn(c, "submit");
    const { user } = mountGame(c);
    const s = c.getSnapshot();
    const theirs = s.views.Two!.you.hand;
    const hand = screen.getByRole("group", { name: "Camp 2 — Rempart : main" });
    const tiles = within(hand).getAllByRole("button");
    expect(tiles.map((t) => t.getAttribute("aria-label"))).toEqual(theirs.map((x) => expectedCardLabel(x, { inHand: true })));
    for (const t of tiles) {
      expect(t.getAttribute("aria-haspopup")).toBe("dialog");
      expect((t as HTMLButtonElement).disabled).toBe(false);
    }
    await user.click(tiles[0]);
    const dialog = screen.getByRole("dialog", { name: theirs[0].name! });
    expect(dialog.querySelector(".card-detail-cost")!.textContent).toBe(`Coût ${theirs[0].cost} temps`);
    expect(screen.getByRole("img", { name: "Deck Rempart" })).toBeTruthy();
    expect(within(dialog).getAllByRole("button").map((b) => b.textContent)).toEqual(["Fermer"]);
    expect(submit).not.toHaveBeenCalled();
    await user.keyboard("{Escape}");
    expect(document.activeElement).toBe(within(hand).getAllByRole("button")[0]);
  });

  test("my own board tiles are popup buttons that open the detail (#106)", async () => {
    const c = await withOpponentCreature();
    const { user } = mountGame(c);
    const own = within(screen.getByRole("group", { name: "Ton plateau" })).getAllByRole("button");
    expect(own.length).toBeGreaterThan(0);
    for (const t of own) expect(t.getAttribute("aria-haspopup")).toBe("dialog");
    await user.click(own[0]);
    expect(screen.getByRole("dialog", { name: c.getSnapshot().views.One!.you.board[0].name! })).toBeTruthy();
  });
});
