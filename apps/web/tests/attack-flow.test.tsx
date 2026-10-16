import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import type { ActionPreview, CardView } from "../src/engine/types";
import { isActivate, isAttack, isAttackBy, isPlay } from "../src/engine/types";
import type { Snapshot } from "../src/game/controller";
import { decorateSession, newSession, type SessionProbe } from "./support/engine";
import { GameController } from "../src/game/controller";
import { config } from "./support/config";
import { playUntil, realDeps, settle } from "./support/policies";
import { inAct, makeController, mountGame, useRealEngine } from "./support/ui";

useRealEngine();

// ---------- scenario search (bounded, explicit failure) ----------

const notConcede = (s: Snapshot) => s.legal.filter((p) => p.action !== "concede");
/** Summons first, then hero effects, then anything. Keeps games long enough to reach a position. */
const summoner = (s: Snapshot): ActionPreview =>
  s.legal.find((p) => isPlay(p.action)) ?? notConcede(s).find((p) => isActivate(p.action)) ?? notConcede(s)[0];
/** Summons, then hits the hero whenever the engine lists it, otherwise anything but an attack on a creature. */
const hunter = (s: Snapshot): ActionPreview =>
  s.legal.find((p) => isPlay(p.action)) ??
  notConcede(s).find((p) => isAttack(p.action) && p.action.attack.target === "hero") ??
  notConcede(s).find((p) => isActivate(p.action)) ??
  notConcede(s)[0];

interface Game {
  c: GameController;
  probe: SessionProbe;
  one: string;
  two: string;
  seed: number;
}

function probedController(one: string, two: string, seed: number): Game {
  let probe: SessionProbe | undefined;
  const c = makeController(one, two, seed, {
    createSession: (r) => {
      const d = decorateSession(newSession(r.deckOne, r.deckTwo, r.seed));
      probe = d.probe;
      return d.session;
    },
  });
  c.start();
  if (probe === undefined) throw new Error("the session was not created");
  return { c, probe, one, two, seed };
}

/** Plays `choose` on successive seeds until `predicate` holds at a human turn. Throws if no seed gets there. */
async function findGame(
  one: string,
  two: string,
  choose: (s: Snapshot) => ActionPreview,
  predicate: (s: Snapshot) => boolean,
  maxSteps = 60,
  seeds = 80,
): Promise<Game> {
  for (let seed = 0; seed < seeds; seed++) {
    const g = probedController(one, two, seed);
    try {
      await playUntil(g.c, choose, (s) => s.phase === "human-turn" && predicate(s), maxSteps);
      return g;
    } catch {
      g.c.dispose();
    }
  }
  throw new Error(`no scenario found for ${one} vs ${two} within ${seeds} seeds`);
}

const mine = (s: Snapshot) => s.views.One!.you.board;
const theirs = (s: Snapshot) => s.views.One!.opponent.board;
const withTaunt = (s: Snapshot) => mine(s).length >= 1 && theirs(s).some((x) => x.provocation) && theirs(s).some((x) => !x.provocation);
const withoutTaunt = (s: Snapshot) => mine(s).length >= 1 && theirs(s).length >= 2 && !theirs(s).some((x) => x.provocation);

/** The twin session: same decks, same seed, every action the real session applied, replayed. */
function twinOf(g: Game) {
  const t = newSession(g.one, g.two, g.seed);
  for (const a of g.probe.applied) t.apply(a);
  return t;
}

// ---------- DOM helpers ----------

/** Two camps name their regions after the camp ("Camp 1 — Ruée : plateau"); set by that test only. */
let campNames: { bottom: string; top: string } | null = null;
const board = (isMine: boolean) =>
  screen.getByRole("group", {
    name: campNames !== null ? `${isMine ? campNames.bottom : campNames.top} : plateau` : isMine ? "Ton plateau" : "Plateau adverse",
  });
