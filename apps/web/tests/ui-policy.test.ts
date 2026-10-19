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

  test("the three card forms keep the 62 x 112 face: the new classes never size the face", () => {
    const code = stripComments(css);
    const rule = (selector: string) => new RegExp(`${selector.replace(/\./g, "\\.")}\\s*\\{([^}]*)\\}`).exec(code)?.[1] ?? "";
    for (const cls of [".card-kind", ".card-value-empty"]) {
      expect(rule(cls), `${cls} is styled`).not.toBe("");
      expect(rule(cls), cls).not.toMatch(/(^|[;\s])(width|height|min-width|min-height|max-width|max-height)\s*:/);
    }
    expect(rule(".card-face")).toMatch(/height:\s*112px/);
    expect(rule(".card-face")).toMatch(/width:\s*var\(--card-w\)/);
  });

  test("the attack line stays a 44px target", () => {
    const code = stripComments(css);
    const rule = /\.card-detail-effect-button\s*\{([^}]*)\}/.exec(code)?.[1] ?? "";
    expect(rule).toMatch(/min-height:\s*44px/);
  });

  test("a polarity is carried by a border and a text mark, never by colour alone", () => {
    const code = stripComments(css);
    const rule = (selector: string) => new RegExp(`${selector.replace(/\./g, "\\.")}\\s*\\{([^}]*)\\}`).exec(code)?.[1] ?? "";
    for (const p of ["bonus", "malus", "neutral"]) expect(rule(`.effect-polarity-${p}`), p).toMatch(/border-inline-start:\s*\d+px\s+(solid|dashed|dotted)/);
    const styles = ["bonus", "malus", "neutral"].map((p) => /border-inline-start:\s*\d+px\s+(solid|dashed|dotted)/.exec(rule(`.effect-polarity-${p}`))![1]);
    expect(new Set(styles).size).toBe(3);
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

  test("art images fill their slot and crop identically (cover), without z-index", () => {
    const code = stripComments(css);
    const rule = /\.card-art img,\s*\.card-detail-art img\s*\{([^}]*)\}/.exec(code)?.[1] ?? "";
    expect(rule).toMatch(/width:\s*100%/);
    expect(rule).toMatch(/height:\s*100%/);
    expect(rule).toMatch(/object-fit:\s*cover/);
    expect(rule).toMatch(/display:\s*block/);
    expect(code).not.toMatch(/z-index/);
  });
});

describe("vite.config.ts", () => {
  test("SVG files are never inlined as data: URIs (they stay separate requests, invisible otherwise to the network tests)", () => {
    const config = readFileSync(join(webRoot, "vite.config.ts"), "utf8");
    expect(config).toMatch(/assetsInlineLimit:\s*\(\w+\)\s*=>\s*\(\w+\.endsWith\("\.svg"\)\s*\?\s*false\s*:\s*undefined\)/);
  });
});
