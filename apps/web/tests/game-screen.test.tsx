import { act, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import type { ActionPreview, CardView } from "../src/engine/types";
import { isActivate, isPlay } from "../src/engine/types";
import type { Snapshot } from "../src/game/controller";
import { messageFor } from "../src/ui/errors.fr";
import { decorateSession, newSession } from "./support/engine";
import { faultyFactory } from "./support/faulty";
import { playHuman, playUntil, randomPreview, settle, stallPolicy, stallPreview } from "./support/policies";
import { inAct, makeController, mountGame, useRealEngine } from "./support/ui";

useRealEngine();

const label = (c: CardView, extra = ""): string =>
  `${c.name}, coût ${c.cost}, force ${c.force}, coût d'attaque ${c.attack_cost}, PV ${c.health}${extra}` +
  (c.provocation ? ", provocation" : "") +
  (c.riposte > 0 ? `, riposte ${c.riposte}` : "");

const hand = () => screen.getByRole("group", { name: "Ta main" });
const handButtons = () => within(hand()).getAllByRole("button") as HTMLButtonElement[];
const board = (mine: boolean) => screen.getByRole("group", { name: mine ? "Ton plateau" : "Plateau adverse" });
const controls = () => [...screen.queryAllByRole("button")] as HTMLButtonElement[];

/** First seed (human `rush` opens) whose opening hand holds a summon that keeps the main. */
function seedWithQuietSummon(): number {
  for (let seed = 0; seed < 40; seed++) {
    const s = newSession("rush", "bulwark", seed);
    try {
      if (s.legalActionsWithTime().some((p) => isPlay(p.action) && !p.hands_over && p.ends_game === null)) return seed;
    } finally {
      s.dispose();
    }
  }
  throw new Error("no seed found");
}

describe("the permanent information", () => {
  test("heroes, health, counters, time band, named hand", () => {
    const c = makeController("rush", "bulwark", 3);
    const twin = newSession("rush", "bulwark", 3);
    mountGame(c);
    const v = twin.view("One");

    const me = screen.getByRole("group", { name: "Ton héros" });
    expect(within(me).getByText(v.you.hero.name)).toBeTruthy();
    expect(within(me).getByText(`PV ${v.you.hero.health}`)).toBeTruthy();
    expect(within(me).getByText(`Main : ${v.you.hand.length}`)).toBeTruthy();
    expect(within(me).getByText(`Pioche : ${v.you.deck_count}`)).toBeTruthy();

    const opp = screen.getByRole("group", { name: "Héros adverse" });
    expect(within(opp).getByText(v.opponent.hero.name)).toBeTruthy();
    expect(within(opp).getByText(`PV ${v.opponent.hero.health}`)).toBeTruthy();
    expect(within(opp).getByText(`Main : ${v.opponent.hand_count}`)).toBeTruthy();
    expect(within(opp).getByText(`Pioche : ${v.opponent.deck_count}`)).toBeTruthy();

    const band = screen.getByRole("group", { name: "Temps et jeton" });
    expect(v.active).toBe("One");
    expect(within(band).getByText("À toi")).toBeTruthy();
    expect(within(band).getByText(`Temps : ${v.budget}`)).toBeTruthy();
    expect(within(band).getByText(`max ${v.you.hero.max_time}`)).toBeTruthy();

    expect(handButtons().map((b) => b.getAttribute("aria-label")?.split(",")[0])).toEqual(v.you.hand.map((x) => x.name));
    expect(screen.getByText("Graine : 3")).toBeTruthy();
    twin.dispose();
  });

  test("the card accessible name is complete, board cards show force, attack cost, health", async () => {
    const c = makeController("rush", "bulwark", 3);
    c.start();
    await playUntil(c, randomPreview(5), (s) => (s.views.One?.you.board.length ?? 0) >= 1 && (s.views.One?.opponent.board.length ?? 0) >= 1, 40);
    mountGame(c);
    const v = c.getSnapshot().views.One!;
    for (const [mine, cards] of [
      [true, v.you.board],
      [false, v.opponent.board],
    ] as const) {
      const names = within(board(mine))
        .getAllByRole("group")
        .map((g) => g.getAttribute("aria-label"));
      expect(names).toEqual(cards.map((x) => label(x)));
      for (const g of within(board(mine)).getAllByRole("group")) {
        const text = g.textContent ?? "";
        expect(text).toMatch(/Force \d+/);
        expect(text).toMatch(/Attaque \d+/);
        expect(text).toMatch(/PV \d+/);
      }
    }
  });

  test("provocation and riposte carry a visible text label, never colour alone", async () => {
    // The human only summons (never attacks), so the game lasts; bounded search over seeds.
    const summoner = (s: Snapshot): ActionPreview =>
      s.legal.find((p) => isPlay(p.action)) ?? (s.legal.find((p) => p.action !== "concede") as ActionPreview);
    const wanted = (s: Snapshot) => {
      const v = s.views.One!;
      return [...v.you.board, ...v.you.hand].some((x) => x.provocation) && v.opponent.board.some((x) => x.riposte >= 1);
    };
    let c = makeController("bulwark", "rush", 0);
    let found = false;
    for (let seed = 0; seed < 30 && !found; seed++) {
      c = makeController("bulwark", "rush", seed);
      c.start();
      try {
        await playUntil(c, summoner, wanted, 60);
        found = true;
      } catch {
        c.dispose();
      }
    }
    expect(found).toBe(true);
    mountGame(c);
    const v = c.getSnapshot().views.One!;
    const shown = [...v.you.hand, ...v.you.board, ...v.opponent.board];
    expect(screen.getAllByText("Provocation")).toHaveLength(shown.filter((x) => x.provocation).length);
    expect(screen.getAllByText("Riposte 1").length).toBe(shown.filter((x) => x.riposte === 1).length);
    expect(screen.getAllByText("Riposte 1").length).toBeGreaterThan(0);
    for (const svg of document.querySelectorAll("svg")) expect(svg.getAttribute("aria-hidden")).toBe("true");
  });
});

describe("the opponent's hand never reaches the DOM", () => {
  test("human bulwark against rush, after several turns", async () => {
    const RUSH_ONLY = ["Escarmoucheur", "Pillard", "Loup hérissé", "Lancier", "Cavalier"];
    const c = makeController("bulwark", "rush", 5);
    const { container } = mountGame(c);
    const choose = randomPreview(1);
    for (let round = 0; round < 6; round++) {
      await inAct(async () => {
        const s = await settle(c);
        if (s.phase === "human-turn") c.submit(choose(s));
      });
    }
    await waitFor(() => expect(c.getSnapshot().phase).not.toBe("computer-turn"));
    const v = c.getSnapshot().views.One!;
    const onItsBoard = new Set(v.opponent.board.map((x) => x.name));
    const labels = [...container.querySelectorAll("[aria-label]")].map((e) => e.getAttribute("aria-label") ?? "").join("\n");
    const text = container.textContent ?? "";
    let hidden = 0;
    for (const name of RUSH_ONLY) {
      if (onItsBoard.has(name)) continue;
      hidden += 1;
      expect(text, `${name} leaked in the text`).not.toContain(name);
      expect(labels, `${name} leaked in an aria-label`).not.toContain(name);
    }
    expect(hidden).toBeGreaterThan(0);
    expect(screen.queryByRole("group", { name: /main adverse/i })).toBeNull();
  });
});

describe("cost and hand-over come from the engine preview", () => {
  test("every hand card and hero effect shows the cost and the hand-over of the twin session's preview", () => {
    const seed = 3;
    const c = makeController("rush", "bulwark", seed);
    const twin = newSession("rush", "bulwark", seed);
    mountGame(c);
    const previews = twin.legalActionsWithTime();
    const v = twin.view("One");

    const buttons = handButtons();
    expect(buttons).toHaveLength(v.you.hand.length);
    v.you.hand.forEach((card, i) => {
      const p = previews.find((x) => isPlay(x.action) && x.action.play.card === card.id) as ActionPreview;
      const text = buttons[i].textContent ?? "";
      expect(text, card.name ?? "").toContain(`Coût ${p.cost}`);
      expect(/Passe la main/.test(text), `${card.name} hand-over`).toBe(p.hands_over);
      expect(/passe la main/.test(buttons[i].getAttribute("aria-label") ?? "")).toBe(p.hands_over);
      if (!p.hands_over) expect(text).toContain(`Il te restera ${p.budget_after} de temps`);
      else expect(text).toContain(`l'adversaire aura ${p.budget_after} de temps`);
    });

    const actions = screen.getByRole("group", { name: "Actions" });
    v.you.hero.effects.forEach((e, i) => {
      const p = previews.find((x) => isActivate(x.action) && x.action.activate.effect === i) as ActionPreview;
      const b = within(actions).getByRole("button", { name: new RegExp(`^${e.name}`) });
      expect(b.textContent).toContain(`Coût ${p.cost}`);
      expect(/Passe la main/.test(b.textContent ?? "")).toBe(p.hands_over);
    });
    twin.dispose();
  });
});

describe("summon and draw", () => {
  test("pressing a card: it leaves the hand, lands on the board, the time is the preview's", async () => {
    const seed = seedWithQuietSummon();
    const c = makeController("rush", "bulwark", seed);
    const { user } = mountGame(c);
    const snap = c.getSnapshot();
    const v = snap.views.One!;
    const index = v.you.hand.findIndex((card) => {
      const p = snap.legal.find((x) => isPlay(x.action) && x.action.play.card === card.id) as ActionPreview;
      return !p.hands_over && p.ends_game === null;
    });
    const card = v.you.hand[index];
    const p = snap.legal.find((x) => isPlay(x.action) && x.action.play.card === card.id) as ActionPreview;

    await user.click(handButtons()[index]);

    expect(handButtons()).toHaveLength(v.you.hand.length - 1);
    const onBoard = within(board(true)).getAllByRole("group");
    expect(onBoard.map((g) => g.getAttribute("aria-label"))).toContain(label(card));
    const band = screen.getByRole("group", { name: "Temps et jeton" });
    expect(within(band).getByText(`Temps : ${p.budget_after}`)).toBeTruthy();
    expect(within(band).getByText("À toi")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  test("the hero's draw effect: the hand grows, the deck shrinks, the cost seen is the cost paid", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    const snap = c.getSnapshot();
    const v = snap.views.One!;
    const p = snap.legal.find((x) => isActivate(x.action) && x.action.activate.effect === 0) as ActionPreview;
    expect(p.hands_over).toBe(false);
    const actions = screen.getByRole("group", { name: "Actions" });
    const button = within(actions).getByRole("button", { name: new RegExp(`^${v.you.hero.effects[0].name}`) });
    expect(button.textContent).toContain(`Coût ${p.cost}`);

    await user.click(button);

    const me = screen.getByRole("group", { name: "Ton héros" });
    expect(within(me).getByText(`Main : ${v.you.hand.length + 1}`)).toBeTruthy();
    expect(within(me).getByText(`Pioche : ${v.you.deck_count - 1}`)).toBeTruthy();
    expect(handButtons()).toHaveLength(v.you.hand.length + 1);
    expect(within(screen.getByRole("group", { name: "Temps et jeton" })).getByText(`Temps : ${p.budget_after}`)).toBeTruthy();
    const after = c.getSnapshot();
    expect(after.records.at(-1)?.events.some((e) => "effect_activated" in e && e.effect_activated.cost === p.cost)).toBe(true);
  });

  test("a double press plays one single summon and shows no error", async () => {
    const seed = 3;
    const { session, probe } = decorateSession(newSession("rush", "bulwark", seed));
    const c = makeController("rush", "bulwark", seed, { createSession: () => session });
    const { user } = mountGame(c);
    const first = handButtons()[0];
    const id = c.getSnapshot().views.One!.you.hand[0].id;
    await user.dblClick(first);
    await waitFor(() => expect(c.getSnapshot().phase).not.toBe("computer-turn"));
    expect(probe.applied.filter((a) => isPlay(a as never) && (a as { play: { card: number } }).play.card === id)).toHaveLength(1);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

describe("conceding needs a confirmation", () => {
  test("without confirmation nothing happens; Continue closes; Concede ends the game", async () => {
    const { session, probe } = decorateSession(newSession("rush", "bulwark", 3));
    const c = makeController("rush", "bulwark", 3, { createSession: () => session });
    const { user } = mountGame(c);
    const concede = screen.getByRole("button", { name: "Abandonner" });

    await user.click(concede);
    const dialog = screen.getByRole("dialog", { name: "Abandonner la partie ?" });
    expect(probe.calls.apply).toBe(0);
    expect(c.getSnapshot().phase).toBe("human-turn");

    await user.click(within(dialog).getByRole("button", { name: "Continuer" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(probe.calls.apply).toBe(0);
    expect(document.activeElement).toBe(concede);

    await user.click(screen.getByRole("button", { name: "Abandonner" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Abandonner" }));
    expect(c.getSnapshot().phase).toBe("over");
    expect(probe.applied).toEqual(["concede"]);
    expect(screen.getByRole("heading", { name: "L'ordinateur a gagné." })).toBeTruthy();
  });

  test("Escape in the confirmation dialog does not concede", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    await user.click(screen.getByRole("button", { name: "Abandonner" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(c.getSnapshot().phase).toBe("human-turn");
  });
});

describe("the computer's turn", () => {
  test("the computer opens: indicator at once, every control disabled, then one single announcement", async () => {
    const gates: (() => void)[] = [];
    const gate = { yield: () => new Promise<void>((resolve) => gates.push(resolve)) };
    const c = makeController("bulwark", "rush", 3, { scheduler: gate });
    mountGame(c);

    expect(c.getSnapshot().phase).toBe("computer-turn");
    const status = screen.getByRole("status");
    expect(status.textContent).toBe("L'ordinateur joue…");
    expect(controls().length).toBeGreaterThan(0);
    for (const b of controls()) expect(b.disabled, b.textContent ?? "").toBe(true);
    expect(screen.queryByText(/L'ordinateur a joué/)).toBeNull();

    await act(async () => {
      gates.shift()?.();
      await Promise.resolve();
      for (let i = 0; i < 50 && c.getSnapshot().phase === "computer-turn"; i++) {
        if (gates.length) gates.shift()?.();
        await Promise.resolve();
      }
    });
    await waitFor(() => expect(c.getSnapshot().phase).toBe("human-turn"));

    expect(screen.queryByRole("status")).toBeNull();
    const n = c.getSnapshot().computerSteps;
    expect(n).toBeGreaterThan(0);
    const said = screen.getAllByText(`L'ordinateur a joué ${n} action${n > 1 ? "s" : ""}. À toi de jouer.`);
    expect(said).toHaveLength(1);
    expect(said[0].closest("[aria-live]")?.getAttribute("aria-live")).toBe("polite");
    expect(handButtons().some((b) => !b.disabled)).toBe(true);
  });

  test("one single announcement per chain even when every action publishes a snapshot", async () => {
    let t = 0;
    const c = makeController("bulwark", "rush", 4, { now: () => (t += 50) }); // every action overruns the slice
    const { container } = mountGame(c);
    const live = container.querySelector("[aria-live='polite']") as HTMLElement;
    const seen: string[] = [];
    const publications: number[] = [];
    c.subscribe(() => publications.push(c.getSnapshot().computerSteps));
    new MutationObserver(() => seen.push(live.textContent ?? "")).observe(live, { childList: true, characterData: true, subtree: true });
    await waitFor(() => expect(c.getSnapshot().phase).toBe("human-turn"));
    await Promise.resolve();
    const n = c.getSnapshot().computerSteps;
    expect(n).toBeGreaterThan(1);
    expect(publications).toEqual(Array.from({ length: n }, (_, i) => i + 1)); // one snapshot per action, each one a chance to announce
    expect(seen.filter((x) => x !== "")).toEqual([`L'ordinateur a joué ${n} actions. À toi de jouer.`]);
  });

  test("a later human action that keeps the main removes the old announcement (computerSteps is stale)", async () => {
    // Seed 4: the computer opens, and the human then has an action that keeps the main.
    const c = makeController("bulwark", "rush", 4);
    const { container } = mountGame(c);
    await waitFor(() => expect(c.getSnapshot().phase).toBe("human-turn"));
    const live = container.querySelector("[aria-live='polite']") as HTMLElement;
    expect(live.textContent).toMatch(/L'ordinateur a joué \d+ actions?\. À toi de jouer\./);

    const snap = c.getSnapshot();
    const quiet = snap.legal.findIndex((p) => !p.hands_over && p.ends_game === null && p.action !== "concede");
    expect(quiet).toBeGreaterThanOrEqual(0);
    const seen: string[] = [];
    new MutationObserver(() => seen.push(live.textContent ?? "")).observe(live, { childList: true, characterData: true, subtree: true });
    await act(async () => {
      c.submit(snap.legal[quiet]);
    });
    await Promise.resolve();
    expect(c.getSnapshot().phase).toBe("human-turn");
    expect(c.getSnapshot().computerSteps).toBe(snap.computerSteps); // still the stale value
    expect(live.textContent).toBe("");
    expect(seen.filter((x) => x !== "")).toEqual([]);
  });

  test("a human action that hands the main over clears the old announcement, the next chain announces its own", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { container } = mountGame(c);
    const snap = c.getSnapshot();
    const handOver = snap.legal.findIndex((p) => p.hands_over && p.ends_game === null);
    expect(handOver).toBeGreaterThanOrEqual(0);
    expect(container.querySelector("[aria-live='polite']")?.textContent ?? "").toBe("");
    await act(async () => {
      c.submit(snap.legal[handOver]);
    });
    await waitFor(() => expect(c.getSnapshot().phase).toBe("human-turn"));
    const n = c.getSnapshot().computerSteps;
    expect(screen.getAllByText(`L'ordinateur a joué ${n} action${n > 1 ? "s" : ""}. À toi de jouer.`)).toHaveLength(1);
  });

  test("a chain that ends the game announces nothing: the end screen takes over", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { container } = mountGame(c);
    await inAct(() => playHuman(c, randomPreview(4)));
    expect(c.getSnapshot().phase).toBe("over");
    expect(container.textContent).not.toMatch(/À toi de jouer\./);
  });
});

describe("the three ends of the game", () => {
  /**
   * A finished game, by a bounded seed search. Against a random computer the human `rush` always
   * wins and the human `bulwark` almost always loses (60 seeds each), hence the two pairings.
   */
  async function finishedGame(winner: "One" | "Two") {
    const [one, two] = winner === "One" ? ["rush", "bulwark"] : ["bulwark", "rush"];
    for (let seed = 0; seed < 60; seed++) {
      const c = makeController(one, two, seed);
      c.start();
      const { snapshot } = await playHuman(c, randomPreview(seed));
      const r = snapshot.views.One?.result;
      if (r?.winner === winner && r.reason === "hero_down") return { c, seed };
      c.dispose();
    }
    throw new Error("no game found");
  }

  test.each([
    ["One", "Tu as gagné !"],
    ["Two", "L'ordinateur a gagné."],
  ] as const)("hero_down, winner %s: title, reason with the fallen hero's name, seeds", async (winner, title) => {
    const found = await finishedGame(winner);
    const view = found.c.getSnapshot().views.One!;
    const loser = winner === "One" ? view.opponent.hero.name : view.you.hero.name;
    mountGame(found.c);
    expect(screen.getByRole("heading", { name: title })).toBeTruthy();
    expect(screen.getByText(`Le héros ${loser} est tombé à 0 PV.`)).toBeTruthy();
    expect(screen.getByText(`Graine : ${found.seed}`)).toBeTruthy();
    expect(screen.getByText(`Graine de l'ordinateur : ${found.c.getSnapshot().aiSeed}`)).toBeTruthy();
    expect(screen.queryByText(/abandonné|épuisement/)).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("heading", { name: title }));
  });

  test("exhaustion (pinned case: rush against bulwark, seed 2, both sides stalling): the reason is not swapped", async () => {
    const c = makeController("rush", "bulwark", 2, { policy: stallPolicy });
    c.start();
    const { snapshot } = await playHuman(c, stallPreview);
    const result = snapshot.views.One!.result!;
    expect(result.reason).toBe("exhaustion");
    mountGame(c);
    const loserText = result.winner === "One" ? "L'ordinateur n'avait plus aucune action possible (épuisement)." : "Tu n'avais plus aucune action possible (épuisement).";
    expect(screen.getByText(loserText)).toBeTruthy();
    expect(screen.queryByText(/abandonné|tombé/)).toBeNull();
    expect(screen.getByRole("heading", { name: result.winner === "One" ? "Tu as gagné !" : "L'ordinateur a gagné." })).toBeTruthy();
    expect(screen.getByText("Graine : 2")).toBeTruthy();
  });

  test("concede: the human lost and says so", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user } = mountGame(c);
    await user.click(screen.getByRole("button", { name: "Abandonner" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Abandonner" }));
    expect(screen.getByRole("heading", { name: "L'ordinateur a gagné." })).toBeTruthy();
    expect(screen.getByText("Tu as abandonné.")).toBeTruthy();
    expect(screen.queryByText(/épuisement|tombé/)).toBeNull();
    expect(screen.getByText("Graine : 3")).toBeTruthy();
    expect(screen.getByText(`Graine de l'ordinateur : ${c.getSnapshot().aiSeed}`)).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("heading", { name: "L'ordinateur a gagné." }));
  });

  test("after the end, no game control is left and the three buttons call back", async () => {
    const c = makeController("rush", "bulwark", 3);
    const { user, onReplaySameSeed, onReplayNewSeed, onHome } = mountGame(c);
    await user.click(screen.getByRole("button", { name: "Abandonner" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Abandonner" }));
    expect(screen.queryByRole("group", { name: "Ta main" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Abandonner" })).toBeNull();
    expect(screen.queryByRole("group", { name: "Actions" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Rejouer avec la même graine" }));
    await user.click(screen.getByRole("button", { name: "Rejouer avec une nouvelle graine" }));
    await user.click(screen.getByRole("button", { name: "Retour à l'accueil" }));
    expect([onReplaySameSeed.mock.calls.length, onReplayNewSeed.mock.calls.length, onHome.mock.calls.length]).toEqual([1, 1, 1]);
  });
});

describe("errors", () => {
  test("a refused action: French alert (never the English text), the game intact, Close clears it", async () => {
    const phantom: CardView = { id: 999999, name: "Fantôme", cost: 1, force: 1, attack_cost: 1, health: 1, provocation: false, riposte: 0 };
    const injected: ActionPreview = { action: { play: { card: 999999 } }, cost: 1, hands_over: false, budget_after: 1, ends_game: null };
    const { session } = decorateSession(newSession("rush", "bulwark", 3), {
      injectPreview: injected,
      forceView: (seat, v) => (seat === "One" ? { ...v, you: { ...v.you, hand: [...v.you.hand, phantom] } } : v),
    });
    const c = makeController("rush", "bulwark", 3, { createSession: () => session });
    const { user } = mountGame(c);
    const before = handButtons().map((b) => b.getAttribute("aria-label"));
    const viewBefore = c.getSnapshot().views;

    await user.click(within(hand()).getByRole("button", { name: /^Fantôme/ }));

    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain(messageFor("card_not_in_hand"));
    expect(alert.textContent).not.toContain(c.getSnapshot().error!.message);
    expect(c.getSnapshot().phase).toBe("human-turn");
    expect(c.getSnapshot().views.One?.you.board).toEqual(viewBefore.One?.you.board);
    expect(handButtons().map((b) => b.getAttribute("aria-label"))).toEqual(before);

    await user.click(within(alert).getByRole("button", { name: "Fermer" }));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(c.getSnapshot().error).toBeNull();
  });

  test("a fatal error: the dedicated screen, no control of the game, the two exits", async () => {
    const { factory } = faultyFactory("apply");
    const c = makeController("rush", "bulwark", 3, {}, factory);
    const { user, onHome, onReplaySameSeed } = mountGame(c);
    await user.click(handButtons()[0]);
    const s: Snapshot = c.getSnapshot();
    expect(s.phase).toBe("fatal");
    expect(screen.getByText(messageFor(s.error!.code))).toBeTruthy();
    expect(screen.queryByRole("group", { name: "Ta main" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Abandonner" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Fermer" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Rejouer avec la même graine" }));
    await user.click(screen.getByRole("button", { name: "Retour à l'accueil" }));
    expect(onReplaySameSeed).toHaveBeenCalledTimes(1);
    expect(onHome).toHaveBeenCalledTimes(1);
  });
});
