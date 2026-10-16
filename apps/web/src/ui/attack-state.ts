// The attack flow's derived state: pure, no React. Nothing here knows a rule (taunt, cost, who
// can be hit): the targets are read from the engine's previews, and a selection that no preview
// backs any more simply derives to `null`.
import type { ActionPreview, Target } from "../engine/types";
import { isAttackBy, isAttackOn } from "../engine/types";

export interface AttackState {
  /** The selected attacker, or null when nothing is selected or the selection is stale. */
  selected: number | null;
  /** The engine previews of the selected attacker's attacks (the very objects of `legal`). */
  attacks: ActionPreview[];
  heroTargetable: boolean;
  creatureTargets: ReadonlySet<number>;
}

const NOTHING: AttackState = { selected: null, attacks: [], heroTargetable: false, creatureTargets: new Set() };

/** `legal` is `snapshot.legal` (empty outside a human turn). The guards apply to `p.action`, not to the preview. */
export function deriveAttackState(selected: number | null, legal: readonly ActionPreview[]): AttackState {
  if (selected === null) return NOTHING;
  const attacks = legal.filter((p) => isAttackBy(selected)(p.action));
  if (attacks.length === 0) return NOTHING;
  const creatures = new Set<number>();
  for (const p of attacks) {
    if (isAttackBy(selected)(p.action) && p.action.attack.target !== "hero") creatures.add(p.action.attack.target.creature);
  }
  return {
    selected,
    attacks,
    heroTargetable: attacks.some((p) => isAttackOn("hero")(p.action)),
    creatureTargets: creatures,
  };
}

/** The preview of an attack on `target`, as the engine listed it; undefined when it did not. */
export function previewOn(attacks: readonly ActionPreview[], target: Target): ActionPreview | undefined {
  return attacks.find((p) => isAttackOn(target)(p.action));
}
