// Rendering `App` with the real WASM engine supplied through its `deps` prop (no engine mock).
import { cleanup, render, screen, waitFor } from "@testing-library/react";
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

/** Waits for the engine, picks a deck and a seed (both optional) and presses "Lancer la partie". */
export async function launch(
  user: ReturnType<typeof userEvent.setup>,
  opts: { deck?: string; seed?: string } = {},
): Promise<void> {
  const button = (await screen.findByRole("button", { name: "Lancer la partie" })) as HTMLButtonElement;
  await waitFor(() => expect(button.disabled).toBe(false));
  if (opts.deck) await user.click(screen.getByRole("radio", { name: opts.deck }));
  if (opts.seed !== undefined) {
    await user.clear(seedField());
    if (opts.seed !== "") await user.type(seedField(), opts.seed);
  }
  await user.click(button);
}

export { testFactory };
