// #80: the build identifier shown on the home screen is the first 7 hex characters of a commit
// SHA (`VITE_BUILD_SHA`), `dev` otherwise. Under Vitest the variable is not set.
import { describe, expect, test } from "vitest";
import { BUILD_ID, buildIdFrom } from "../src/data/build";

const hex = (n: number) => "0123456789abcdef".repeat(3).slice(0, n);

describe("buildIdFrom", () => {
  test("cuts a SHA to its first 7 characters", () => {
    expect(buildIdFrom("abc1234def")).toBe("abc1234");
    expect(buildIdFrom(hex(40))).toBe(hex(7));
  });

  test("exactly 7 hex characters is kept as is", () => {
    expect(buildIdFrom("abc1234")).toBe("abc1234");
  });

  test.each([[undefined], [""], ["abc123"], ["ABC1234"], ["abc123g"], [hex(41)], [42], [null], [" abc1234"], ["abc1234\n"]])("%j gives dev", (raw) => {
    expect(buildIdFrom(raw)).toBe("dev");
  });

  test("BUILD_ID is dev under test", () => {
    expect(BUILD_ID).toBe("dev");
  });
});
