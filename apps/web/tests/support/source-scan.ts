import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { webRoot } from "./paths";

export const srcDir = join(webRoot, "src");

/** All `.ts`/`.tsx` files under `dir`, except the generated `src/wasm/`. Tolerates a missing dir. */
export function sourceFiles(dir = srcDir): { path: string; rel: string; text: string }[] {
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return [];
  }
  return entries.flatMap((name) => {
    const full = join(dir, name);
    const rel = relative(srcDir, full).split("\\").join("/");
    if (rel === "wasm") return [];
    if (statSync(full).isDirectory()) return sourceFiles(full);
    return /\.(ts|tsx)$/.test(name) ? [{ path: full, rel, text: readFileSync(full, "utf8") }] : [];
  });
}

/** Removes `//` and `/* *​/` comments (naive, enough for this code base: no URLs in strings). */
export function stripComments(text: string): string {
  return text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:"'`\\])\/\/.*$/gm, "$1");
}

/** Module specifiers of static imports, re-exports and dynamic `import()`. */
export function importsOf(text: string): string[] {
  const code = stripComments(text);
  return [...code.matchAll(/(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s+)["']([^"']+)["']/g)].map((m) => m[1]);
}
