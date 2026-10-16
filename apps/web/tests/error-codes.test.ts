import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, test } from "vitest";
import { CLIENT_CODES } from "../src/engine/errors";
import { ERROR_TEXT, messageFor } from "../src/ui/errors.fr";
import { repoRoot } from "./support/paths";

const rust = (p: string) => readFileSync(resolve(repoRoot, p), "utf8");

/** Literals returned by the body of `pub fn code(&self)`: `=> "code"`. */
export function codesOfCodeFn(src: string): string[] {
  const start = src.indexOf("pub fn code(&self)");
  if (start === -1) return [];
  const body = src.slice(start, src.indexOf("\n    }\n", start));
  return [...body.matchAll(/=>\s*"([a-z_]+)"/g)].map((m) => m[1]);
}

/** First literal argument of every `error_json(` call, outside the `tests` module. */
export function codesOfErrorJson(src: string): string[] {
  const code = src.split(/\n#\[cfg\(test\)\]/)[0];
  const calls = code.split("error_json(").slice(1);
  return calls.flatMap((c) => {
    const m = /^\s*"([a-z_]+)"/.exec(c);
    return m ? [m[1]] : [];
  });
}

/** Messages of `#[error("...")]` attributes (the English texts the UI must not show). */
const englishMessages = (src: string) => [...src.matchAll(/#\[error\("([^"]+)"\)\]/g)].map((m) => m[1]);

const files = ["crates/engine/src/rules.rs", "crates/engine/src/setup.rs", "crates/engine/src/data.rs"];
const engineCodes = new Set<string>([
  ...files.flatMap((f) => codesOfCodeFn(rust(f))),
  ...codesOfErrorJson(rust("crates/engine-wasm/src/lib.rs")),
]);
const client = new Set<string>(CLIENT_CODES);

describe("the extraction itself", () => {
  test("codesOfCodeFn and codesOfErrorJson read what they should", () => {
    expect(codesOfCodeFn('impl X {\n    pub fn code(&self) -> &str {\n        match self {\n            Self::A => "a_b",\n        }\n    }\n}\n')).toEqual(["a_b"]);
    expect(codesOfErrorJson('fn f() { error_json("x_y", "m"); error_json(\n  "z",\n "m") }\n#[cfg(test)]\nmod t { error_json("hidden", "") }')).toEqual(["x_y", "z"]);
  });

  test("finds at least 13 distinct engine codes (guards against a blind regex)", () => {
    expect(engineCodes.size).toBeGreaterThanOrEqual(13);
  });
});

describe("the French error table against the Rust sources", () => {
  test("every engine code has a French text", () => {
    for (const c of engineCodes) expect(Object.keys(ERROR_TEXT), c).toContain(c);
  });

  test("no dead entry: every ERROR_TEXT key is an engine or client code", () => {
    for (const k of Object.keys(ERROR_TEXT)) expect(engineCodes.has(k) || client.has(k), k).toBe(true);
  });

  test("every text is French prose, not the code and not the English message", () => {
    const english = new Set([...files.flatMap((f) => englishMessages(rust(f)))]);
    expect(english.size).toBeGreaterThan(0);
    for (const [code, text] of Object.entries(ERROR_TEXT)) {
      expect(text.trim(), code).not.toBe("");
      expect(text, code).not.toContain(code);
      expect(english.has(text), code).toBe(false);
      expect(text, code).toContain(" ");
      expect(text, code).toMatch(/[àâçéèêëîïôûùü]|\b(partie|carte|action|créature|page|jeu|deck|pouvoir)\b/i);
    }
  });

  test("messageFor uses the table, and falls back without throwing for an unknown code", () => {
    expect(messageFor("game_over")).toBe("La partie est terminée.");
    expect(messageFor("target_forbidden_by_provocation")).toBe("Une créature adverse avec provocation doit être attaquée en premier.");
    expect(messageFor("code_inconnu")).toBe("Une erreur inattendue s'est produite (code : code_inconnu).");
    expect(() => messageFor("toString")).not.toThrow();
    expect(messageFor("toString")).toContain("toString");
    expect(messageFor("")).toContain("Une erreur inattendue");
  });
});
