import { act, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import type { Snapshot } from "../src/game/controller";
import { formatBlocks, formatRecords, logContext } from "../src/ui/log-format";
import { playHuman, playUntil, randomPreview, settle, stallPolicy, stallPreview } from "./support/policies";
import { makeController, mountGame, useRealEngine } from "./support/ui";

useRealEngine();

const bar = () => screen.getByRole("log");
const journalButton = () => screen.getByRole("button", { name: "Journal" });
const lastLine = (c: { getSnapshot(): Snapshot }) => {
  const s = c.getSnapshot();
  const lines = formatRecords(s.records, logContext(s));
  return lines[lines.length - 1].text;
};

describe("the last line of the log", () => {
  test("is always visible, a polite log region at a human turn", () => {
    const c = makeController("rush", "bulwark", 3);
    mountGame(c);
    expect(bar().getAttribute("aria-live")).toBe("polite");
    expect(bar().getAttribute("aria-atomic")).toBe("false");
    expect(bar().textContent).toBe(lastLine(c));
    expect(bar().textContent).toContain("La partie commence");
  });

  test("follows the game: after a human action it shows that action's last line", async () => {
    const c = makeController("rush", "bulwark", 3);
    mountGame(c);
    await act(async () => {
      c.submit(c.getSnapshot().legal.find((p) => p.action !== "concede")!);
      await settle(c);
    });
    expect(bar().textContent).toBe(lastLine(c));
    expect(bar().textContent).not.toContain("La partie commence");
  });

  test("stays visible while the drawer is open (never hidden by it)", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    await user.click(journalButton());
    expect(screen.getByRole("dialog", { name: "Journal" })).toBeTruthy();
    expect(bar().textContent).toBe(lastLine(c));
    expect(bar().hidden).toBe(false);
    expect(bar().closest("[aria-hidden='true']")).toBeNull();
  });

  test("is not announced during the computer's chain; the single announcement is the summary", async () => {
    const gates: (() => void)[] = [];
    const gate = { yield: () => new Promise<void>((resolve) => gates.push(resolve)) };
    const c = makeController("bulwark", "rush", 3, { scheduler: gate });
    const { container } = mountGame(c);
    expect(c.getSnapshot().phase).toBe("computer-turn");
    expect(bar().getAttribute("aria-live")).toBe("off");

    // Every change of the log bar must happen while it is not live.
    const liveChanges: string[] = [];
    new MutationObserver(() => {
      if (bar().getAttribute("aria-live") !== "off") liveChanges.push(bar().textContent ?? "");
    }).observe(bar(), { childList: true, characterData: true, subtree: true });
    const announce = container.querySelector(".announce") as HTMLElement;
    const summaries: string[] = [];
    new MutationObserver(() => summaries.push(announce.textContent ?? "")).observe(announce, { childList: true, characterData: true, subtree: true });

    await act(async () => {
      gates.shift()?.();
      await Promise.resolve();
      for (let i = 0; i < 50 && c.getSnapshot().phase === "computer-turn"; i++) {
        if (gates.length) gates.shift()?.();
        await Promise.resolve();
      }
    });
    await waitFor(() => expect(c.getSnapshot().phase).toBe("human-turn"));
    await Promise.resolve();

    expect(bar().textContent).toBe(lastLine(c));
    expect(liveChanges).toEqual([]);
    expect(summaries.filter((x) => x !== "")).toHaveLength(1);
    // While the summary is the announcement, the bar stays quiet; the next human action makes it live again.
    expect(bar().getAttribute("aria-live")).toBe("off");
    act(() => {
      c.submit(c.getSnapshot().legal.find((p) => p.action !== "concede")!);
    });
    expect(bar().getAttribute("aria-live")).toBe(c.getSnapshot().phase === "human-turn" ? "polite" : "off");
  });
});

describe("the log drawer", () => {
  test("opens from a 44px-class button, traps focus, closes with Escape and gives the focus back", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    const opener = journalButton();
    expect(opener.className).toContain("btn");
    await user.click(opener);
    const dialog = screen.getByRole("dialog", { name: "Journal" });
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.contains(document.activeElement)).toBe(true);
    await user.tab();
    await user.tab();
    expect(dialog.contains(document.activeElement)).toBe(true);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  test("closes with its own button too", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    await user.click(journalButton());
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Fermer" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  test("holds the whole history, titled by blocks, and is NOT a live region", async () => {
    const c = makeController("bulwark", "rush", 3); // the computer opens: its block is there
    const { user } = mountGame(c);
    await act(async () => {
      await settle(c);
    });
    await user.click(journalButton());
    const dialog = screen.getByRole("dialog", { name: "Journal" });
    const s = c.getSnapshot();
    const blocks = formatBlocks(s.records, logContext(s));
    expect(within(dialog).getAllByRole("listitem")).toHaveLength(blocks.flatMap((b) => b.lines).length);
    expect(within(dialog).getAllByRole("heading", { level: 3 }).map((h) => h.textContent)).toEqual(blocks.map((b) => b.title));
    expect(blocks.map((b) => b.title)).toContain("Tour de l'ordinateur");
    expect(dialog.querySelector("[aria-live]")).toBeNull();
    expect(dialog.querySelector("[role='log'], [role='status'], [role='alert']")).toBeNull();
    expect(dialog.closest("[aria-live]")).toBeNull();
  });
});

