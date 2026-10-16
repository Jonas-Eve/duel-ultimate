import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// Plain strings, never a `file:` URL handed to `fs`: under jsdom (the Vitest environment)
// `readFileSync(new URL(..., import.meta.url))` throws "The URL must be of scheme file".
const here = dirname(fileURLToPath(import.meta.url));

/** `apps/web`. */
export const webRoot = resolve(here, "../..");
/** The repository root. */
export const repoRoot = resolve(webRoot, "../..");
