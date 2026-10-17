import { render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { EngineError } from "../src/engine/errors";
import { DATA_VERSION } from "../src/data/starter";
import { Home } from "../src/ui/Home";
import { messageFor } from "../src/ui/errors.fr";
import { fr } from "../src/ui/fr";
import { launch, renderApp, seedField, testFactory, useAppEngine } from "./support/app";
import { fakeCrypto } from "./support/ui";

useAppEngine();

const home = async () => screen.findByRole("heading", { name: "Choisis ton deck" });

describe("home screen", () => {
  test("shows the build identifier it is given, apart from the data version (#80)", () => {
    render(<Home decks={[{ id: "a", name: "A" }]} dataVersion="starter@1.1.0" buildId="abc1234" status="ready" loadError={null} startError={null} onStart={() => {}} onReload={() => {}} />);
    expect(screen.getByText("Version : abc1234")).toBeTruthy();
    expect(screen.getByText("Données : starter@1.1.0")).toBeTruthy();
  });

  test("shows the data version, the decks, the opponent's deck (computer mode by default)", async () => {
    const { user } = renderApp();
    await home();
    expect(screen.getByText(`Données : ${DATA_VERSION}`)).toBeTruthy();
    expect(DATA_VERSION).toBe("starter@1.1.0");
    // The build identifier is its own paragraph (`dev` outside a deployment, #80).
    expect(screen.getByText("Version : dev")).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Ruée" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Rempart" })).toBeTruthy();
    // Two mode radios plus the two decks.
    expect(screen.getAllByRole("radio")).toHaveLength(4);
    // The other deck goes to the opponent, and follows the choice.
    expect(screen.getByText("L'adversaire joue : Rempart")).toBeTruthy();
    await user.click(screen.getByRole("radio", { name: "Rempart" }));
    expect(screen.getByText("L'adversaire joue : Ruée")).toBeTruthy();
  });

  test("mode group: computer by default, two camps on demand, the second deck only in two camps", async () => {
    const { user } = renderApp();
    await home();
    const mode = screen.getByRole("group", { name: "Mode" });
    const computer = within(mode).getByRole("radio", { name: "Contre l'ordinateur" }) as HTMLInputElement;
    const camps = within(mode).getByRole("radio", { name: "Je joue les deux camps" }) as HTMLInputElement;
    expect(computer.checked).toBe(true);
    expect(camps.checked).toBe(false);
    expect(screen.getByRole("group", { name: "Ton deck" })).toBeTruthy();
    expect(screen.queryByRole("group", { name: "Deck du camp 1" })).toBeNull();
    expect(screen.queryByRole("group", { name: "Deck du camp 2" })).toBeNull();

    await user.click(camps);
    expect(camps.checked).toBe(true);
    const one = screen.getByRole("group", { name: "Deck du camp 1" });
    const two = screen.getByRole("group", { name: "Deck du camp 2" });
    expect(screen.queryByRole("group", { name: "Ton deck" })).toBeNull();
    expect(screen.queryByText(/L'adversaire joue/)).toBeNull();
    // Default: the OTHER deck goes to camp 2, and follows camp 1 until camp 2 is chosen.
    const checked = (g: HTMLElement) => (within(g).getAllByRole("radio") as HTMLInputElement[]).find((r) => r.checked)?.labels?.[0]?.textContent;
    expect([checked(one), checked(two)]).toEqual(["Ruée", "Rempart"]);
    await user.click(within(one).getByRole("radio", { name: "Rempart" }));
    expect([checked(one), checked(two)]).toEqual(["Rempart", "Ruée"]);
    // The mirror is accepted: the same deck on both sides.
    await user.click(within(two).getByRole("radio", { name: "Rempart" }));
    expect([checked(one), checked(two)]).toEqual(["Rempart", "Rempart"]);
    // Back to the computer: the opponent line returns.
    await user.click(computer);
    expect(screen.getByText("L'adversaire joue : Ruée")).toBeTruthy();
  });

  test("the chosen mode and decks reach the controller: a two-camps game, camp labels, no computer", async () => {
    const chooseAction = vi.fn(() => "concede" as const);
    const { user, counters } = renderApp({ chooseAction });
    await launch(user, { twoCamps: true, deckOne: "Rempart", deckTwo: "Ruée", seed: "4" });
    // Rempart is seat One (camp 1) but Ruée has the initiative: the camp holding the main is at the bottom.
    expect(screen.getByRole("group", { name: "Camp 2 — Ruée : héros" })).toBeTruthy();
    expect(screen.getByRole("group", { name: "Camp 1 — Rempart : héros" })).toBeTruthy();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByText(/ordinateur/i)).toBeNull();
    expect(counters).toMatchObject({ creations: 1, seeds: [4] });
    expect(chooseAction).not.toHaveBeenCalled();
  });

  test("the mirror launches a game, camps told apart by their number", async () => {
    const { user } = renderApp();
    await launch(user, { twoCamps: true, deckOne: "Ruée", deckTwo: "Ruée", seed: "9" });
    expect(screen.getByRole("group", { name: "Camp 1 — Ruée : héros" })).toBeTruthy();
    expect(screen.getByRole("group", { name: "Camp 2 — Ruée : héros" })).toBeTruthy();
    expect(screen.getByRole("group", { name: "Camp 1 — Ruée : main" })).toBeTruthy();
    expect(screen.getByRole("group", { name: "Camp 2 — Ruée : main" })).toBeTruthy();
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
