import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import type { CardView } from "../src/engine/types";
import { CardTile } from "../src/ui/CardTile";
import { expectedCardLabel } from "./support/card-label";

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
        pressed={false}
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
