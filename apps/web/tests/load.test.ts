import { describe, expect, test, vi } from "vitest";
import { EngineError } from "../src/engine/errors";
import { createEngineLoader } from "../src/engine/load";

const fakeModule = () => {
  const init = vi.fn(async () => undefined);
  const from_data = vi.fn();
  return { init, from_data, module: { default: init, Game: { from_data } } };
};

describe("createEngineLoader", () => {
  test("two calls make one import and one init", async () => {
    const { init, module } = fakeModule();
    const importer = vi.fn(async () => module);
    const load = createEngineLoader(importer);
    const [a, b] = await Promise.all([load(), load()]);
    await load();
    expect(importer).toHaveBeenCalledTimes(1);
    expect(init).toHaveBeenCalledTimes(1);
    expect(a).toBe(b);
  });

  test("the factory delegates to Game.from_data", async () => {
    const { from_data, module } = fakeModule();
    from_data.mockReturnValue("game");
    const factory = await createEngineLoader(async () => module)();
    expect(factory("c", "d", "one", "two", "[]", 3)).toBe("game");
    expect(from_data).toHaveBeenCalledWith("c", "d", "one", "two", "[]", 3);
  });

  test("a failure is wasm_load_failed and a retry is possible", async () => {
    const { init, module } = fakeModule();
    let attempts = 0;
    const importer = vi.fn(async () => {
      attempts += 1;
      if (attempts === 1) throw new Error("network down");
      return module;
    });
    const load = createEngineLoader(importer);
    const err = await load().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(EngineError);
    expect((err as EngineError).code).toBe("wasm_load_failed");
    await load();
    expect(importer).toHaveBeenCalledTimes(2);
    expect(init).toHaveBeenCalledTimes(1);
  });

  test("an init() failure is also wasm_load_failed and retryable", async () => {
    const { module } = fakeModule();
    const init = vi.fn().mockRejectedValueOnce(new Error("bad wasm")).mockResolvedValue(undefined);
    const load = createEngineLoader(async () => ({ ...module, default: init }));
    await expect(load()).rejects.toMatchObject({ code: "wasm_load_failed" });
    await load();
    expect(init).toHaveBeenCalledTimes(2);
  });
});
