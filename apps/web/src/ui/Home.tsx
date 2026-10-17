import { useEffect, useId, useRef, useState } from "react";
import type { EngineError } from "../engine/errors";
import type { DeckRef, Mode } from "../game/controller";
import type { StartResult } from "../game/start";
import { messageFor } from "./errors.fr";
import { fr } from "./fr";

/** A failed `startGame`, as `App` keeps it: a new object per failure so that the screen reacts to each one. */
export type StartFailure = Extract<StartResult, { ok: false }>;

interface HomeProps {
  /** The closed list of decks and the data version come from `App` (this layer never reads the data). */
  decks: DeckRef[];
  dataVersion: string;
  /** Short commit SHA of the deployed build, or `dev` (read by `App`, like the data version). */
  buildId: string;
  status: "loading" | "ready" | "failed";
  loadError: EngineError | null;
  startError: StartFailure | null;
  /** Against the computer `deckTwo` is the opponent's deck (the other one); in two camps it is camp 2's. */
  onStart(start: { mode: Mode; deckOne: string; deckTwo: string; seedText: string }): void;
  onReload(): void;
}

function DeckGroup({ name, legend, decks, value, onChange }: { name: string; legend: string; decks: DeckRef[]; value: string; onChange(id: string): void }) {
  return (
    <fieldset className="decks">
      <legend>{legend}</legend>
      {decks.map((d) => (
        <label key={d.id} className="deck-choice">
          <input type="radio" name={name} value={d.id} checked={d.id === value} onChange={() => onChange(d.id)} />
          {d.name}
        </label>
      ))}
    </fieldset>
  );
}

export function Home({ decks, dataVersion, buildId, status, loadError, startError, onStart, onReload }: HomeProps) {
  const [mode, setMode] = useState<Mode>("computer");
  const [deckId, setDeckId] = useState(decks[0]?.id ?? "");
  // Camp 2's deck follows "the other deck" until the player picks one (the mirror is allowed).
  const [chosenTwo, setChosenTwo] = useState<string | null>(null);
  const [seedText, setSeedText] = useState("");
  const seed = useRef<HTMLInputElement>(null);
  const ids = useId();
  const helpId = `${ids}-help`;
  const errorId = `${ids}-error`;

  // A seed problem sends the focus back to the seed field, where the fix is.
  useEffect(() => {
    if (startError !== null && startError.reason !== "engine") seed.current?.focus();
  }, [startError]);

  const opponent = decks.find((d) => d.id !== deckId);
  const deckTwoId = chosenTwo ?? opponent?.id ?? deckId;
  const seedMessage =
    startError === null || startError.reason === "engine"
      ? null
      : startError.reason === "entropy_unavailable"
        ? fr.entropyUnavailable
        : messageFor("invalid_seed");

  return (
    <section className="screen home" aria-labelledby={`${ids}-title`}>
      <h2 id={`${ids}-title`}>{fr.homeHeading}</h2>

      {status === "loading" && <p role="status">{fr.loadingEngine}</p>}
      {status === "failed" && (
        <div role="alert" className="alert">
          <p>{messageFor(loadError?.code ?? "wasm_load_failed")}</p>
          <button type="button" className="btn" onClick={onReload}>
            {fr.reload}
          </button>
        </div>
      )}
      {startError?.reason === "engine" && (
        <p role="alert" className="alert">
          {messageFor(startError.error?.code ?? "internal_error")}
        </p>
      )}

      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (status === "ready") onStart({ mode, deckOne: deckId, deckTwo: mode === "computer" ? (opponent?.id ?? deckId) : deckTwoId, seedText });
        }}
      >
        <fieldset className="decks">
          <legend>{fr.modeLegend}</legend>
          {(
            [
              ["computer", fr.modeComputer],
              ["two-camps", fr.modeTwoCamps],
            ] as const
          ).map(([value, text]) => (
            <label key={value} className="deck-choice">
              <input type="radio" name="mode" value={value} checked={mode === value} onChange={() => setMode(value)} />
              {text}
            </label>
          ))}
        </fieldset>
        <DeckGroup name="deck" legend={mode === "computer" ? fr.deckLegend : fr.campDeckLegend(1)} decks={decks} value={deckId} onChange={setDeckId} />
        {mode === "computer" ? (
          opponent !== undefined && <p>{fr.opponentDeck(opponent.name)}</p>
        ) : (
          <DeckGroup name="deck-two" legend={fr.campDeckLegend(2)} decks={decks} value={deckTwoId} onChange={setChosenTwo} />
        )}

        <label htmlFor={`${ids}-seed`} className="field-label">
          {fr.seedLabel}
        </label>
        <input
          id={`${ids}-seed`}
          ref={seed}
          type="text"
          inputMode="numeric"
          autoComplete="off"
          className="field"
          value={seedText}
          aria-invalid={seedMessage !== null}
          aria-describedby={seedMessage === null ? helpId : `${helpId} ${errorId}`}
          onChange={(e) => setSeedText(e.target.value)}
        />
        <p id={helpId} className="help">
          {fr.seedHelp}
        </p>
        {seedMessage !== null && (
          <p id={errorId} role="alert" className="alert">
            {seedMessage}
          </p>
        )}

        <button type="submit" className="btn btn-primary" disabled={status !== "ready"}>
          {fr.start}
        </button>
      </form>
      <p className="help">{fr.dataVersion(dataVersion)}</p>
      <p className="help">{fr.buildVersion(buildId)}</p>
    </section>
  );
}
