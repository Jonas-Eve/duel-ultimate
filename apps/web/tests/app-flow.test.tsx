// End to end through `App` on the real WASM: launching, playing, replaying, leaving.
import { act, screen, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { messageFor } from "../src/ui/errors.fr";
import { fr } from "../src/ui/fr";
import { faultyFactory } from "./support/faulty";
import { launch, renderApp, useAppEngine } from "./support/app";
import { fakeCrypto } from "./support/ui";
import { playTile } from "./support/play";
import type userEvent from "@testing-library/user-event";

useAppEngine();

const handLabels = () =>
  within(screen.getByRole("group", { name: "Ta main" }))
    .getAllByRole("button")
    .map((b) => b.getAttribute("aria-label"));

async function concede(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Abandonner" }));
  await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Abandonner" }));
}

describe("replay and leaving", () => {
  test("same seed: the same starting hand, a new instance, the old one freed", async () => {
    const { user, counters } = renderApp();
    await launch(user, { seed: "7" });
    const first = handLabels();
    await concede(user);
    expect(counters).toMatchObject({ creations: 1, frees: 0 });

    await user.click(screen.getByRole("button", { name: "Rejouer avec la même graine" }));
    expect(handLabels()).toEqual(first);
    expect(screen.getByText("Graine : 7")).toBeTruthy();
    expect(counters).toMatchObject({ creations: 2, frees: 1, callsAfterFree: 0 });
    expect(counters.seeds).toEqual([7, 7]);
  });

  test("new seed: crypto is called again and the displayed seed changes", async () => {
    const crypto = fakeCrypto(101, 102);
    const { user, counters } = renderApp({ crypto });
    await launch(user);
    await screen.findByText("Graine : 101");
    await concede(user);
    await user.click(screen.getByRole("button", { name: "Rejouer avec une nouvelle graine" }));
    await screen.findByText("Graine : 102");
    expect(crypto.spy).toHaveBeenCalledTimes(2);
    expect(counters).toMatchObject({ creations: 2, frees: 1 });
  });

  test("replaying with a new seed without entropy: home screen with the alert, the old instance freed", async () => {
    const fake = fakeCrypto(5);
    const crypto: { getRandomValues?: Crypto["getRandomValues"] } = { getRandomValues: fake.getRandomValues };
    const { user, counters } = renderApp({ crypto: crypto as Crypto });
    await launch(user, { seed: "5" }); // a typed seed needs no entropy
    await concede(user);
    delete crypto.getRandomValues;
    await user.click(screen.getByRole("button", { name: "Rejouer avec une nouvelle graine" }));
    expect((await screen.findByRole("alert")).textContent).toBe(fr.entropyUnavailable);
    expect(document.activeElement).toBe(screen.getByLabelText("Graine (optionnelle)"));
    expect(counters).toMatchObject({ creations: 1, frees: 1 });
    expect(screen.queryByRole("button", { name: "Abandonner" })).toBeNull();
  });

  test("back to the home screen frees the instance; unmounting frees the last one", async () => {
    const { user, counters, unmount } = renderApp();
    await launch(user, { seed: "7" });
    await concede(user);
    await user.click(screen.getByRole("button", { name: "Retour à l'accueil" }));
    await screen.findByRole("heading", { name: "Choisis ton deck" });
    expect(counters).toMatchObject({ creations: 1, frees: 1 });

    await user.click(screen.getByRole("button", { name: "Lancer la partie" }));
    await screen.findByRole("button", { name: "Abandonner" });
    expect(counters).toMatchObject({ creations: 2, frees: 1 });
    unmount();
    expect(counters).toMatchObject({ creations: 2, frees: 2, callsAfterFree: 0 });
  });

  test("N games make N frees", async () => {
    const { user, counters, unmount } = renderApp();
    await launch(user, { seed: "1" });
    for (let i = 0; i < 3; i++) {
      await concede(user);
      await user.click(screen.getByRole("button", { name: "Rejouer avec la même graine" }));
    }
    unmount();
    expect(counters.creations).toBe(4);
    expect(counters.frees).toBe(4);
  });
});

describe("StrictMode", () => {
  test("the controller survives the double effects: launch and summon a card", async () => {
    const { user, counters } = renderApp({}, { strict: true });
    await launch(user, { seed: "3" });
    const hand = within(screen.getByRole("group", { name: "Ta main" }));
    const before = hand.getAllByRole("button").length;
    await playTile(user, hand.getAllByRole("button")[0]);
    expect(within(screen.getByRole("group", { name: "Ta main" })).getAllByRole("button").length).toBe(before - 1);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(counters.frees).toBe(0);
    expect(counters.callsAfterFree).toBe(0);
    expect(counters.creations).toBe(1);
  });
});

describe("the computer opens (human plays bulwark)", () => {
  test("the indicator is there on arrival, controls are inactive, then the human can play", async () => {
    const gates: (() => void)[] = [];
    const { user } = renderApp({ scheduler: { yield: () => new Promise<void>((r) => gates.push(r)) } });
    await launch(user, { deck: "Rempart", seed: "3" });
    // The tiles open their detail (reading is always possible); only "Jouer" is inactive, with its reason.
    const tiles = within(screen.getByRole("group", { name: "Ta main" })).getAllByRole("button");
    for (const b of tiles) expect((b as HTMLButtonElement).disabled).toBe(false);
    await user.click(tiles[0]);
    const play = within(screen.getByRole("dialog")).getByRole("button", { name: "Jouer" });
    expect(play.getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByText(fr.playUnavailableComputer)).toBeTruthy();
    await user.click(play);
    await user.keyboard("{Escape}");
    expect(screen.getByRole("status").textContent).toBe("L'ordinateur joue…");
    await act(async () => {
      gates.shift()?.();
      await Promise.resolve();
    });
    await screen.findByText(/L'ordinateur a joué \d+ actions?\. À toi de jouer\./);
    expect(screen.queryByRole("status")).toBeNull();
  });
});

describe("fatal error through App", () => {
  test("the dedicated screen, then the home screen", async () => {
    const { factory } = faultyFactory("apply");
    const { user } = renderApp({ engine: () => Promise.resolve(factory) });
    await launch(user, { seed: "3" });
    const hand = within(screen.getByRole("group", { name: "Ta main" }));
    await playTile(user, hand.getAllByRole("button")[0]);
    expect(screen.getByText(messageFor("internal_error"))).toBeTruthy();
    expect(screen.queryByRole("group", { name: "Ta main" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Retour à l'accueil" }));
    await screen.findByRole("heading", { name: "Choisis ton deck" });
  });
});
