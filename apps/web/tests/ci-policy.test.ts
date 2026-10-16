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

describe(".github/workflows/ci.yml", () => {
  test("runs on pull_request and push to main only", () => {
    expect(code).toMatch(/^on:\s*\n\s+pull_request:\s*\n\s+push:\s*\n\s+branches: \[main\]/m);
    for (const bad of ["pull_request_target", "workflow_run", "issue_comment", "workflow_dispatch"]) {
      expect(code).not.toContain(bad);
    }
  });

  test("least privilege, no secrets", () => {
    expect(code).toMatch(/^permissions:\s*\n\s+contents: read\s*\n(?!\s+\S+:)/m);
    expect(code.match(/permissions:/g)).toHaveLength(1);
    expect(code).not.toMatch(/secrets\./);
    expect(code).toContain("persist-credentials: false");
    expect(code).toMatch(/timeout-minutes: \d+/);
    expect(code).toMatch(/runs-on: ubuntu-24\.04/);
  });

  test("every action is first-party and pinned to a full SHA", () => {
    const uses = [...code.matchAll(/^\s*-?\s*uses:\s*(\S+)/gm)].map((m) => m[1]);
    expect(uses.length).toBeGreaterThan(0);
    for (const u of uses) expect(u).toMatch(/^actions\/(checkout|setup-node)@[0-9a-f]{40}$/);
  });

  test("never interpolates a GitHub context inside a run script", () => {
    const runs = code.split(/\n(?=\s*- name:)/).filter((s) => /\brun:/.test(s));
    for (const r of runs) expect(r).not.toContain("${{");
  });

  test("setup-node reads an existing node-version file", () => {
    const m = code.match(/node-version-file:\s*(\S+)/);
    expect(m).not.toBeNull();
    expect(existsSync(resolve(repoRoot, m![1]))).toBe(true);
  });

  test("every cargo build/clippy/test/install is --locked", () => {
    const cmds = code.split("\n").filter((l) => /\bcargo (build|clippy|test|install|tree)\b/.test(l));
    expect(cmds.length).toBeGreaterThanOrEqual(5);
    for (const c of cmds) expect(c).toContain("--locked");
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

  test("installs the CLI at the version read from the lock by the build script", () => {
    expect(code).toContain("build-wasm.mjs --print-version");
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
