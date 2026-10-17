// Playing a hand card the way the interface does it since #105: a touch opens the detail, "Jouer" plays.
import { screen, within } from "@testing-library/react";
import type userEvent from "@testing-library/user-event";

/** The detail sheet's play button (named "Jouer (N temps)", or just "Jouer" when the engine offers no preview). */
export const playButton = () => within(screen.getByRole("dialog")).getByRole("button", { name: /^Jouer/ });

/** Opens the tile's detail and presses "Jouer". */
export async function playTile(user: ReturnType<typeof userEvent.setup>, tile: HTMLElement): Promise<void> {
  await user.click(tile);
  await user.click(playButton());
}

/** The effect line "Attaque (N) : inflige D dégâts" of an open detail: the attack's button (#106). */
export const attackLine = () => within(screen.getByRole("dialog")).getByRole("button", { name: /^Attaque \(/ });

/** Opens the detail of one of my board tiles and presses its attack line: the attacker is selected. */
export async function selectAttacker(user: ReturnType<typeof userEvent.setup>, tile: HTMLElement): Promise<void> {
  await user.click(tile);
  await user.click(attackLine());
}
