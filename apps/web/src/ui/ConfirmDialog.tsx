import { Dialog } from "./Dialog";
import { fr } from "./fr";

interface ConfirmDialogProps {
  /** What conceding will do, from the engine preview (`describeConsequence`). */
  lines: string[];
  onConfirm(): void;
  onCancel(): void;
  /** Two camps: "Le camp N abandonne ?"; defaults to the single-player title. */
  title?: string;
}

/** Concession confirmation. "Continuer" comes first so it holds the focus: the safe answer. */
export function ConfirmDialog({ lines, onConfirm, onCancel, title = fr.concedeTitle }: ConfirmDialogProps) {
  return (
    <Dialog title={title} onClose={onCancel}>
      {lines.map((l) => (
        <p key={l}>{l}</p>
      ))}
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={onCancel}>
          {fr.keepPlaying}
        </button>
        <button type="button" className="btn btn-danger" onClick={onConfirm}>
          {fr.concede}
        </button>
      </div>
    </Dialog>
  );
}
