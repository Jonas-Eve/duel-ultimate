// @vitest-environment node
// #80: a real `vite build` under a sub-path (`VITE_BASE`) puts the page, the assets and the
// `.wasm` under it, and shows the build identifier; without variables the output stays at the
// root. Same model as `csp.test.ts`: temporary folders, cleaned up afterwards.
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { build } from "vite";
import { afterAll, beforeAll, describe, expect, test } from "vitest";
import { webRoot } from "./support/paths";

interface Output {
  html: string;
  assets: string[];
  js: string;
}

const dirs: string[] = [];

/** Builds with the given variables, then restores `process.env` whatever happens. */
async function buildWith(vars: Record<string, string>): Promise<Output> {
  const outDir = mkdtempSync(join(tmpdir(), "duel-base-"));
  dirs.push(outDir);
  const saved = { VITE_BASE: process.env.VITE_BASE, VITE_BUILD_SHA: process.env.VITE_BUILD_SHA };
  delete process.env.VITE_BASE;
  delete process.env.VITE_BUILD_SHA;
  Object.assign(process.env, vars);
  try {
    await build({ root: webRoot, logLevel: "silent", build: { outDir, emptyOutDir: true } });
  } finally {
    for (const [k, v] of Object.entries(saved)) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
  }
  const assets = readdirSync(join(outDir, "assets"));
  const js = assets
    .filter((a) => a.endsWith(".js"))
    .map((a) => readFileSync(join(outDir, "assets", a), "utf8"))
    .join("\n");
  return { html: readFileSync(join(outDir, "index.html"), "utf8"), assets, js };
}

/** Every local `src=` / `href=` of the page (not `data:`, not an absolute URL). */
function localRefs(html: string): string[] {
  return [...html.matchAll(/\b(?:src|href)="([^"]*)"/g)].map((m) => m[1]).filter((u) => !u.startsWith("data:") && !/^[a-z][a-z0-9+.-]*:/i.test(u));
}

let sub: Output;
let root: Output;

beforeAll(async () => {
  sub = await buildWith({ VITE_BASE: "/duel-ultimate/", VITE_BUILD_SHA: "abc1234def" });
  root = await buildWith({});
}, 240_000);

afterAll(() => {
  for (const d of dirs) rmSync(d, { recursive: true, force: true });
});

describe("build under VITE_BASE=/duel-ultimate/", () => {
  test("every local reference of the page is under the base", () => {
    const refs = localRefs(sub.html);
    expect(refs.length).toBeGreaterThan(0);
    for (const r of refs) expect(r.startsWith("/duel-ultimate/"), r).toBe(true);
  });

  test("the .wasm is emitted and referenced under the base, never from /assets/", () => {
    const wasm = sub.assets.filter((a) => a.endsWith(".wasm"));
    expect(wasm).toHaveLength(1);
    expect(sub.js).toContain(`/duel-ultimate/assets/${wasm[0]}`);
    expect(sub.js).not.toContain('"/assets/');
  });

  test("the build identifier is in the bundle, cut at 7 characters", () => {
    expect(sub.js).toContain("abc1234");
    expect(sub.js).not.toContain("abc1234d");
  });
});

describe("build without variables", () => {
  test("references stay at the root, the .wasm under /assets/", () => {
    const refs = localRefs(root.html);
    expect(refs.length).toBeGreaterThan(0);
    for (const r of refs) expect(r.startsWith("/assets/"), r).toBe(true);
    const wasm = root.assets.filter((a) => a.endsWith(".wasm"));
    expect(wasm).toHaveLength(1);
    expect(root.js).toContain(`/assets/${wasm[0]}`);
    expect(root.js).not.toContain("/duel-ultimate/");
  });

  test("the bundle carries no build SHA", () => {
    expect(root.js).not.toContain("abc1234");
  });
});

describe("invalid VITE_BASE", () => {
  test("fails the build with the explicit error, and process.env is restored", async () => {
    const before = process.env.VITE_BASE;
    await expect(buildWith({ VITE_BASE: "./" })).rejects.toThrow(/VITE_BASE/);
    expect(process.env.VITE_BASE).toBe(before);
  }, 60_000);
});
