import { fr } from "./fr";
import type { LogBlock } from "./log-format";

/**
 * The whole history as titled blocks of lines, in a plain (not live) list: used by the drawer
 * and by the end screen. `level` is the heading level of the block titles.
 */
export function LogHistory({ blocks, level }: { blocks: LogBlock[]; level: 3 | 4 }) {
  const Heading = level === 3 ? "h3" : "h4";
  if (blocks.length === 0) return <p>{fr.logEmpty}</p>;
  return (
    <div className="log-history">
      {blocks.map((block, i) => (
        <section key={`${block.key}-${i}`} className="log-block" aria-labelledby={`log-block-${level}-${i}`}>
          <Heading id={`log-block-${level}-${i}`} className="log-block-title">
            {block.title}
          </Heading>
          <ol className="log-lines">
            {block.lines.map((line) => (
              <li key={line.id} className={`log-line log-${line.tone}`}>
                {line.text}
              </li>
            ))}
          </ol>
        </section>
      ))}
    </div>
  );
}