/** A bounded search for a game where each side has a card in its discard. */
async function withDiscards() {
  for (const [one, two] of [
    ["rush", "bulwark"],
    ["bulwark", "rush"],
  ] as const) {
    for (let seed = 0; seed < 30; seed++) {
      const c = makeController(one, two, seed);
      c.start();
      try {
        await playUntil(
          c,
          randomPreview(seed),
          (s) => (s.views.One?.you.discard.length ?? 0) >= 1 && (s.views.One?.opponent.discard.length ?? 0) >= 2,
          80,
        );
        return c;
      } catch {
        c.dispose();
      }
    }
  }
  throw new Error("no game with a discard on each side: widen the search");
}

describe("the discards (public, both camps)", () => {
  test("a button per hero opens the dead cards in death order, 'Détruite', never 'PV 0'", async () => {
    const c = await withDiscards();
    const { user } = mountGame(c);
    const v = c.getSnapshot().views.One!;
    const mine = screen.getByRole("group", { name: "Ton héros" });
    const theirs = screen.getByRole("group", { name: "Héros adverse" });

    for (const [group, discard] of [
      [mine, v.you.discard],
      [theirs, v.opponent.discard],
    ] as const) {
      const button = within(group).getByRole("button", { name: `Défausse (${discard.length})` });
      await user.click(button);
      const dialog = screen.getByRole("dialog");
      const items = within(dialog).getAllByRole("listitem");
      expect(items.map((li) => li.querySelector(".card-name")?.textContent)).toEqual(discard.map((x) => x.name));
      for (const [i, li] of items.entries()) {
        const card = discard[i];
        const tile = within(li).getByRole("group");
        // The name carries attack and riposte (even 0), "détruite" instead of a life, provocation last.
        expect(tile.getAttribute("aria-label")).toBe(
          `${card.name}, attaque ${card.force}, riposte ${card.riposte}, détruite${card.provocation ? ", provocation" : ""}`,
        );
        const text = li.textContent ?? "";
        expect(text).toContain("Détruite");
        expect(text).not.toMatch(/PV/);
        expect(text).not.toMatch(/Coût|Force|Attaque/);
        expect(text.includes("Provocation")).toBe(card.provocation);
        expect(li.querySelectorAll(".card-face").length).toBe(1);
      }
      expect(dialog.textContent).not.toMatch(/PV 0/);
      await user.keyboard("{Escape}");
      expect(document.activeElement).toBe(button);
    }
  });

  test("an empty discard says so", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    await user.click(within(screen.getByRole("group", { name: "Héros adverse" })).getByRole("button", { name: "Défausse (0)" }));
    expect(within(screen.getByRole("dialog")).getByText("Aucune carte détruite")).toBeTruthy();
  });
});

describe("the end screen shows the whole log", () => {
  const fullLog = () => screen.getByRole("region", { name: "Journal complet" });

  function expectComplete(c: { getSnapshot(): Snapshot }) {
    const s = c.getSnapshot();
    const lines = formatRecords(s.records, logContext(s));
    const region = fullLog();
    expect(within(region).getAllByRole("listitem")).toHaveLength(lines.length);
    expect(lines.length).toBeGreaterThan(5);
    expect(region.querySelector("[aria-live], [role='log']")).toBeNull();
    expect(region.closest("[aria-live]")).toBeNull();
    expect(within(region).getByText(lines[lines.length - 1].text)).toBeTruthy();
    return lines;
  }

  test("hero_down: same number of lines as formatRecords, the last one says who won and why", async () => {
    let c = makeController("rush", "bulwark", 0);
    for (let seed = 0; seed < 60; seed++) {
      c = makeController("rush", "bulwark", seed);
      c.start();
      const { snapshot } = await playHuman(c, randomPreview(seed));
      if (snapshot.views.One?.result?.reason === "hero_down") break;
    }
    mountGame(c);
    const lines = expectComplete(c);
    expect(lines[lines.length - 1].text).toMatch(/^(Tu gagnes|L'ordinateur gagne) : Le héros .+ est tombé à 0 PV\.$/);
  });

  // 220 actions rendered through jsdom: ~3.5 s alone in the full suite, 7+ s under CPU contention;
  // the default 5 s timeout flaked in CI (#77).
  test("exhaustion (pinned: rush vs bulwark, seed 2, both stalling)", async () => {
    const c = makeController("rush", "bulwark", 2, { chooseAction: stallPolicy });
    c.start();
    await playHuman(c, stallPreview);
    mountGame(c);
    const lines = expectComplete(c);
    expect(lines[lines.length - 1].text).toMatch(/ gagnes? : .* n'avai(s|t) plus aucune action possible \(épuisement\)\.$/);
    expect(lines.length).toBeGreaterThan(200); // a log cut at N lines would fail
  }, 30_000);

  test("concede", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    for (let i = 0; i < 4; i++) {
      await act(async () => {
        const s = await settle(c);
        const p = s.legal.find((x) => x.action !== "concede");
        if (s.phase === "human-turn" && p) c.submit(p);
      });
    }
    await waitFor(() => expect(c.getSnapshot().phase).toBe("human-turn"));
    await user.click(screen.getByRole("button", { name: "Abandonner" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Abandonner" }));
    const lines = expectComplete(c);
    expect(lines[lines.length - 1].text).toBe("L'ordinateur gagne : Tu as abandonné.");
  });
});
