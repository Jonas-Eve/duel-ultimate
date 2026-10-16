// @vitest-environment node
// Decision S3 of #64: a Content-Security-Policy in the BUILD only (the dev server injects an
// inline script, so no CSP there). The build is run for real, into a temporary folder.
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { build } from "vite";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { cspPlugin } from "../vite/csp-plugin";
import { webRoot } from "./support/paths";

let outDir = "";
let html = "";

beforeAll(async () => {
  outDir = mkdtempSync(join(tmpdir(), "duel-csp-"));
  await build({ root: webRoot, logLevel: "silent", build: { outDir, emptyOutDir: true } });
  html = readFileSync(join(outDir, "index.html"), "utf8");
}, 120_000);

afterAll(() => {
  if (outDir) rmSync(outDir, { recursive: true, force: true });
});

function policy(): Record<string, string[]> {
  const m = /<meta[^>]*http-equiv="Content-Security-Policy"[^>]*content="([^"]*)"/i.exec(html);
  expect(m, "the CSP meta tag is missing from dist/index.html").not.toBeNull();
  const out: Record<string, string[]> = {};
  // The attribute is HTML-escaped by the build ("&#39;" for a quote).
  const content = (m as RegExpExecArray)[1].replaceAll("&#39;", "'").replaceAll("&quot;", '"').replaceAll("&amp;", "&");
  for (const part of content.split(";")) {
    const [name, ...values] = part.trim().split(/\s+/);
    if (name) out[name] = values;
  }
  return out;
}

describe("CSP of the production build", () => {
  test("the meta tag is there, first in the head", () => {
    policy();
    const head = html.slice(html.indexOf("<head>"));
    expect(head.indexOf("Content-Security-Policy")).toBeLessThan(head.indexOf("<script"));
  });

  test("default-src 'none', scripts from 'self' plus wasm-unsafe-eval", () => {
    const p = policy();
    expect(p["default-src"]).toEqual(["'none'"]);
    expect(p["script-src"]).toEqual(["'self'", "'wasm-unsafe-eval'"]);
    expect(p["style-src"]).toEqual(["'self'"]);
    expect(p["connect-src"]).toEqual(["'self'"]);
    expect(p["base-uri"]).toEqual(["'none'"]);
    expect(p["form-action"]).toEqual(["'none'"]);
  });

  test("neither unsafe-inline nor unsafe-eval anywhere", () => {
    expect(html).not.toMatch(/unsafe-inline/);
    expect(html).not.toMatch(/'unsafe-eval'/);
  });

  test("no inline script and no inline style in the built page", () => {
    const scripts = [...html.matchAll(/<script\b([^>]*)>/gi)];
    expect(scripts.length).toBeGreaterThan(0);
    for (const s of scripts) expect(s[1], `inline script: ${s[0]}`).toMatch(/\bsrc=/);
    expect(html).not.toMatch(/<style\b/i);
    expect(html).not.toMatch(/\sstyle=/i);
  });

  test("the plugin only applies to the build (the dev server has inline scripts)", () => {
    expect(cspPlugin().apply).toBe("build");
  });
});
