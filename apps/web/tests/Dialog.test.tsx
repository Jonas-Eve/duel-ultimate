import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, test } from "vitest";
import { Dialog } from "../src/ui/Dialog";

afterEach(cleanup);

function Harness() {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button type="button" onClick={() => setOpen(true)}>
        Ouvrir
      </button>
      <button type="button">Autre bouton</button>
      {open && (
        <Dialog title="Titre du dialogue" onClose={() => setOpen(false)}>
          <p>Texte</p>
          <button type="button">Premier</button>
          <button type="button">Dernier</button>
        </Dialog>
      )}
    </div>
  );
}

async function open() {
  const user = userEvent.setup();
  render(<Harness />);
  const trigger = screen.getByRole("button", { name: "Ouvrir" });
  await user.click(trigger);
  return { user, trigger, dialog: screen.getByRole("dialog") };
}

describe("Dialog (reused as is by #65 and #66)", () => {
  test("is a modal dialog named by its title", async () => {
    const { dialog } = await open();
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    const labelledBy = dialog.getAttribute("aria-labelledby");
    expect(labelledBy).toBeTruthy();
    expect(document.getElementById(labelledBy as string)?.textContent).toBe("Titre du dialogue");
    expect(screen.getByRole("dialog", { name: "Titre du dialogue" })).toBe(dialog);
  });

  test("moves the focus inside on opening", async () => {
    const { dialog } = await open();
    expect(dialog.contains(document.activeElement)).toBe(true);
  });

  test("Tab wraps from the last control to the first, Shift+Tab from the first to the last", async () => {
    const { user, dialog } = await open();
    const first = within(dialog).getByRole("button", { name: "Premier" });
    const last = within(dialog).getByRole("button", { name: "Dernier" });
    first.focus();
    await user.tab({ shift: true });
    expect(document.activeElement).toBe(last);
    await user.tab();
    expect(document.activeElement).toBe(first);
    await user.tab();
    await user.tab();
    expect(document.activeElement).toBe(first);
  });

  test("the focus never leaves the dialog while Tab is pressed", async () => {
    const { user, dialog } = await open();
    for (let i = 0; i < 8; i++) {
      await user.tab();
      expect(dialog.contains(document.activeElement), `after ${i + 1} tabs`).toBe(true);
    }
  });

  test("Escape closes it and the focus returns to the trigger", async () => {
    const { user, trigger } = await open();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  test("closing by a button inside also returns the focus to the trigger", async () => {
    const user = userEvent.setup();
    function Closing() {
      const [o, setO] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setO(true)}>
            Ouvrir
          </button>
          {o && (
            <Dialog title="T" onClose={() => setO(false)}>
              <button type="button" onClick={() => setO(false)}>
                Fermer
              </button>
            </Dialog>
          )}
        </>
      );
    }
    render(<Closing />);
    const trigger = screen.getByRole("button", { name: "Ouvrir" });
    await user.click(trigger);
    await user.click(screen.getByRole("button", { name: "Fermer" }));
    expect(document.activeElement).toBe(trigger);
  });
});
