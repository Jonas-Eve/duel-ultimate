import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode } from "react";

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

interface DialogProps {
  title: string;
  /** Escape asks for it; the caller unmounts the dialog. */
  onClose(): void;
  children: ReactNode;
  /** The title still names the dialog but is visually hidden (the content shows the name itself). */
  hideTitle?: boolean;
  /** "first" (default): the first control. "dialog": the container, so a screen reader reads the whole sheet. */
  initialFocus?: "first" | "dialog";
  /** Ids of the elements that describe the dialog (`aria-describedby`). */
  describedBy?: string;
}

/**
 * Generic modal dialog (reused as is by the log and the attack confirmation): `role="dialog"`,
 * `aria-modal`, named by its title; focus moves inside on opening, Tab wraps, Escape closes, and
 * the focus returns to whatever held it before. No `<dialog>.showModal()` (absent from jsdom) and
 * no `inert`: the trap is explicit.
 */
export function Dialog({ title, onClose, children, hideTitle = false, initialFocus = "first", describedBy }: DialogProps) {
  const titleId = useId();
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const first = initialFocus === "first" ? box.current?.querySelector<HTMLElement>(FOCUSABLE) : null;
    (first ?? box.current)?.focus();
    return () => opener?.focus();
    // Read once, on opening: the option is not meant to change while the dialog is open.
  }, []);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== "Tab" || box.current === null) return;
    const items = [...box.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
    if (items.length === 0) {
      e.preventDefault();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement;
    // The container itself holds the focus with `initialFocus="dialog"`: Shift+Tab from it goes to the last control too.
    if (e.shiftKey && (active === first || active === box.current || !box.current.contains(active))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (active === last || !box.current.contains(active))) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <div className="dialog-backdrop">
      <div ref={box} role="dialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={describedBy} tabIndex={-1} className="dialog" onKeyDown={onKeyDown}>
        <h2 id={titleId} className={hideTitle ? "dialog-title sr-only" : "dialog-title"}>
          {title}
        </h2>
        {children}
      </div>
    </div>
  );
}
