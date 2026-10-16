import { Dialog } from "./Dialog";
import { fr } from "./fr";

interface AttackSheetProps {
  /** "{attacker} attaque {target}". */
  summary: string;
  /** What the attack will do, from the engine preview (`describeConsequence`): cost, hand-over, end of game. */
  lines: string[];
  onConfirm(): void;
  onCancel(): void;
}

/** The attack confirmation. "Annuler" comes first so it holds the focus: the safe answer. It announces no damage. */
export function AttackSheet({ summary, lines, onConfirm, onCancel }: AttackSheetProps) {
  return (
    <Dialog title={fr.attackTitle} onClose={onCancel}>
      <p>{summary}</p>
      {lines.map((l) => (
        <p key={l}>{l}</p>
      ))}
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={onCancel}>
          {fr.cancel}
        </button>
        <button type="button" className="btn btn-primary" onClick={onConfirm}>
          {fr.confirm}
        </button>
      </div>
    </Dialog>
  );
}
