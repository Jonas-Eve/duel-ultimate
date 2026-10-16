import { defineConfig } from "@playwright/test";

// End-to-end tests against the PRODUCTION build (CSP, hashed .wasm) served by `vite preview`.
// Chromium comes from PLAYWRIGHT_BROWSERS_PATH (never run `playwright install` in the agents'
// environment). `@playwright/test` is pinned to 1.56.1: see CLAUDE.md, "Playwright".
export default defineConfig({
  testDir: "e2e",
  testMatch: "**/*.e2e.ts",
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  forbidOnly: !!process.env.CI,
  reporter: [["list"]],
  timeout: 60_000,
  use: {
    baseURL: "http://127.0.0.1:4173",
    viewport: { width: 360, height: 740 },
    hasTouch: true,
    isMobile: true,
    locale: "fr-FR",
    trace: "off",
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
  webServer: {
    command: "npm run preview -- --host 127.0.0.1 --port 4173 --strictPort",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: false,
  },
});