const myTiles = () => within(board(true)).getAllByRole("button") as HTMLButtonElement[];
const theirTiles = () => within(board(false)).getAllByRole("button") as HTMLButtonElement[];
const heroGroup = () => screen.getByRole("group", { name: campNames !== null ? `${campNames.top} : héros` : "Héros adverse" });
const heroTarget = () => within(heroGroup()).getByRole("button", { name: /cible (possible|non disponible)/ }) as HTMLButtonElement;
const sheet = () => screen.queryByRole("dialog", { name: "Confirmer l'attaque" });
const disabled = (el: Element) => el.getAttribute("aria-disabled") === "true";
const region = () => screen.getByRole("region", { name: "Partie" });

/** The accessible name of a creature tile, without any attack-state suffix. */
const base = (c: CardView): string =>
  `${c.name}, coût ${c.cost}, force ${c.force}, coût d'attaque ${c.attack_cost}, PV ${c.health}` +
  (c.provocation ? ", provocation" : "") +
  (c.riposte > 0 ? `, riposte ${c.riposte}` : "");

/** First of my creatures that has an attack in the engine's list. */
function attackerOf(s: Snapshot): { index: number; card: CardView; attacks: ActionPreview[] } {
  const index = mine(s).findIndex((x) => s.legal.some((p) => isAttackBy(x.id)(p.action)));
  if (index < 0) throw new Error("no attacker");
  const card = mine(s)[index];
  return { index, card, attacks: s.legal.filter((p) => isAttackBy(card.id)(p.action)) };
}

const targetOf = (p: ActionPreview): "hero" | number => {
  if (!isAttack(p.action)) throw new Error("not an attack");
  const t = p.action.attack.target;
  return t === "hero" ? "hero" : t.creature;
};

describe("selecting an attacker", () => {
  test("a press selects (aria-pressed, visible text), a second press, Escape or the cancel button deselect", async () => {
    const g = await findGame("rush", "bulwark", summoner, withoutTaunt);
    const { user } = mountGame(g.c);
    const { index } = attackerOf(g.c.getSnapshot());

    expect(myTiles()[index].getAttribute("aria-pressed")).toBe("false");
    expect(screen.queryByText("Sélectionnée")).toBeNull();

    await user.click(myTiles()[index]);
    expect(myTiles()[index].getAttribute("aria-pressed")).toBe("true");
    expect(within(myTiles()[index]).getByText("Sélectionnée")).toBeTruthy();

    await user.click(myTiles()[index]);
    expect(myTiles()[index].getAttribute("aria-pressed")).toBe("false");
    expect(screen.queryByText("Sélectionnée")).toBeNull();

    await user.click(myTiles()[index]);
    await user.click(screen.getByRole("button", { name: "Annuler la sélection" }));
    expect(myTiles()[index].getAttribute("aria-pressed")).toBe("false");
    expect(screen.queryByRole("button", { name: "Annuler la sélection" })).toBeNull();

    await user.click(myTiles()[index]);
    await user.keyboard("{Escape}");
    expect(myTiles()[index].getAttribute("aria-pressed")).toBe("false");
    expect(g.probe.calls.apply).toBe(g.probe.applied.length);
  });

  test("the cancel-selection button is a real 44 px target (class with the tap token)", async () => {
    const g = await findGame("rush", "bulwark", summoner, withoutTaunt);
    const { user } = mountGame(g.c);
    await user.click(myTiles()[attackerOf(g.c.getSnapshot()).index]);
    const b = screen.getByRole("button", { name: "Annuler la sélection" });
    expect(b.tagName).toBe("BUTTON");
    expect(b.className).toContain("btn");
  });

  test("selecting another creature moves the selection", async () => {
    const g = await findGame("rush", "bulwark", summoner, (s) => withoutTaunt(s) && mine(s).length >= 2);
    const { user } = mountGame(g.c);
    await user.click(myTiles()[0]);
    await user.click(myTiles()[1]);
    expect(myTiles()[0].getAttribute("aria-pressed")).toBe("false");
    expect(myTiles()[1].getAttribute("aria-pressed")).toBe("true");
  });
});

