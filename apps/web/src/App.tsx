// Assembles the screens: the engine is loaded first (loading screen), then the home screen, then
// the game. `App` is the only place that reads the starter data and knows the engine factory;
// `ui/` and `game/` get what they need by props and closures. The controller is created in an
// event handler (never in an effect: StrictMode replays effects, and a cleanup would free the
// second mount's instance), through `startGame`, which never throws.
import { useEffect, useRef, useState } from "react";
import { CARDS_JSON, DATA_VERSION, DECKS, DECKS_JSON } from "./data/starter";
import { parseEngineError, type EngineError } from "./engine/errors";
import { loadEngine } from "./engine/load";
import type { GameFactory } from "./engine/session";
import { createWasmSession } from "./engine/wasm-session";
import type { ComputerPolicy } from "./game/computer";
import type { ControllerDeps, GameController, Mode, SessionRequest } from "./game/controller";
import { startGame } from "./game/start";
import { fr } from "./ui/fr";
import { GameScreen } from "./ui/GameScreen";
import { Home, type StartFailure } from "./ui/Home";

/** Injection points for tests: the real engine is supplied, never mocked. All optional. */
export interface AppDeps {
  engine?: () => Promise<GameFactory>;
  /** `null` simulates a device without `crypto.getRandomValues`. */
  crypto?: Pick<Crypto, "getRandomValues"> | null;
  policy?: ComputerPolicy;
  scheduler?: ControllerDeps["scheduler"];
  now?: () => number;
}

interface Running {
  key: number;
  controller: GameController;
}

export function App({ deps = {} }: { deps?: AppDeps }) {
  const load = deps.engine ?? loadEngine;
  const [factory, setFactory] = useState<GameFactory | null>(null);
  const [loadError, setLoadError] = useState<EngineError | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [running, setRunning] = useState<Running | null>(null);
  const [startError, setStartError] = useState<StartFailure | null>(null);
  const current = useRef<GameController | null>(null);
  const counter = useRef(0);

  // The engine loads during the loading screen, with a cancellation flag for unmounting.
  useEffect(() => {
    let cancelled = false;
    load().then(
      (f) => {
        if (cancelled) return;
        setFactory(() => f);
        setLoadError(null);
      },
      (e: unknown) => {
        if (!cancelled) setLoadError(parseEngineError(e));
      },
    );
    return () => {
      cancelled = true;
    };
    // `load` is read once per attempt: "Recharger" bumps `attempt`.
  }, [attempt]);

  // Unmounting frees the running game. The controller itself is never created here.
  useEffect(
    () => () => {
      current.current?.dispose();
      current.current = null;
    },
    [],
  );

  const release = () => {
    current.current?.dispose();
    current.current = null;
    setRunning(null);
  };

  const begin = (mode: Mode, deckOneId: string, deckTwoId: string, seedText: string) => {
    release();
    const one = DECKS.find((d) => d.id === deckOneId);
    const two = DECKS.find((d) => d.id === deckTwoId);
    if (factory === null || one === undefined || two === undefined) return;
    // Synchronous: the engine is already loaded; `startGame` wraps whatever it throws.
    const createSession = (r: SessionRequest) =>
      createWasmSession(factory, {
        cards: CARDS_JSON,
        decks: DECKS_JSON,
        deckOne: r.deckOne,
        deckTwo: r.deckTwo,
        seed: r.seed,
        dataVersion: DATA_VERSION,
      });
    const result = startGame(
      { mode, deckOne: one, deckTwo: two, seedText },
      { createSession, policy: deps.policy, scheduler: deps.scheduler, now: deps.now },
      deps.crypto,
    );
    if (!result.ok) {
      setStartError(result);
      return;
    }
    setStartError(null);
    current.current = result.controller;
    counter.current += 1;
    setRunning({ key: counter.current, controller: result.controller });
    result.controller.start();
  };

  const startFromHome = (deckId: string, seedText: string) => {
    const other = DECKS.find((d) => d.id !== deckId);
    begin("computer", deckId, other?.id ?? deckId, seedText);
  };

  // "Replay" is built from the current snapshot: same decks and mode, the same seed or a new draw.
  const replay = (sameSeed: boolean) => {
    const snapshot = current.current?.getSnapshot();
    if (snapshot === undefined) return;
    begin(snapshot.mode, snapshot.players.One.id, snapshot.players.Two.id, sameSeed ? String(snapshot.seed) : "");
  };

  return (
    <main className="app">
      <h1>{fr.appTitle}</h1>
      {running === null ? (
        <Home
          decks={DECKS}
          dataVersion={DATA_VERSION}
          status={factory !== null ? "ready" : loadError !== null ? "failed" : "loading"}
          loadError={loadError}
          startError={startError}
          onStart={startFromHome}
          onReload={() => {
            setLoadError(null);
            setAttempt((n) => n + 1);
          }}
        />
      ) : (
        <GameScreen
          key={running.key}
          controller={running.controller}
          onReplaySameSeed={() => replay(true)}
          onReplayNewSeed={() => replay(false)}
          onHome={release}
        />
      )}
    </main>
  );
}
