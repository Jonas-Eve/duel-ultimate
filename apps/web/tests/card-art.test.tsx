// The illustration slots (#113): the card's image when the art lookup knows its name, the landscape otherwise.
// The lookup is injected (`ArtProvider`), never the real data: this is about the slots, not the files.
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import type { ArtInfo, ArtLookup } from "../src/game/art";
import type { CardView } from "../src/engine/types";
import { ArtProvider } from "../src/ui/ArtContext";
import { CardDetailSheet } from "../src/ui/CardDetailSheet";
import { CardTile } from "../src/ui/CardTile";

const info: ArtInfo = { url: "/art/garde.svg", author: "Équipe PILOT", license: "CC0-1.0", source: "dessin original" };
const lookup: ArtLookup = (name) => (name === "Garde du rempart" ? info : undefined);

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

const compact = (c: CardView, destroyed = false, l: ArtLookup = lookup) =>
  render(
    <ArtProvider lookup={l}>
      <CardTile card={c} emblem="battlements" destroyed={destroyed} />
    </ArtProvider>,
  );
const detail = (c: CardView, l: ArtLookup = lookup) =>
  render(
    <ArtProvider lookup={l}>
      <CardDetailSheet card={c} emblem="battlements" deckName="Rempart" onClose={() => undefined} />
    </ArtProvider>,
  );

describe.each([
  ["the compact card", ".card-art", (c: CardView, l?: ArtLookup) => compact(c, false, l)],
  ["a destroyed card (discard)", ".card-art", (c: CardView, l?: ArtLookup) => compact(c, true, l)],
  ["the detail sheet", ".card-detail-art", (c: CardView, l?: ArtLookup) => detail(c, l)],
] as const)("%s", (_label, slot, mount) => {
  test("with art: one decorative image, no title, and no placeholder drawing", () => {
    const { baseElement } = mount(card());
    const art = baseElement.querySelector(slot)!;
    const imgs = art.querySelectorAll("img");
    expect(imgs).toHaveLength(1);
    expect(imgs[0].getAttribute("src")).toBe(info.url);
    expect(imgs[0].getAttribute("alt")).toBe("");
    expect(imgs[0].hasAttribute("title")).toBe(false);
    expect(imgs[0].getAttribute("draggable")).toBe("false");
    expect(art.querySelectorAll("svg:not(.card-emblem)")).toHaveLength(0);
  });

  test("without art (unknown name): the landscape, no image", () => {
    const { baseElement } = mount(card({ name: "Inconnue" }));
    const art = baseElement.querySelector(slot)!;
    expect(art.querySelectorAll("img")).toHaveLength(0);
    expect(art.querySelectorAll("svg:not(.card-emblem)")).toHaveLength(1);
  });

  test("a null name gives the landscape (the lookup is not even asked)", () => {
    let asked = 0;
    const { baseElement } = mount(card({ name: null }), () => {
      asked += 1;
      return info;
    });
    const art = baseElement.querySelector(slot)!;
    expect(art.querySelectorAll("img")).toHaveLength(0);
    expect(art.querySelectorAll("svg:not(.card-emblem)")).toHaveLength(1);
    expect(asked).toBe(0);
  });
});

describe("the compact art slot", () => {
  test("the image comes after the chip in the DOM (no z-index), and the slot holds no text outside the chip", () => {
    const { container } = compact(card());
    const art = container.querySelector(".card-art")!;
    expect([...art.children].map((n) => n.tagName.toLowerCase())).toEqual(["svg", "img"]);
    const walker = document.createTreeWalker(art, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n !== null; n = walker.nextNode()) {
      expect(n.parentElement!.closest(".card-emblem"), `text "${n.textContent}" outside the chip`).not.toBeNull();
    }
  });

  test("cost and values are outside the art slot", () => {
    const { container } = render(
      <ArtProvider lookup={lookup}>
        <CardTile card={card()} cost={2} emblem="battlements" />
      </ArtProvider>,
    );
    const art = container.querySelector(".card-art")!;
    expect(art.contains(container.querySelector(".card-cost"))).toBe(false);
    expect(art.contains(container.querySelector(".card-values"))).toBe(false);
  });
});

describe("the detail art slot", () => {
  test("holds nothing but the image", () => {
    const { baseElement } = detail(card());
    const art = baseElement.querySelector(".card-detail-art")!;
    expect([...art.children].map((n) => n.tagName.toLowerCase())).toEqual(["img"]);
    expect(art.textContent).toBe("");
  });
});
