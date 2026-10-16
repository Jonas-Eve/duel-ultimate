import { describe, expect, test } from "vitest";
import { sourceFiles, stripComments } from "./support/source-scan";

// Repo rule (CLAUDE.md, #64): the `GameController` is created in an event handler, never in an
// effect. StrictMode replays mount effects, and a cleanup that frees the controller would kill the
// instance of the second mount. The StrictMode mount test (app-flow.test.tsx) covers the cleanup
// that frees; this source scan covers the creation itself.

/** Bodies of every `useEffect(...)` / `useLayoutEffect(...)` call, by balancing parentheses. */
function effectBodies(code: string): string[] {
  const bodies: string[] = [];
  for (const m of code.matchAll(/\buse(?:Layout)?Effect\s*\(/g)) {
    const open = (m.index ?? 0) + m[0].length;
    let depth = 1;
    let i = open;
    while (i < code.length && depth > 0) {
      if (code[i] === "(") depth++;
      else if (code[i] === ")") depth--;
      i++;
    }
    bodies.push(code.slice(open, i - 1));
  }
  return bodies;
}

const CREATES_CONTROLLER = /\bstartGame\s*\(|\bGameController\s*\.\s*create\s*\(/;

function violations(code: string): number {
  return effectBodies(stripComments(code)).filter((body) => CREATES_CONTROLLER.test(body)).length;
}

const files = sourceFiles().filter((f) => f.rel === "App.tsx" || (f.rel.startsWith("ui/") && f.rel.endsWith(".tsx")));

describe("the controller is never created in an effect", () => {
  test("the scan covers App and the components, and they do use effects", () => {
    expect(files.some((f) => f.rel === "App.tsx")).toBe(true);
    expect(files.some((f) => f.rel === "ui/GameScreen.tsx")).toBe(true);
    expect(files.flatMap((f) => effectBodies(stripComments(f.text))).length).toBeGreaterThan(3);
  });

  test("no startGame( / GameController.create( inside useEffect / useLayoutEffect", () => {
    for (const f of files) {
      expect(violations(f.text), `${f.rel} creates the controller inside an effect`).toBe(0);
    }
  });

  test("the scan sees a violation", () => {
    const inEffect = "useEffect(() => { const r = startGame(req, deps); keep(r); }, [req]);";
    expect(violations(inEffect)).toBe(1);
    expect(violations("React.useLayoutEffect(() => { GameController.create(c, d); });")).toBe(1);
    expect(violations("useEffect(() => { setup(() => { startGame(a, b); }); return () => {}; }, []);")).toBe(1);
  });

  test("the scan lets the handler pattern through", () => {
    const handler = "function onClick() { const r = startGame(req, deps); } useEffect(() => { focus(); }, []);";
    expect(violations(handler)).toBe(0);
    expect(violations("useEffect(() => { /* startGame( is created in the handler */ }, []);")).toBe(0);
  });
});
