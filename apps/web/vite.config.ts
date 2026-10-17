import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import { resolveBase } from "./vite/base.ts";
import { cspPlugin } from "./vite/csp-plugin.ts";

const webRoot = fileURLToPath(new URL(".", import.meta.url));
const dataRoot = fileURLToPath(new URL("../../data", import.meta.url));

export default defineConfig({
  // `VITE_BASE`: the sub-path of a static deployment (`/` by default, so dev, preview and e2e stay at the root).
  base: resolveBase(process.env.VITE_BASE),
  build: {
    // Without this Vite inlines any file under 4 KiB as a `data:` URL in the JavaScript, which the network tests
    // cannot see and which would undo "card images are not loaded on the home screen" (#89). SVG stays a file.
    assetsInlineLimit: (file) => (file.endsWith(".svg") ? false : undefined),
  },
  plugins: [react(), cspPlugin()],
  server: {
    // `src/data/starter.ts` imports `data/starter/<version>/*.json?raw` from the repository
    // root, outside this app: without this, Vite (and so Vitest) answers "Denied ID". Setting
    // `allow` REPLACES Vite's default list, so the app root (and its node_modules) must be in
    // it; never the repository root, which would expose Cargo.toml, .git, ...
    fs: { allow: [webRoot, dataRoot] },
  },
  test: { environment: "jsdom" },
});
