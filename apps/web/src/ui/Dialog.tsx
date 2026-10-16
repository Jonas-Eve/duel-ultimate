import { useEffect, useId, useRef, type KeyboardEvent, type ReactNode } from "react";

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

interface DialogProps {
  title: string;
  /** Escape asks for it; the caller unmounts the dialog. */
  onClose(): void;
  children: ReactNode;
}

/**
 * Generic modal dialog (reused as is by the log and the attack confirmation): `role="dialog"`,
 * `aria-modal`, named by its title; focus moves inside on opening, Tab wraps, Escape closes, and
 * the focus returns to whatever held it before. No `<dialog>.showModal()` (absent from jsdom) and
 * no `inert`: the trap is explicit.
 */
export function Dialog({ title, onClose, children }: DialogProps) {
  const titleId = useId();
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const first = box.current?.querySelector<HTMLElement>(FOCUSABLE);
    (first ?? box.current)?.focus();
    return () => opener?.focus();
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
    if (e.shiftKey && (active === first || !box.current.contains(active))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (active === last || !box.current.contains(active))) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <div className="dialog-backdrop">
      <div ref={box} role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1} className="dialog" onKeyDown={onKeyDown}>
        <h2 id={titleId} className="dialog-title">
          {title}
        </h2>
        {children}
      </div>
    </div>
  );
}
