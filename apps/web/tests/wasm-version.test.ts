import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "vitest";
import { checkCliVersion, lockedVersion } from "../scripts/wasm-version.mjs";
import { repoRoot } from "./support/paths";

const entry = (name: string, version: string) =>
  `[[package]]\nname = "${name}"\nversion = "${version}"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n`;

describe("lockedVersion", () => {
  test("reads the single wasm-bindgen entry", () => {
    const lock = ["version = 4", entry("serde", "1.0.1"), entry("wasm-bindgen", "0.2.129")].join("\n");
    expect(lockedVersion(lock)).toBe("0.2.129");
  });

  test("ignores wasm-bindgen-macro and siblings, even placed first", () => {
    const lock = [
      entry("wasm-bindgen-macro", "9.9.9"),
      entry("wasm-bindgen-shared", "8.8.8"),
      entry("wasm-bindgen", "0.2.129"),
    ].join("\n");
    expect(lockedVersion(lock)).toBe("0.2.129");
  });

  test("reads the real Cargo.lock of the repository", () => {
    const lock = readFileSync(resolve(repoRoot, "Cargo.lock"), "utf8");
    expect(lockedVersion(lock)).toMatch(/^\d+\.\d+\.\d+$/);
  });

  test("two entries are an error, not 'the first one'", () => {
    const lock = [entry("wasm-bindgen", "0.2.1"), entry("wasm-bindgen", "0.2.2")].join("\n");
    expect(() => lockedVersion(lock)).toThrow(/wasm-bindgen/);
  });

  test("no entry is an error", () => {
    expect(() => lockedVersion(entry("serde", "1.0.1"))).toThrow(/wasm-bindgen/);
    expect(() => lockedVersion(entry("wasm-bindgen-macro", "0.2.1"))).toThrow(/wasm-bindgen/);
  });

  test("a malformed version is an error", () => {
    expect(() => lockedVersion(entry("wasm-bindgen", "0.2"))).toThrow();
    expect(() => lockedVersion(entry("wasm-bindgen", "0.2.129-rc.1"))).toThrow();
    expect(() => lockedVersion(entry("wasm-bindgen", '0.2.1"; rm -rf /'))).toThrow();
  });
});

describe("checkCliVersion", () => {
  const ok = (stdout: string) => ({ status: 0, stdout, error: undefined });
  const install = "cargo install wasm-bindgen-cli --version 0.2.129 --locked";

  test("accepts exactly the expected version", () => {
    expect(() => checkCliVersion("0.2.129", ok("wasm-bindgen 0.2.129\n"))).not.toThrow();
  });

  test.each([
    ["a different patch", "wasm-bindgen 0.2.130\n"],
    ["a prefix of the expected version", "wasm-bindgen 0.2.12\n"],
    ["a pre-release suffix", "wasm-bindgen 0.2.129-rc\n"],
    ["an unexpected output", "something else\n"],
    ["an empty output", ""],
  ])("rejects %s with the install command", (_label, stdout) => {
    expect(() => checkCliVersion("0.2.129", ok(stdout))).toThrow(install);
  });

  test("a missing CLI (ENOENT) fails with the install command", () => {
    const error = Object.assign(new Error("spawnSync wasm-bindgen ENOENT"), { code: "ENOENT" });
    expect(() => checkCliVersion("0.2.129", { error, status: null, stdout: "" })).toThrow(install);
  });

  test("an error wins even if the status and output look fine", () => {
    const error = new Error("boom");
    expect(() => checkCliVersion("0.2.129", { error, status: 0, stdout: "wasm-bindgen 0.2.129\n" })).toThrow(install);
  });

  test("a non-zero status fails with the install command", () => {
    expect(() => checkCliVersion("0.2.129", { status: 1, stdout: "wasm-bindgen 0.2.129\n" })).toThrow(install);
  });
});
