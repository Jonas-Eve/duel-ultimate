import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { EngineError } from "../src/engine/errors";
import { DATA_VERSION } from "../src/data/starter";
import { messageFor } from "../src/ui/errors.fr";
import { fr } from "../src/ui/fr";
import { launch, renderApp, seedField, testFactory, useAppEngine } from "./support/app";
import { fakeCrypto } from "./support/ui";

useAppEngine();

const home = async () => screen.findByRole("heading", { name: "Choisis ton deck" });

describe("home screen", () => {
  test("shows the data version, the decks, the opponent's deck, and no mode selector", async () => {
    const { user } = renderApp();
    await home();
    expect(screen.getByText(`Données : ${DATA_VERSION}`)).toBeTruthy();
    expect(DATA_VERSION).toBe("starter@1.0.0");
    expect(screen.getByRole("radio", { name: "Ruée" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Rempart" })).toBeTruthy();
    expect(screen.getAllByRole("radio")).toHaveLength(2);
    // The other deck goes to the opponent, and follows the choice.
    expect(screen.getByText("L'adversaire joue : Rempart")).toBeTruthy();
    await user.click(screen.getByRole("radio", { name: "Rempart" }));
    expect(screen.getByText("L'adversaire joue : Ruée")).toBeTruthy();
    // One single mode: no mode control, no disabled "soon" control.
    expect(screen.queryByText(/deux camps|je joue les deux|mode/i)).toBeNull();
  });

  test("does not say who starts (initiative decides, known only once the game exists)", async () => {
    renderApp();
    await home();
    expect(screen.queryByText(/commence|ouvre|premier|initiative/i)).toBeNull();
  });

  test("seed field has its label and help text", async () => {
    renderApp();
    await home();
    expect(seedField().getAttribute("aria-describedby")).toBeTruthy();
    expect(screen.getByText("Laisse vide pour une graine aléatoire (nombre entier de 0 à 4 294 967 295)")).toBeTruthy();
  });

  test.each(["-1", "1.5", "4294967296", "abc"])("invalid seed %j: alert tied to the field, no session created", async (text) => {
    const { user, counters, crypto } = renderApp();
    await launch(user, { seed: text });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(messageFor("invalid_seed"));
    expect(seedField().getAttribute("aria-describedby")?.split(" ")).toContain(alert.id);
    expect(counters.creations).toBe(0);
    expect(crypto.spy).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Abandonner" })).toBeNull();
  });

  test("empty seed: the injected crypto is called and its value is the seed shown in the game", async () => {
    const { user, counters, crypto } = renderApp();
    await launch(user);
    await screen.findByText("Graine : 12345");
    expect(crypto.spy).toHaveBeenCalledTimes(1);
    expect(counters.seeds).toEqual([12345]);
  });

  test("a typed valid seed is passed as is and crypto is not called", async () => {
    const { user, counters, crypto } = renderApp();
    await launch(user, { seed: "7" });
    await screen.findByText("Graine : 7");
    expect(crypto.spy).not.toHaveBeenCalled();
    expect(counters.seeds).toEqual([7]);
  });

  test("4294967295 is accepted", async () => {
    const { user, counters } = renderApp();
    await launch(user, { seed: "4294967295" });
    await screen.findByText("Graine : 4294967295");
    expect(counters.seeds).toEqual([4294967295]);
  });

  test("the chosen deck is player one, the other deck the opponent", async () => {
    const { user } = renderApp();
    await launch(user, { deck: "Rempart", seed: "3" });
    const me = await screen.findByRole("group", { name: "Ton héros" });
    expect(within(me).getByText("Gardienne du rempart")).toBeTruthy();
    expect(within(screen.getByRole("group", { name: "Héros adverse" })).getByText("Capitaine d'avant-garde")).toBeTruthy();
  });

  test("the launch button is disabled while the engine loads", async () => {
    renderApp({ engine: () => new Promise(() => undefined) });
    expect((screen.getByRole("button", { name: "Lancer la partie" }) as HTMLButtonElement).disabled).toBe(true);
  });

  test("a load failure shows a French message and Reload tries the loader again", async () => {
    let calls = 0;
    const { user } = renderApp({
      engine: () => {
        calls += 1;
        return calls === 1
          ? Promise.reject(new EngineError("wasm_load_failed", "fetch failed"))
          : Promise.resolve(testFactory);
      },
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain(messageFor("wasm_load_failed"));
    expect(alert.textContent).not.toContain("fetch failed");
    expect((screen.getByRole("button", { name: "Lancer la partie" }) as HTMLButtonElement).disabled).toBe(true);
    await user.click(screen.getByRole("button", { name: "Recharger" }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Lancer la partie" }) as HTMLButtonElement).disabled).toBe(false));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(calls).toBe(2);
  });
});

describe("failures of startGame are shown, never thrown", () => {
  test.each([
    ["null", null],
    ["without getRandomValues", {} as unknown as Crypto],
  ])("entropy unavailable (%s) and empty seed: French alert, focus on the seed field, nothing created", async (_n, crypto) => {
    const { user, counters } = renderApp({ crypto });
    await launch(user);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toBe(fr.entropyUnavailable);
    expect(document.activeElement).toBe(seedField());
    expect(seedField().getAttribute("aria-describedby")?.split(" ")).toContain(alert.id);
    expect(counters.creations).toBe(0);
    expect(counters.seeds).toEqual([]);
    expect(screen.queryByRole("button", { name: "Abandonner" })).toBeNull();
    expect(screen.queryByText(/Graine : 0$/)).toBeNull();
  });

  test("then a typed seed starts the game without touching crypto", async () => {
    const { user, counters } = renderApp({ crypto: null });
    await launch(user);
    await screen.findByRole("alert");
    await user.type(seedField(), "5");
    await user.click(screen.getByRole("button", { name: "Lancer la partie" }));
    await screen.findByText("Graine : 5");
    expect(counters.seeds).toEqual([5]);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  test("invalid seed: it is the startGame result that is displayed (text of messageFor)", async () => {
    const { user } = renderApp();
    await launch(user, { seed: "4294967296" });
    expect((await screen.findByRole("alert")).textContent).toBe(messageFor("invalid_seed"));
    // Fixing the seed clears the way: the next launch works.
    await user.clear(seedField());
    await user.type(seedField(), "4294967295");
    await user.click(screen.getByRole("button", { name: "Lancer la partie" }));
    await screen.findByText("Graine : 4294967295");
  });

  test.each(["invalid_setup", "unknown_deck"])("an engine that throws %s: home with the French alert, never the English text", async (code) => {
    let calls = 0;
    const { user } = renderApp({
      engine: () =>
        Promise.resolve(((...args: Parameters<typeof testFactory>) => {
          calls += 1;
          if (calls === 1) throw new Error(JSON.stringify({ code, message: "boom-english-text" }));
          return testFactory(...args);
        }) as typeof testFactory),
    });
    await launch(user, { seed: "3" });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain(messageFor(code));
    expect(alert.textContent).not.toContain("boom-english-text");
    await home();
    expect(screen.queryByRole("button", { name: "Abandonner" })).toBeNull();
    expect(calls).toBe(1);
    // No controller or instance was kept: the next game can be launched.
    await user.click(screen.getByRole("button", { name: "Lancer la partie" }));
    await screen.findByRole("button", { name: "Abandonner" });
    expect(calls).toBe(2);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  test("a fake crypto of another value gives another seed", async () => {
    const { user, counters } = renderApp({ crypto: fakeCrypto(99) });
    await launch(user);
    await screen.findByText("Graine : 99");
    expect(counters.seeds).toEqual([99]);
  });
});