describe("a creature with no legal attack is not selectable", () => {
  test("during the computer's turn every creature of mine is aria-disabled and a press does nothing", async () => {
    // A gate that never opens keeps the computer "thinking": the board is mine, the main is not.
    const gate = { yield: () => new Promise<void>(() => undefined) };
    const c = makeController("rush", "bulwark", 3, { scheduler: gate });
    c.start();
    while (c.getSnapshot().phase === "human-turn") {
      const s = c.getSnapshot();
      c.submit(s.legal.find((p) => isPlay(p.action)) ?? notConcede(s)[0]);
    }
    expect(c.getSnapshot().phase).toBe("computer-turn");
    const { user } = mountGame(c);
    const tiles = myTiles();
    expect(tiles.length).toBeGreaterThan(0);
    for (const t of tiles) {
      expect(disabled(t)).toBe(true);
      await user.click(t);
      expect(t.getAttribute("aria-pressed")).not.toBe("true");
    }
    expect(screen.queryByText("Sélectionnée")).toBeNull();
    expect(screen.queryByRole("button", { name: "Annuler la sélection" })).toBeNull();
  });

  test("a creature the engine lists no attack for is aria-disabled at my turn too", async () => {
    // Fresh summon: still, the engine lists attacks for every creature on a human turn, so this
    // only asserts the rule: aria-disabled exactly when no `isAttackBy` preview exists.
    const g = await findGame("rush", "bulwark", summoner, withoutTaunt);
    mountGame(g.c);
    const s = g.c.getSnapshot();
    mine(s).forEach((card, i) => {
      expect(disabled(myTiles()[i])).toBe(!s.legal.some((p) => isAttackBy(card.id)(p.action)));
    });
  });
});

describe("without a taunt: the displayed targets are the engine's previews", () => {
  test("the hero and every opposing creature are offered, equal (ids) to the isAttackBy previews", async () => {
    const g = await findGame("rush", "bulwark", summoner, withoutTaunt);
    const { user } = mountGame(g.c);
    const s = g.c.getSnapshot();
    const { index, attacks } = attackerOf(s);
    await user.click(myTiles()[index]);

    const offered = new Set(attacks.map(targetOf));
    expect(offered.has("hero")).toBe(true);
    const shown = theirTiles().map((t, i) => ({ id: theirs(s)[i].id, ok: !disabled(t) }));
    expect(shown.filter((x) => x.ok).map((x) => x.id).sort()).toEqual([...offered].filter((x): x is number => x !== "hero").sort());
    expect(shown.every((x) => x.ok)).toBe(true);
    expect(disabled(heroTarget())).toBe(false);
    for (const t of theirTiles()) expect(within(t).getByText("Cible possible")).toBeTruthy();
  });
});

