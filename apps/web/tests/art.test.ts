import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { renderHook } from "@testing-library/react";
import { createElement } from "react";
import { describe, expect, test } from "vitest";
import { ART, ART_LICENSES, buildArtIndex } from "../src/data/art";
import { CARDS_JSON, DATA_VERSION } from "../src/data/starter";
import type { ArtInfo } from "../src/game/art";
import { ArtProvider, useArtLookup } from "../src/ui/ArtContext";
import { repoRoot, webRoot } from "./support/paths";

const cards = (...names: [id: string, name: string][]) =>
  JSON.stringify(names.map(([id, name]) => ({ kind: "creature", id, name, cost: 1, force: 1, attack_cost: 1, health: 1 })));
const entry = (over: Record<string, unknown> = {}) => ({
  id: "a",
  file: "wolf.webp",
  author: "Jane",
  license: "CC0-1.0",
  source: "https://example.org/wolf",
  ...over,
});
const IMAGES = { "wolf.webp": "/assets/wolf-abc.webp", "bear.svg": "data:image/svg+xml,x" };
const CARDS = cards(["a", "Loup"], ["b", "Ours"]);
const lookup = (art: unknown, images: Record<string, string> = IMAGES, c = CARDS) =>
  buildArtIndex(c, art === undefined ? undefined : typeof art === "string" ? art : JSON.stringify(art), images);

describe("buildArtIndex", () => {
  test("a valid entry is found by the display name, with text fields only", () => {
    expect(lookup([entry()])("Loup")).toEqual<ArtInfo>({
      url: "/assets/wolf-abc.webp",
      author: "Jane",
      license: "CC0-1.0",
      source: "https://example.org/wolf",
    });
    expect(lookup([entry()])("Ours")).toBeUndefined();
    expect(lookup([entry()])("loup")).toBeUndefined();
  });

  test("no art.json: nothing found, no throw", () => {
    expect(lookup(undefined)("Loup")).toBeUndefined();
  });

  test.each(["{", "", "null", "{}", '"x"', "42"])("art.json %j is ignored", (raw) => {
    expect(lookup(raw)("Loup")).toBeUndefined();
  });

  test.each([
    ["missing author", { author: undefined }],
    ["missing id", { id: undefined }],
    ["non string source", { source: 3 }],
    ["unknown licence", { license: "MIT" }],
    ["licence in lower case", { license: "cc0-1.0" }],
    ["parent directory", { file: "../wolf.webp" }],
    ["absolute path", { file: "/wolf.webp" }],
    ["http url", { file: "http:wolf.webp" }],
    ["data url", { file: "data:image/svg+xml,x" }],
    ["subfolder", { file: "art/wolf.webp" }],
    ["unknown extension", { file: "wolf.png" }],
    ["unknown card id", { id: "zzz" }],
  ])("an invalid entry is ignored: %s", (_label, over) => {
    expect(lookup([entry(over)])("Loup")).toBeUndefined();
  });

  test("an invalid entry does not hide the valid ones", () => {
    const l = lookup([null, 3, entry({ id: "zzz" }), entry({ id: "b", file: "bear.svg" })]);
    expect(l("Ours")?.url).toBe(IMAGES["bear.svg"]);
    expect(l("Loup")).toBeUndefined();
  });

  test("the file is a key in the closed table, never a path: a missing key is a placeholder", () => {
    expect(lookup([entry({ file: "missing.webp" })])("Loup")).toBeUndefined();
    // an `images` table with a hostile key is not reachable through `file`
    expect(lookup([entry({ file: "wolf.webp" })], { "../wolf.webp": "x" })("Loup")).toBeUndefined();
    expect(lookup([entry({ file: "toString" })], {})("Loup")).toBeUndefined();
  });

  test("two cards with the same name: no art for that name", () => {
    const dup = cards(["a", "Loup"], ["b", "Loup"]);
    expect(lookup([entry(), entry({ id: "b", file: "bear.svg" })], IMAGES, dup)("Loup")).toBeUndefined();
  });

  test("a duplicated art entry for one card is ambiguous: no art", () => {
    expect(lookup([entry(), entry({ file: "bear.svg" })])("Loup")).toBeUndefined();
  });

  test.each(["__proto__", "toString", "constructor", "hasOwnProperty"])("the name %s is an ordinary name", (name) => {
    expect(lookup([])(name)).toBeUndefined();
    const c = cards(["a", name]);
    expect(lookup([entry()], IMAGES, c)(name)?.url).toBe(IMAGES["wolf.webp"]);
    expect(lookup([entry()], IMAGES, c)("other")).toBeUndefined();
  });

  test("invalid cards.json: never throws, finds nothing", () => {
    for (const bad of ["{", "null", "[null, 1, {}]", '[{"id":1,"name":2}]']) {
      expect(lookup([entry()], IMAGES, bad)("Loup")).toBeUndefined();
    }
  });

  test("non-string names never match", () => {
    expect(lookup([entry()])(undefined as unknown as string)).toBeUndefined();
  });
});

