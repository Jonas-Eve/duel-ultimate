// @vitest-environment node
// #80: `VITE_BASE` sets Vite's `base` (the sub-path of a static deployment); strict validation.
import { describe, expect, test } from "vitest";
import { resolveBase } from "../vite/base";

describe("resolveBase", () => {
  test.each([[undefined], [""], ["/"]])("%j gives the root", (raw) => {
    expect(resolveBase(raw)).toBe("/");
  });

  test.each([["/duel-ultimate/"], ["/a/b.c_d-e/"], ["/a/"], ["/.hidden/"], ["/a..b/"]])("%j is accepted as is", (raw) => {
    expect(resolveBase(raw)).toBe(raw);
  });

  test.each([
    ["duel-ultimate"],
    ["/duel-ultimate"],
    ["./"],
    ["../"],
    ["https://x.github.io/duel-ultimate/"],
    ["//x/"],
    ["/a b/"],
    ["/a//b/"],
    ["/../"],
    ["/./"],
    ["/a/../"],
    ["/a?x=1/"],
    ["/a#b/"],
    ["\\a\\"],
    ["/a/\n"],
  ])("%j is rejected with an error that names VITE_BASE", (raw) => {
    expect(() => resolveBase(raw)).toThrow(/VITE_BASE/);
  });

  test("the error carries the received value and the expected form", () => {
    expect(() => resolveBase("./")).toThrow(/\.\//);
    expect(() => resolveBase("./")).toThrow(/\/duel-ultimate\//);
  });
});
