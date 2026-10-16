import { fr } from "./fr";
import type { LogLine } from "./log-format";

interface LogBarProps {
  /** The last line of the log: always shown, whether the drawer is open or not. */
  line: LogLine | undefined;
  /** False during the computer's chain (and while its summary is the announcement): bursts are not read out. */
  live: boolean;
}

/** The last line of the log, in a log region. Polite when live, `off` otherwise; the drawer is never a live region. */
export function LogBar({ line, live }: LogBarProps) {
  return (
    <div role="log" aria-live={live ? "polite" : "off"} aria-atomic="false" aria-label={fr.logLast} className="log-bar">
      {line !== undefined && (
        <p key={line.id} className={`log-line log-${line.tone}`}>
          {line.text}
        </p>
      )}
    </div>
  );
}