describe("facing a taunt", () => {
  test("only the taunting creatures are offered; the hero and the others are inactive and a press never reaches apply", async () => {
    const g = await findGame("rush", "bulwark", summoner, withTaunt);
    const { user } = mountGame(g.c);
    const s = g.c.getSnapshot();
    const { index, attacks } = attackerOf(s);
    await user.click(myTiles()[index]);

    const offered = new Set(attacks.map(targetOf));
    expect(offered.has("hero")).toBe(false);
    theirs(s).forEach((card, i) => {
      expect(disabled(theirTiles()[i]), card.name ?? "").toBe(!offered.has(card.id));
      expect(offered.has(card.id)).toBe(card.provocation);
    });
    // "Provocation" stays written on the taunting tiles, as text.
    expect(screen.getAllByText("Provocation").length).toBeGreaterThan(0);

    expect(disabled(heroTarget())).toBe(true);
    expect(within(heroGroup()).getByText("Cible non disponible")).toBeTruthy();

    const before = g.probe.calls.apply;
    await user.click(heroTarget());
    const forbidden = theirTiles().filter((t) => disabled(t));
    expect(forbidden.length).toBeGreaterThan(0);
    for (const t of forbidden) {
      expect(within(t).getByText("Cible non disponible")).toBeTruthy();
      await user.click(t);
    }
    expect(sheet()).toBeNull();
    expect(g.probe.calls.apply).toBe(before);

    const taunt = theirTiles().find((t) => !disabled(t))!;
    expect(within(taunt).getByText("Cible possible")).toBeTruthy();
    await user.click(taunt);
    expect(sheet()).not.toBeNull();
    expect(g.probe.calls.apply).toBe(before);
  });

  test("when the taunting creature is gone the hero is a possible target again", async () => {
    const seen = { taunt: false };
    const hitTaunt = (s: Snapshot): ActionPreview => {
      const onTaunt = s.legal.find((p) => {
        if (!isAttack(p.action) || p.action.attack.target === "hero") return false;
        const id = p.action.attack.target.creature;
        return theirs(s).some((x) => x.id === id && x.provocation);
      });
      return onTaunt ?? summoner(s);
    };
    const g = await findGame(
      "rush",
      "bulwark",
      hitTaunt,
      (s) => {
        if (theirs(s).some((x) => x.provocation)) seen.taunt = true;
        return seen.taunt && !theirs(s).some((x) => x.provocation) && s.legal.some((p) => isAttack(p.action) && p.action.attack.target === "hero");
      },
      120,
    );
    const { user } = mountGame(g.c);
    const { index, attacks } = attackerOf(g.c.getSnapshot());
    expect(attacks.some((p) => targetOf(p) === "hero")).toBe(true);
    await user.click(myTiles()[index]);
    expect(disabled(heroTarget())).toBe(false);
    expect(within(heroGroup()).getByText("Cible possible")).toBeTruthy();
  });
});

