import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "vitest";
import { webRoot } from "./support/paths";

const css = readFileSync(resolve(webRoot, "src/index.css"), "utf8");
const token = (name: string): string => {
  const m = css.match(new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})`));
  if (m === null) throw new Error(`token ${name} not found`);
  return m[1];
};
const lum = (hex: string): number => {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const ratio = (a: string, b: string): number => {
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

describe("the emblem chip contrast (WCAG 1.4.11, 3:1 for non-text)", () => {
  test("the chip body against both tones of the art it sits on, and the glyph against the chip body", () => {
    const fill = token("--emblem-fill");
    const glyph = token("--emblem-glyph");
    // The two stripe colours of the art placeholder (`.card-art`), literally as in the stylesheet.
    for (const art of ["#e4d8bd", "#f1e8d2"]) {
      expect(css).toContain(art);
      expect(ratio(fill, art), `fill vs ${art}`).toBeGreaterThanOrEqual(3);
    }
    expect(ratio(glyph, fill)).toBeGreaterThanOrEqual(3);
  });

  test("the chip ring against the frame colour that surrounds the art", () => {
    expect(ratio(token("--emblem-ring"), token("--card-border"))).toBeGreaterThanOrEqual(3);
  });

  test("the chip against any art background: for every luminance, the disc or the ring stands out by 3:1", () => {
    // The art is now an arbitrary image: whatever its luminance L (0..1), the dark disc or the light ring is at 3:1.
    const lumOf = (hex: string) => lum(hex);
    const ratioL = (a: number, b: number) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
    const disc = lumOf(token("--emblem-fill"));
    const ring = lumOf(token("--emblem-ring"));
    for (let i = 0; i <= 100; i++) {
      const L = i / 100;
      expect(Math.max(ratioL(disc, L), ratioL(ring, L)), `background luminance ${L}`).toBeGreaterThanOrEqual(3);
    }
  });
});
