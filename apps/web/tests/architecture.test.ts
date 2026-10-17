import { describe, expect, test } from "vitest";
import { importsOf, sourceFiles, stripComments } from "./support/source-scan";

// Layers (apps/web/README.md), dependencies go down only: engine/ <- game/ <- ui/.
// Read as text, so it also covers files added later without any change here.

const files = sourceFiles();
const inDir = (dir: string) => files.filter((f) => f.rel.startsWith(`${dir}/`));
const isWasm = (spec: string) => /(^|\/)wasm(\/|$)/.test(spec);
const isReact = (spec: string) => /^react(-dom)?(\/|$)/.test(spec);

describe("the import scanner", () => {
  test("sees static, type, re-export and dynamic imports, ignores comments", () => {
    const src = `import a from "./a";\nimport type { B } from '../wasm/x';\nexport * from "./c";\nconst m = await import("../wasm/duel_engine_wasm");\nimport "./side";\n// import z from "react";\n/* import y from "react"; */`;
    expect([...importsOf(src)].sort()).toEqual(["../wasm/duel_engine_wasm", "../wasm/x", "./a", "./c", "./side"]);
    expect(importsOf(src)).toHaveLength(5);
    expect(importsOf(src).filter(isWasm)).toHaveLength(2);
    expect(importsOf(src).some(isReact)).toBe(false);
    expect(importsOf('import { useState } from "react";').some(isReact)).toBe(true);
  });

  test("the scan finds the source files", () => {
    expect(files.length).toBeGreaterThan(5);
    expect(files.some((f) => f.rel === "engine/load.ts")).toBe(true);
    expect(files.some((f) => f.rel.startsWith("wasm/"))).toBe(false);
  });
});

describe("architecture of apps/web/src", () => {
  test("game/ imports neither React nor the WASM package", () => {
    for (const f of inDir("game")) {
      for (const spec of importsOf(f.text)) {
        expect(isReact(spec) || isWasm(spec), `${f.rel} imports ${spec}`).toBe(false);
      }
    }
  });

  test("engine/ and game/ never import ui/ or data/ upwards", () => {
    for (const f of [...inDir("engine"), ...inDir("game")]) {
      for (const spec of importsOf(f.text)) {
        expect(/(^|\/)ui(\/|$)/.test(spec), `${f.rel} imports ${spec}`).toBe(false);
      }
    }
    for (const f of inDir("engine")) {
      for (const spec of importsOf(f.text)) expect(/(^|\/)game(\/|$)/.test(spec), `${f.rel} imports ${spec}`).toBe(false);
    }
  });

  test("ui/ does not import the WASM package", () => {
    for (const f of inDir("ui")) {
      for (const spec of importsOf(f.text)) expect(isWasm(spec), `${f.rel} imports ${spec}`).toBe(false);
    }
  });

  test("only engine/wasm-session.ts (types) and engine/load.ts import the WASM package", () => {
    const importers = files.filter((f) => importsOf(f.text).some(isWasm)).map((f) => f.rel);
    for (const rel of importers) expect(["engine/load.ts", "engine/wasm-session.ts"], rel).toContain(rel);
    expect(importers).toContain("engine/load.ts");
  });

  test("data/starter.ts, data/build.ts and engine/load.ts are read by App only (#64): not by engine/, game/ or ui/", () => {
    for (const f of [...inDir("engine"), ...inDir("game"), ...inDir("ui")]) {
      for (const spec of importsOf(f.text)) {
        expect(/(^|\/)data\/(starter|build)$/.test(spec), `${f.rel} imports ${spec}`).toBe(false);
        expect(/(^|\/)engine\/load$/.test(spec) || (f.rel.startsWith("ui/") && /(^|\/)load$/.test(spec)), `${f.rel} imports ${spec}`).toBe(false);
      }
    }
  });

  test("ui/ only reaches game/ and the engine's types and errors", () => {
    for (const f of inDir("ui")) {
      for (const spec of importsOf(f.text)) {
        if (!spec.startsWith(".")) continue;
        const ok = /^\.\/[A-Za-z0-9.-]+$/.test(spec) || /(^|\/)game\/[a-z-]+$/.test(spec) || /(^|\/)engine\/(types|errors)$/.test(spec);
        expect(ok, `${f.rel} imports ${spec}`).toBe(true);
      }
    }
  });

  test("no layer imports data/ at all: only App.tsx does (#101)", () => {
    for (const f of [...inDir("engine"), ...inDir("game"), ...inDir("ui")]) {
      for (const spec of importsOf(f.text)) expect(/(^|\/)data(\/|$)/.test(spec), `${f.rel} imports ${spec}`).toBe(false);
    }
  });

  test("only App.tsx imports data/art", () => {
    const importers = files.filter((f) => importsOf(f.text).some((s) => /(^|\/)data\/art$/.test(s))).map((f) => f.rel);
    expect(importers).toEqual(["App.tsx"]);
  });

  test("no import.meta.glob, ?raw, ?url or data/ path string in engine/, game/ or ui/ (importsOf does not see globs)", () => {
    const forbidden = [/import\.meta\.glob/, /\?raw\b/, /\?url\b/, /["'`][^"'`\n]*\bdata\/[^"'`\n]*["'`]/];
    for (const f of [...inDir("engine"), ...inDir("game"), ...inDir("ui")]) {
      const code = stripComments(f.text);
      for (const re of forbidden) expect(re.test(code), `${f.rel} matches ${re}`).toBe(false);
    }
    expect(forbidden[0].test("const g = import.meta.glob('x')")).toBe(true);
    expect(forbidden[3].test('const p = "../data/starter/art.json";')).toBe(true);
  });
});