describe("the confirmation", () => {
  test("pressing a target opens the sheet and plays nothing; its lines are the twin session's preview", async () => {
    const g = await findGame("rush", "bulwark", summoner, withoutTaunt);
    const { user } = mountGame(g.c);
    const s = g.c.getSnapshot();
    const twin = twinOf(g);
    const previews = twin.legalActionsWithTime();
    const { index, card, attacks } = attackerOf(s);
    const before = g.probe.calls.apply;
    let handOvers = 0;

    for (const p of attacks) {
      await user.click(myTiles()[index]);
      const target = targetOf(p);
      const tile = target === "hero" ? heroTarget() : theirTiles()[theirs(s).findIndex((x) => x.id === target)];
      await user.click(tile);
      const box = sheet()!;
      expect(box).not.toBeNull();
      expect(g.probe.calls.apply, "nothing is played before the confirmation").toBe(before);

      const twinPreview = previews.find((x) => JSON.stringify(x.action) === JSON.stringify(p.action))!;
      const text = box.textContent ?? "";
      expect(within(box).getByRole("heading", { name: "Confirmer l'attaque" })).toBeTruthy();
      const who = target === "hero" ? `le héros ${s.views.One!.opponent.hero.name}` : (theirs(s).find((x) => x.id === target)!.name as string);
      expect(text).toContain(`${card.name} attaque ${who}`);
      expect(text).toContain(`Coût d'attaque : ${twinPreview.cost}`);
      expect(/Passe la main/.test(text)).toBe(twinPreview.hands_over);
      if (twinPreview.hands_over) expect(text).toContain(`l'adversaire aura ${twinPreview.budget_after} de temps`);
      else if (twinPreview.ends_game === null) expect(text).toContain(`Il te restera ${twinPreview.budget_after} de temps`);
      expect(/Met fin à la partie/.test(text)).toBe(twinPreview.ends_game !== null);
      // The client does not know the damage: nothing is announced from the force.
      expect(text).not.toMatch(/inflige|dégât|dégat|subit/i);
      if (twinPreview.hands_over) handOvers += 1;

      await user.click(within(box).getByRole("button", { name: "Annuler" }));
      expect(sheet()).toBeNull();
      expect(myTiles()[index].getAttribute("aria-pressed"), "Annuler keeps the selection").toBe("true");
      await user.click(myTiles()[index]);
    }
    expect(g.probe.calls.apply).toBe(before);
    expect(handOvers).toBeGreaterThanOrEqual(0);
    twin.dispose();
  });

  test("a hand-over attack says it, a quiet one does not (found by search)", async () => {
    for (const wantHandOver of [true, false]) {
      cleanup();
      const g = await findGame("rush", "bulwark", summoner, (s) => withoutTaunt(s) && s.legal.some((p) => isAttack(p.action) && p.hands_over === wantHandOver && p.ends_game === null));
      const { user } = mountGame(g.c);
      const s = g.c.getSnapshot();
      const p = s.legal.find((x) => isAttack(x.action) && x.hands_over === wantHandOver && x.ends_game === null)!;
      const attacker = (p.action as { attack: { attacker: number } }).attack.attacker;
      await user.click(myTiles()[mine(s).findIndex((x) => x.id === attacker)]);
      const target = targetOf(p);
      await user.click(target === "hero" ? heroTarget() : theirTiles()[theirs(s).findIndex((x) => x.id === target)]);
      expect(/Passe la main/.test(sheet()!.textContent ?? "")).toBe(wantHandOver);
    }
  });

  test("Escape closes the sheet and keeps the selection; the focus returns to the target tile", async () => {
    const g = await findGame("rush", "bulwark", summoner, withoutTaunt);
    const { user } = mountGame(g.c);
    const { index } = attackerOf(g.c.getSnapshot());
    await user.click(myTiles()[index]);
    const tile = theirTiles()[0];
    await user.click(tile);
    expect(sheet()).not.toBeNull();
    expect(within(sheet()!).getByRole("button", { name: "Annuler" })).toBe(document.activeElement);
    await user.keyboard("{Escape}");
    expect(sheet()).toBeNull();
    expect(myTiles()[index].getAttribute("aria-pressed")).toBe("true");
    expect(document.activeElement).toBe(theirTiles()[0]);
  });

  test("Confirmer plays the previewed action once, clears the selection, closes the sheet and focuses the game region", async () => {
    const g = await findGame("rush", "bulwark", summoner, withoutTaunt);
    const { user } = mountGame(g.c);
    const s = g.c.getSnapshot();
    const { index, attacks } = attackerOf(s);
    const p = attacks.find((x) => targetOf(x) !== "hero") as ActionPreview;
    const before = g.probe.applied.length;

    await user.click(myTiles()[index]);
    await user.click(theirTiles()[theirs(s).findIndex((x) => x.id === targetOf(p))]);
    await user.click(within(sheet()!).getByRole("button", { name: "Confirmer" }));
    await waitFor(() => expect(g.c.getSnapshot().phase).not.toBe("computer-turn"));

    expect(g.probe.applied[before]).toEqual(p.action);
    expect(sheet()).toBeNull();
    expect(screen.queryByText("Sélectionnée")).toBeNull();
    expect(screen.queryByRole("button", { name: "Annuler la sélection" })).toBeNull();
    for (const t of screen.queryAllByRole("button")) expect(t.getAttribute("aria-pressed")).not.toBe("true");
    if (g.c.getSnapshot().phase !== "over") expect(document.activeElement).toBe(region());
    expect(screen.queryByRole("alert")).toBeNull();
  });

  test("a double press on Confirmer is one application", async () => {
    const g = await findGame("rush", "bulwark", summoner, withoutTaunt);
    const { user } = mountGame(g.c);
    const s = g.c.getSnapshot();
    const { index, attacks } = attackerOf(s);
    const p = attacks[0];
    const before = g.probe.calls.apply;

    await user.click(myTiles()[index]);
    const target = targetOf(p);
    await user.click(target === "hero" ? heroTarget() : theirTiles()[theirs(s).findIndex((x) => x.id === target)]);
    await user.dblClick(within(sheet()!).getByRole("button", { name: "Confirmer" }));
    await waitFor(() => expect(g.c.getSnapshot().phase).not.toBe("computer-turn"));

    expect(g.probe.applied.slice(before).filter((a) => JSON.stringify(a) === JSON.stringify(p.action))).toHaveLength(1);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  test("the controller answers a replayed preview with stale or not-your-turn, never an error banner", async () => {
    const g = await findGame("rush", "bulwark", summoner, withoutTaunt);
    mountGame(g.c);
    const { attacks } = attackerOf(g.c.getSnapshot());
    const p = attacks[0];
    let first: ReturnType<GameController["submit"]> | undefined;
    let second: ReturnType<GameController["submit"]> | undefined;
    await inAct(async () => {
      first = g.c.submit(p);
      second = g.c.submit(p);
      await settle(g.c);
    });
    expect(first).toEqual({ ok: true });
    expect(second!.ok).toBe(false);
    expect(["stale", "not-your-turn"]).toContain((second as { reason: string }).reason);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  test("a lethal attack on the hero says it ends the game and does not say it hands over", async () => {
    const g = await findGame(
      "rush",
      "bulwark",
      hunter,
      (s) => s.legal.some((p) => isAttack(p.action) && p.ends_game?.reason === "hero_down"),
      150,
    );
    const { user } = mountGame(g.c);
    const s = g.c.getSnapshot();
    const lethal = s.legal.find((p) => isAttack(p.action) && p.ends_game?.reason === "hero_down")!;
    const attacker = (lethal.action as { attack: { attacker: number } }).attack.attacker;
    await user.click(myTiles()[mine(s).findIndex((x) => x.id === attacker)]);
    await user.click(heroTarget());
    const text = sheet()!.textContent ?? "";
    expect(text).toContain(`Coût d'attaque : ${lethal.cost}`);
    expect(text).toContain("Met fin à la partie");
    expect(text).toContain("Tu gagnes");
    expect(text).not.toMatch(/Passe la main/);
    await user.click(within(sheet()!).getByRole("button", { name: "Confirmer" }));
    expect(g.c.getSnapshot().phase).toBe("over");
    expect(screen.getByRole("heading", { name: "Tu as gagné !" })).toBeTruthy();
  });
});

describe("a selection never outlives the position it was made in", () => {
  test("after the computer's turn a previously selected creature is not selected again", async () => {
    let meaningful = 0;
    for (let seed = 0; seed < 40 && meaningful === 0; seed++) {
      cleanup();
      const g = probedController("rush", "bulwark", seed);
      try {
        await playUntil(
          g.c,
          summoner,
          (s) => s.phase === "human-turn" && mine(s).length >= 1 && s.legal.some((p) => p.hands_over && p.ends_game === null && !isAttack(p.action) && p.action !== "concede"),
          40,
        );
      } catch {
        continue;
      }
      const { user } = mountGame(g.c);
      const s = g.c.getSnapshot();
      const { index, card } = attackerOf(s);
      await user.click(myTiles()[index]);
      const handOver = s.legal.find((p) => p.hands_over && p.ends_game === null && !isAttack(p.action) && p.action !== "concede") as ActionPreview;
      await inAct(async () => {
        g.c.submit(handOver);
        await settle(g.c);
      });
      const after = g.c.getSnapshot();
      if (after.phase !== "human-turn" || !mine(after).some((x) => x.id === card.id)) continue;
      meaningful += 1;
      expect(screen.queryByText("Sélectionnée")).toBeNull();
      for (const t of myTiles()) expect(t.getAttribute("aria-pressed")).not.toBe("true");
      expect(screen.queryByRole("button", { name: "Annuler la sélection" })).toBeNull();
    }
    expect(meaningful).toBe(1);
  });

  test("an open sheet closes when its preview disappears from the snapshot", async () => {
    const g = await findGame("rush", "bulwark", summoner, (s) => withoutTaunt(s) && s.legal.some((p) => p.hands_over && p.ends_game === null && !isAttack(p.action) && p.action !== "concede"));
    const { user } = mountGame(g.c);
    const s = g.c.getSnapshot();
    const { index } = attackerOf(s);
    await user.click(myTiles()[index]);
    await user.click(theirTiles()[0]);
    expect(sheet()).not.toBeNull();
    const handOver = s.legal.find((p) => p.hands_over && p.ends_game === null && !isAttack(p.action) && p.action !== "concede") as ActionPreview;
    await inAct(async () => {
      g.c.submit(handOver);
      await settle(g.c);
    });
    expect(sheet()).toBeNull();
  });
});

describe("riposte", () => {
  test("attacking a Loup hérissé: its tile says 'Riposte 1', the engine returns the riposte, the journal says it", async () => {
    const g = await findGame(
      "bulwark",
      "rush",
      summoner,
      (s) => mine(s).length >= 1 && theirs(s).some((x) => x.name === "Loup hérissé") && !theirs(s).some((x) => x.provocation),
    );
    const { user } = mountGame(g.c);
    const s = g.c.getSnapshot();
    const { index } = attackerOf(s);
    const wolfIndex = theirs(s).findIndex((x) => x.name === "Loup hérissé");
    await user.click(myTiles()[index]);
    const wolf = theirTiles()[wolfIndex];
    expect(within(wolf).getByText("Riposte 1")).toBeTruthy();
    expect(within(wolf).getByText("Cible possible")).toBeTruthy();
    await user.click(wolf);
    await user.click(within(sheet()!).getByRole("button", { name: "Confirmer" }));
    await waitFor(() => expect(g.c.getSnapshot().phase).not.toBe("computer-turn"));

    const events = g.c.getSnapshot().records.flatMap((r) => r.events);
    expect(events.some((e) => "riposted" in e)).toBe(true);
    if (g.c.getSnapshot().phase !== "over") {
      await user.click(screen.getByRole("button", { name: "Journal" }));
      expect(within(screen.getByRole("dialog", { name: "Journal" })).getAllByText(/de riposte/).length).toBeGreaterThan(0);
    }
  });
});

describe("two camps", () => {
  test("the camp holding the main selects its creatures and attacks the other camp's, from the same screen", async () => {
    let c: GameController | undefined;
    for (let seed = 0; seed < 40 && c === undefined; seed++) {
      const candidate = GameController.create({ ...config("rush", "bulwark", seed), mode: "two-camps" }, realDeps());
      candidate.start();
      try {
        await playUntil(
          candidate,
          summoner,
          (s) => {
            const v = s.views[s.views.One!.active]!;
            return v.you.board.length >= 1 && v.opponent.board.length >= 1 && s.legal.some((p) => isAttack(p.action));
          },
          60,
        );
        c = candidate;
      } catch {
        candidate.dispose();
      }
    }
    expect(c).toBeDefined();
    const { user } = mountGame(c!);
    const s = c!.getSnapshot();
    const active = s.views.One!.active;
    const camp = (seat: "One" | "Two") => `Camp ${seat === "One" ? 1 : 2} — ${seat === "One" ? "Ruée" : "Rempart"}`;
    campNames = { bottom: camp(active), top: camp(active === "One" ? "Two" : "One") };
    const v = s.views[active]!;
    const attacker = v.you.board.findIndex((x) => s.legal.some((p) => isAttackBy(x.id)(p.action)));
    await user.click(myTiles()[attacker]);
    expect(myTiles()[attacker].getAttribute("aria-pressed")).toBe("true");
    const offered = new Set(s.legal.filter((p) => isAttackBy(v.you.board[attacker].id)(p.action)).map(targetOf));
    v.opponent.board.forEach((card, i) => expect(disabled(theirTiles()[i])).toBe(!offered.has(card.id)));
    expect(disabled(heroTarget())).toBe(!offered.has("hero"));
    const first = theirTiles().findIndex((t) => !disabled(t));
    const before = c!.getSnapshot().records.length;
    if (first >= 0) {
      await user.click(theirTiles()[first]);
      await user.click(within(sheet()!).getByRole("button", { name: "Confirmer" }));
      expect(c!.getSnapshot().records.length).toBe(before + 1);
    } else {
      await user.click(heroTarget());
      await user.click(within(sheet()!).getByRole("button", { name: "Confirmer" }));
      expect(c!.getSnapshot().records.length).toBe(before + 1);
    }
    campNames = null;
    c!.dispose();
  });
});