describe("the starter set", () => {
  test("1.0.0 has no art yet: the index is empty and the placeholder stays", () => {
    const names: string[] = (JSON.parse(CARDS_JSON) as { name: string }[]).map((c) => c.name);
    expect(names.length).toBeGreaterThan(0);
    for (const n of names) expect(ART(n)).toBeUndefined();
  });
});

describe("the glob pinning", () => {
  const src = readFileSync(resolve(webRoot, "src/data/art.ts"), "utf8");
  // Comments only at the start of a line: a naive `/* */` strip would eat the `art/*.{webp,svg}` glob.
  const code = src.replace(/^\s*\/\*[\s\S]*?\*\//gm, "").replace(/^\s*\/\/.*$/gm, "");

  test("literal paths of the same version as DATA_VERSION", () => {
    const globs = [...code.matchAll(/import\.meta\.glob\(\s*"([^"]+)"/g)].map((m) => m[1]);
    expect(globs).toEqual([
      "../../../../data/starter/1.0.0/art.json",
      "../../../../data/starter/1.0.0/art/*.{webp,svg}",
    ]);
    const versions = globs.map((g) => /data\/starter\/(\d+\.\d+\.\d+)\//.exec(g)?.[1]);
    expect(new Set(versions).size).toBe(1);
    expect(DATA_VERSION).toBe(`starter@${versions[0]}`);
  });

  test("the two globs are eager, one raw and one url", () => {
    expect(code).toMatch(/query:\s*"\?raw"/);
    expect(code).toMatch(/query:\s*"\?url"/);
    expect(code.match(/eager:\s*true/g)).toHaveLength(2);
  });

  test("no computed path: no new URL(, no template literal path", () => {
    expect(code).not.toContain("new URL(");
    expect(code).not.toMatch(/glob\(\s*`/);
    expect(code).not.toMatch(/`[^`]*data\/starter/);
  });
});

describe("licences", () => {
  test("the TS ids are those of ART_LICENSES in crates/cards/src/art.rs", () => {
    const rs = readFileSync(resolve(repoRoot, "crates/cards/src/art.rs"), "utf8");
    const m = /pub const ART_LICENSES:\s*\[&str;\s*\d+\]\s*=\s*\[([^\]]*)\]/.exec(rs);
    expect(m).not.toBeNull();
    const ids = [...(m as RegExpExecArray)[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
    expect(ids.length).toBeGreaterThan(0);
    expect([...ART_LICENSES].sort()).toEqual([...ids].sort());
  });
});

describe("useArtLookup", () => {
  test("without a provider, no card has an image", () => {
    const { result } = renderHook(() => useArtLookup());
    expect(result.current("Loup")).toBeUndefined();
  });

  test("with a provider, it returns the given lookup", () => {
    const info: ArtInfo = { url: "u", author: "a", license: "CC0-1.0", source: "s" };
    const { result } = renderHook(() => useArtLookup(), {
      wrapper: ({ children }) => createElement(ArtProvider, { lookup: (n: string) => (n === "Loup" ? info : undefined), children }),
    });
    expect(result.current("Loup")).toBe(info);
    expect(result.current("x")).toBeUndefined();
  });
});
