import { Dialog } from "./Dialog";
import { fr } from "./fr";
import type { LogBlock } from "./log-format";
import { LogHistory } from "./LogHistory";

/** The full history in a dialog titled "Journal". Not a live region: the last line has its own, in `LogBar`. */
export function LogDrawer({ blocks, onClose }: { blocks: LogBlock[]; onClose(): void }) {
  return (
    <Dialog title={fr.logTitle} onClose={onClose}>
      <LogHistory blocks={blocks} level={3} />
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={onClose}>
          {fr.close}
        </button>
      </div>
    </Dialog>
  );
}
