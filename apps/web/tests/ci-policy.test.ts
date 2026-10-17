import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "vitest";
import { repoRoot, webRoot } from "./support/paths";

// Guards the CI and build-chain decisions of story #51 by reading the files as text
// (no YAML dependency): a regression here is a policy regression, not a style one.

const read = (p: string) => readFileSync(p, "utf8");
const ci = read(resolve(repoRoot, ".github/workflows/ci.yml"));
const pkg = JSON.parse(read(resolve(webRoot, "package.json")));
const code = ci.split("\n").filter((l) => !/^\s*#/.test(l)).join("\n");

const e2e = read(resolve(repoRoot, ".github/workflows/e2e.yml"));
const stripComments = (t: string) => t.split("\n").filter((l) => !/^\s*#/.test(l)).join("\n");
const e2eCode = stripComments(e2e);
const stepsOf = (c: string) => c.split(/\n(?=\s*- name:)/).slice(1);
const usesOf = (c: string) => [...c.matchAll(/^\s*-?\s*uses:\s*(\S+)/gm)].map((m) => m[1]);
// The `on:` block alone: a trigger name elsewhere (a variable, a step name) must not count.
const onBlock = (c: string) => c.match(/^on:\s*\n((?:[ \t]+.*\n|[ \t]*\n)*)/m)?.[1] ?? "";

// Rules shared by every workflow of the repo; ci.yml must not be weakened by e2e.yml existing.
describe.each([
  ["ci.yml", code, 5],
  ["e2e.yml", e2eCode, 1],
] as const)(".github/workflows/%s, common security rules", (_name, wf, minCargo) => {
  test("least privilege, no secrets, no artifact", () => {
    expect(wf).toMatch(/^permissions:\s*\n\s+contents: read\s*\n(?!\s+\S+:)/m);
    expect(wf.match(/permissions:/g)).toHaveLength(1);
    expect(wf).not.toMatch(/secrets\./);
    expect(wf).not.toContain("upload-artifact");
    expect(wf).toContain("persist-credentials: false");
    expect(wf).toMatch(/timeout-minutes: \d+/);
    expect(wf).toMatch(/runs-on: ubuntu-24\.04/);
  });

  test("every action is first-party and pinned to a full SHA", () => {
    const uses = usesOf(wf);
    expect(uses.length).toBeGreaterThan(0);
    for (const u of uses) expect(u).toMatch(/^actions\/(checkout|setup-node)@[0-9a-f]{40}$/);
  });

  test("never interpolates a GitHub context inside a run script", () => {
    const runs = stepsOf(wf).filter((s) => /\brun:/.test(s));
    for (const r of runs) expect(r).not.toContain("${{");
  });

  test("setup-node reads an existing node-version file", () => {
    const m = wf.match(/node-version-file:\s*(\S+)/);
    expect(m).not.toBeNull();
    expect(existsSync(resolve(repoRoot, m![1]))).toBe(true);
  });

  test("every cargo build/clippy/test/install is --locked", () => {
    const cmds = wf.split("\n").filter((l) => /\bcargo (build|clippy|test|install|tree)\b/.test(l));
    expect(cmds.length).toBeGreaterThanOrEqual(minCargo);
    for (const c of cmds) expect(c).toContain("--locked");
  });

  test("installs the CLI at the version read from the lock by the build script", () => {
    expect(wf).toContain("build-wasm.mjs --print-version");
  });
});

describe(".github/workflows/ci.yml", () => {
  test("runs on pull_request and push to main only", () => {
    expect(code).toMatch(/^on:\s*\n\s+pull_request:\s*\n\s+push:\s*\n\s+branches: \[main\]/m);
    for (const bad of ["pull_request_target", "workflow_run", "issue_comment", "workflow_dispatch"]) {
      expect(code).not.toContain(bad);
    }
  });

  test("steps run in the specified order", () => {
    const order = [
      "actions/checkout",
      "actions/setup-node",
      "rustup toolchain install",
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --locked -- -D warnings",
      "cargo test --workspace --locked",
      "cargo build --locked --target wasm32-unknown-unknown -p duel-engine-wasm --release",
      "getrandom",
      "cargo install wasm-bindgen-cli",
      "npm ci",
      "npm run build",
      "npm test",
    ];
    let at = -1;
    for (const s of order) {
      const i = code.indexOf(s, at + 1);
      expect(i, `"${s}" missing or out of order`).toBeGreaterThan(at);
      at = i;
    }
  });

  test("has the non-entropy check on the wasm dependency tree", () => {
    expect(code).toContain("cargo tree -p duel-engine-wasm --target wasm32-unknown-unknown -e normal --locked --prefix none");
    expect(code).toMatch(/grep -qE '\^getrandom '/);
  });

  test("never runs the e2e tests (#78: on demand only, see e2e.yml)", () => {
    expect(code).not.toMatch(/playwright/i);
    expect(code).not.toContain("npm run e2e");
  });
});

describe(".github/workflows/e2e.yml (manual, #78)", () => {
  test("is triggered by workflow_dispatch alone, without inputs: never by itself", () => {
    const on = onBlock(e2eCode);
    expect(on.trim(), "the on: block was not found").not.toBe("");
    expect(on.split("\n").filter((l) => /^\s*\S/.test(l)).map((l) => l.trim())).toEqual(["workflow_dispatch:"]);
    for (const bad of [
      "pull_request", "pull_request_target", "push", "schedule", "workflow_run",
      "workflow_call", "repository_dispatch", "issue_comment", "inputs",
    ]) {
      expect(on, `"${bad}" must not trigger e2e.yml`).not.toContain(bad);
    }
  });

  test("the on: guard looks at the on: block only", () => {
    expect(onBlock("name: x\non:\n  workflow_dispatch:\n\njobs:\n  push:\n")).not.toContain("push");
  });

  test("uses exactly the actions (same SHAs) of ci.yml", () => {
    const ours = usesOf(e2eCode);
    const theirs = usesOf(code);
    expect(ours.length).toBeGreaterThan(0);
    for (const u of ours) {
      expect(u).toMatch(/^actions\/(checkout|setup-node)@[0-9a-f]{40}$/);
      expect(theirs, `${u} is not the version pinned in ci.yml`).toContain(u);
    }
  });

  test("steps run in the specified order", () => {
    const order = [
      "actions/checkout",
      "actions/setup-node",
      "rustup toolchain install",
      "cargo install wasm-bindgen-cli",
      "npm ci",
      "npx playwright install --with-deps chromium",
      "npm run e2e",
    ];
    let at = -1;
    for (const s of order) {
      const i = e2eCode.indexOf(s, at + 1);
      expect(i, `"${s}" missing or out of order`).toBeGreaterThan(at);
      at = i;
    }
  });

  test("the e2e steps exist, run in apps/web and interpolate nothing", () => {
    const steps = stepsOf(e2eCode);
    for (const run of ["npx playwright install --with-deps chromium", "npm run e2e"]) {
      const step = steps.filter((s) => new RegExp(`^\\s*run: ${run}\\s*$`, "m").test(s));
      expect(step, `exactly one step runs "${run}"`).toHaveLength(1);
      expect(step[0]).not.toContain("${{");
      expect(step[0]).toMatch(/working-directory: apps\/web/);
    }
  });

  test("Playwright is installed by the local CLI: no version, no remote package, no extra action", () => {
    expect(e2eCode).not.toMatch(/playwright@/);
    expect(e2eCode).not.toMatch(/npx (?:-y |--yes )/);
    expect(e2eCode).not.toMatch(/npx (?:-y |--yes )?@playwright/);
    expect(e2eCode).not.toMatch(/microsoft\/playwright|upload-artifact/);
  });
});

describe("end-to-end tests (Playwright)", () => {
  const deps = { ...pkg.dependencies, ...pkg.devDependencies } as Record<string, string>;
  const config = read(resolve(webRoot, "playwright.config.ts"));

  test("the scripts build first, then run Playwright", () => {
    expect(pkg.scripts.pree2e).toBe("npm run build");
    expect(pkg.scripts.e2e).toBe("playwright test");
  });

  test("versions are exact; @playwright/test is 1.56.1 (the Chromium 141 of the agents' environment)", () => {
    expect(deps["@playwright/test"]).toBe("1.56.1");
    expect(deps["playwright-core"]).toBe("1.56.1"); // the peer of @axe-core/playwright, kept on the same version
    expect(deps["@axe-core/playwright"]).toMatch(/^\d+\.\d+\.\d+$/);
  });

  test("the config serves the production build on 127.0.0.1 only, from scratch", () => {
    expect(config).toContain('testMatch: "**/*.e2e.ts"');
    expect(config).toContain("npm run preview -- --host 127.0.0.1 --port 4173 --strictPort");
    expect(config).toContain("reuseExistingServer: false");
    expect(config).not.toContain("0.0.0.0");
    expect(config).not.toMatch(/npm run dev|vite dev/);
    expect(config).toContain("forbidOnly: !!process.env.CI");
  });

  test("reports and results are git-ignored, e2e is type-checked", () => {
    const ignored = read(resolve(webRoot, ".gitignore")).split("\n");
    expect(ignored).toContain("test-results/");
    expect(ignored).toContain("playwright-report/");
    const include = JSON.parse(read(resolve(webRoot, "tsconfig.json"))).include as string[];
    expect(include).toEqual(expect.arrayContaining(["e2e", "playwright.config.ts"]));
  });
});

describe("build chain", () => {
  test("predev, prebuild and pretest run the single wasm build script", () => {
    for (const hook of ["predev", "prebuild", "pretest"]) {
      expect(pkg.scripts[hook]).toBe("node scripts/build-wasm.mjs");
    }
  });

  test("the generated package is git-ignored", () => {
    expect(read(resolve(webRoot, ".gitignore")).split("\n")).toContain("src/wasm/");
  });

  test("Node is pinned for CI and typed", () => {
    expect(read(resolve(webRoot, ".node-version")).trim()).toBe("24");
    expect(pkg.devDependencies["@types/node"]).toBe("24.19.2");
    expect(JSON.parse(read(resolve(webRoot, "tsconfig.json"))).compilerOptions.types).toContain("node");
  });
});
