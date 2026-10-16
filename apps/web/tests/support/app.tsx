// Rendering `App` with the real WASM engine supplied through its `deps` prop (no engine mock).
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { afterEach, beforeAll, expect } from "vitest";
import { App, type AppDeps } from "../../src/App";
import { fakeCrypto } from "./ui";
import { initTestEngine, instrumentedFactory, testFactory } from "./engine";
import { immediateScheduler } from "./policies";

export function useAppEngine(): void {
  beforeAll(async () => {
    await initTestEngine();
  });
  afterEach(cleanup);
}

export function renderApp(over: Partial<AppDeps> = {}, opts: { strict?: boolean } = {}) {
  const { factory, counters } = instrumentedFactory();
  const crypto = fakeCrypto(12345);
  const deps: AppDeps = {
    engine: () => Promise.resolve(factory),
    crypto,
    scheduler: immediateScheduler,
    now: () => 0,
    ...over,
  };
  const ui = opts.strict ? (
    <StrictMode>
      <App deps={deps} />
    </StrictMode>
  ) : (
    <App deps={deps} />
  );
  const utils = render(ui);
  return { ...utils, user: userEvent.setup(), counters, crypto, deps };
}

export const seedField = () => screen.getByLabelText("Graine (optionnelle)") as HTMLInputElement;

/**
 * Waits for the engine, picks a mode, decks and a seed (all optional) and presses "Lancer la partie".
 * `deck` is the human's deck against the computer; in two camps `deckOne` / `deckTwo` are the camps' decks
 * (names as displayed, e.g. "Ruée").
 */
export async function launch(
  user: ReturnType<typeof userEvent.setup>,
  opts: { deck?: string; seed?: string; twoCamps?: boolean; deckOne?: string; deckTwo?: string } = {},
): Promise<void> {
  const button = (await screen.findByRole("button", { name: "Lancer la partie" })) as HTMLButtonElement;
  await waitFor(() => expect(button.disabled).toBe(false));
  if (opts.twoCamps) await user.click(screen.getByRole("radio", { name: "Je joue les deux camps" }));
  if (opts.deck) await user.click(screen.getByRole("radio", { name: opts.deck }));
  for (const [legend, deck] of [
    ["Deck du camp 1", opts.deckOne],
    ["Deck du camp 2", opts.deckTwo],
  ] as const) {
    if (deck) await user.click(within(screen.getByRole("group", { name: legend })).getByRole("radio", { name: deck }));
  }
  if (opts.seed !== undefined) {
    await user.clear(seedField());
    if (opts.seed !== "") await user.type(seedField(), opts.seed);
  }
  await user.click(button);
}

export { testFactory };
