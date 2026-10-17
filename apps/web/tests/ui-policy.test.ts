import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "vitest";
import { webRoot } from "./support/paths";
import { sourceFiles, stripComments } from "./support/source-scan";

// The build CSP forbids inline styles (S3 of #64): classes only. And the accessibility rules
// of the story that are checkable as text.

const css = readFileSync(join(webRoot, "src/index.css"), "utf8");
const components = sourceFiles().filter((f) => f.rel.endsWith(".tsx"));

describe("components: classes only", () => {
  test("components exist", () => {
    expect(components.some((f) => f.rel === "ui/GameScreen.tsx")).toBe(true);
    expect(components.length).toBeGreaterThan(8);
  });

  test("no style= attribute, no <style> element, no CSSOM style write", () => {
    for (const f of components) {
      const code = stripComments(f.text);
      expect(/\sstyle\s*=/.test(code), `${f.rel} has a style= attribute`).toBe(false);
      expect(/<style\b/.test(code), `${f.rel} has a <style> element`).toBe(false);
      expect(/\.style\./.test(code) || /\.style\s*=/.test(code), `${f.rel} writes element.style`).toBe(false);
    }
  });

  test("the scan sees a violation", () => {
    expect(/\sstyle\s*=/.test(stripComments('<p style={{ color: "red" }} />'))).toBe(true);
  });
});

describe("index.css", () => {
  test("tap target token is 44px and used", () => {
    expect(css).toMatch(/--tap:\s*44px/);
    expect(css).toMatch(/var\(--tap\)/);
  });

  test("no :hover rule at all (never the only carrier of information)", () => {
    expect(stripComments(css)).not.toMatch(/:hover/);
  });

  test("a compact card face is a fixed box, its three values are at least 10px (in px, not rem)", () => {
    const code = stripComments(css);
    expect(code).toMatch(/--card-w:\s*62px/);
    const face = /\.card-face\s*\{([^}]*)\}/.exec(code)?.[1] ?? "";
    expect(face).toMatch(/width:\s*var\(--card-w\)/);
    expect(face).toMatch(/height:\s*112px/);
    expect(face).toMatch(/box-sizing:\s*border-box/);
    const value = /\.card-value\s*\{([^}]*)\}/.exec(code)?.[1] ?? "";
    const size = /font-size:\s*(\d+(?:\.\d+)?)px/.exec(value);
    expect(size, ".card-value font-size in px").not.toBeNull();
    expect(Number(size![1])).toBeGreaterThanOrEqual(10);
  });

  test("the card detail sheet: a 520px card, a 230px art slot, a scrolling effects zone, a 48px close button, a screen-reader-only class", () => {
    const code = stripComments(css);
    const rule = (selector: string) => new RegExp(`${selector.replace(/\./g, "\\.")}\\s*\\{([^}]*)\\}`).exec(code)?.[1] ?? "";
    expect(rule(".card-detail")).toMatch(/height:\s*520px/);
    expect(rule(".card-detail-art")).toMatch(/height:\s*230px/);
    expect(rule(".card-detail-effects")).toMatch(/overflow:\s*auto/);
    expect(rule(".card-detail-close")).toMatch(/min-height:\s*48px/);
    expect(rule(".sr-only")).toMatch(/position:\s*absolute/);
  });

  test("the page column is centred and at most 48rem wide; card rows scroll inside", () => {
    expect(css).toMatch(/max-width:\s*48rem/);
    expect(css).toMatch(/overflow-x:\s*auto/);
  });
});
