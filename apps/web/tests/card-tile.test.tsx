import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import type { CardView } from "../src/engine/types";
import { CardTile } from "../src/ui/CardTile";
import { expectedCardLabel } from "./support/card-label";
import { cardView } from "./support/card-view";
import { newKindsViews } from "./support/new-kinds";

const card = cardView;

afterEach(cleanup);

describe("the compact card", () => {
  test("a plain tile is a group inside one fixed-size face; its three values are always there, riposte 0 included", () => {
    const c = card();
    const { container } = render(<CardTile card={c} />);
    const group = screen.getByRole("group", { name: expectedCardLabel(c) });
    expect(group.classList.contains("card")).toBe(true);
    expect(group.classList.contains("card-compact")).toBe(true);
    expect(group.querySelectorAll(".card-face")).toHaveLength(1);
    const values = [...container.querySelectorAll(".card-values .card-value")].map((v) => v.textContent);
    expect(values).toEqual(["1", "0", "6"]);
    expect(container.querySelector(".card-cost")).toBeNull();
  });

  test("the cost pill appears once, beside the name, only when a cost is given", () => {
    const c = card();
    const { container } = render(<CardTile card={c} cost={2} />);
    const pills = container.querySelectorAll(".card-cost");
    expect(pills).toHaveLength(1);
    expect(pills[0].textContent).toBe("2");
    expect(pills[0].parentElement).toBe(container.querySelector(".card-name")!.parentElement);
    expect(screen.getByRole("group").getAttribute("aria-label")).toBe(expectedCardLabel(c, { inHand: true, cost: 2 }));
  });

  test("one 'Provocation' tag per provocateur, under the card (outside the fixed face)", () => {
    const { container } = render(<CardTile card={card({ provocation: true })} />);
    expect(screen.getAllByText("Provocation")).toHaveLength(1);
    expect(container.querySelector(".card-face")!.contains(screen.getByText("Provocation"))).toBe(false);
    expect(container.querySelector(".card-tags")!.contains(screen.getByText("Provocation"))).toBe(true);
    expect(screen.getByRole("group").getAttribute("aria-label")).toContain(", provocation");
  });

  test("hand legend and mark are text in the tags and in the accessible name", () => {
    const c = card();
    render(
      <CardTile
        card={c}
        cost={2}
        timeLeft="Reste 3"
        onPress={() => undefined}
        mark="selected"
      />,
    );
    const button = screen.getByRole("button");
    expect(button.getAttribute("aria-label")).toBe(
      expectedCardLabel(c, { inHand: true, cost: 2, timeLeft: "Reste 3", mark: "sélectionnée" }),
    );
    expect(button.querySelector(".card-tags")!.textContent).toContain("Reste 3");
    expect(button.querySelector(".card-tags .card-mark")!.textContent).toBe("Sélectionnée");
    expect(button.classList.contains("card-selected")).toBe(true);
    // The selection is shown by the mark and the border, never as a toggle.
    expect(button.hasAttribute("aria-pressed")).toBe(false);
  });

  test("a hand-over reads 'Passe la main' under the card and in the name", () => {
    const c = card();
    render(<CardTile card={c} cost={4} handsOver onPress={() => undefined} />);
    const button = screen.getByRole("button");
    expect(button.getAttribute("aria-label")).toBe(expectedCardLabel(c, { inHand: true, cost: 4, handsOver: true }));
    expect(button.querySelector(".card-tags")!.textContent).toContain("Passe la main");
  });

  test("every svg is decorative", () => {
    const { container } = render(<CardTile card={card({ provocation: true })} cost={1} />);
    const svgs = container.querySelectorAll("svg");
    expect(svgs.length).toBeGreaterThan(3);
    for (const svg of svgs) expect(svg.getAttribute("aria-hidden")).toBe("true");
    expect(container.querySelector("img")).toBeNull();
  });
});

describe("the tile of an immobile unit and of a spell (#210)", () => {
  test("a unit: the label has all three values; an immobile unit has no 'attaque N' and an 'immobile' tag", () => {
    const c = card({ provocation: true });
    const { unmount } = render(<CardTile card={c} />);
    expect(screen.getByRole("group").getAttribute("aria-label")).toBe("Garde du rempart, attaque 1, riposte 0, vie 6, provocation");
    unmount();
    const m = card({ immobile: true, force: 0, riposte: 1, health: 4, effects: [], provocation: true });
    const { container } = render(<CardTile card={m} cost={2} timeLeft="Reste 1" />);
    const label = "Garde du rempart, coût 2, riposte 1, vie 4, provocation, immobile, Reste 1";
    expect(screen.getByRole("group").getAttribute("aria-label")).toBe(label);
    expect(expectedCardLabel(m, { inHand: true, cost: 2, timeLeft: "Reste 1" })).toBe(label);
    // The mark sits under the card with 'Provocation', as text.
    const keywords = [...container.querySelectorAll(".card-tags .card-keyword")].map((k) => k.textContent);
    expect(keywords).toEqual(["Provocation", "Immobile"]);
    expect(container.querySelector(".card-face")!.textContent).not.toContain("Immobile");
  });

  test("an immobile tile shows riposte and life, and an empty attack slot", () => {
    const { container } = render(<CardTile card={card({ immobile: true, force: 0, riposte: 1, health: 4, effects: [] })} />);
    const slots = [...container.querySelectorAll(".card-values .card-value")];
    expect(slots).toHaveLength(3);
    expect(slots.map((v) => v.textContent)).toEqual(["", "1", "4"]);
    expect(slots[0].classList.contains("card-value-empty")).toBe(true);
    expect(slots[0].querySelector("svg")).toBeNull();
  });

  test("a spell: 'sort' in the name and as the visible caption, no attack, riposte or life", async () => {
    const { spell } = await newKindsViews();
    const { container } = render(<CardTile card={spell} cost={1} timeLeft="Reste 2" />);
    const label = "zz-spell, coût 1, sort, Reste 2";
    expect(screen.getByRole("group").getAttribute("aria-label")).toBe(label);
    expect(expectedCardLabel(spell, { inHand: true, cost: 1, timeLeft: "Reste 2" })).toBe(label);
    expect(container.querySelectorAll(".card-values .card-value")).toHaveLength(0);
    expect(container.querySelector(".card-face .card-kind")!.textContent).toBe("Sort");
  });

  test("a spell that hands over the main keeps its name literal", async () => {
    const { spell } = await newKindsViews();
    render(<CardTile card={spell} cost={3} handsOver />);
    expect(screen.getByRole("group").getAttribute("aria-label")).toBe("zz-spell, coût 3, sort, passe la main");
  });

  test("an immobile or spell tile keeps the compact size: one fixed face, no inline style", async () => {
    const { spell, immobile } = await newKindsViews();
    for (const c of [card(), immobile, spell]) {
      const { container, unmount } = render(<CardTile card={c} cost={1} />);
      expect(container.querySelectorAll(".card-face")).toHaveLength(1);
      expect(container.querySelector("[style]")).toBeNull();
      expect(container.querySelector(".card-compact")!.querySelector(":scope > .card-face")).not.toBeNull();
      unmount();
    }
  });
});
