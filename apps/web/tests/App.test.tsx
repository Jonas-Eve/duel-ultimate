import { screen, waitFor } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import type { GameFactory } from "../src/engine/session";
import { renderApp, testFactory, useAppEngine } from "./support/app";

useAppEngine();

describe("App", () => {
  test("renders the title", async () => {
    renderApp();
    expect(screen.getByRole("heading", { level: 1, name: "duel-ultimate" })).toBeTruthy();
    await screen.findByRole("heading", { name: "Choisis ton deck" });
  });

  test("shows the loading state while the engine loads, then the home screen", async () => {
    let release: (f: GameFactory) => void = () => undefined;
    const { counters } = renderApp({
      engine: () =>
        new Promise((resolve) => {
          release = resolve;
        }),
    });
    expect(screen.getByText("Chargement du moteur…")).toBeTruthy();
    const button = screen.getByRole("button", { name: "Lancer la partie" }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    release(testFactory);
    await waitFor(() => expect(button.disabled).toBe(false));
    expect(screen.queryByText("Chargement du moteur…")).toBeNull();
    expect(counters.creations).toBe(0);
  });
});
