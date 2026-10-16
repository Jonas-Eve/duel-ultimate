import type { Plugin } from "vite";

/**
 * Content-Security-Policy of the production build (decision S3 of #64): scripts from the same
 * origin only, plus `'wasm-unsafe-eval'` to instantiate the WebAssembly engine; neither
 * `'unsafe-eval'` nor `'unsafe-inline'` (so no inline script, no `<style>`, no `style=`
 * attribute anywhere). HTTP-header policies (`frame-ancestors`, HSTS...) belong to the deployment.
 */
export const CSP = [
  "default-src 'none'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "style-src 'self'",
  "img-src 'self' data:",
  "connect-src 'self'",
  "font-src 'self'",
  "base-uri 'none'",
  "form-action 'none'",
].join("; ");

/** Build only: Vite's dev server injects an inline module script (React Refresh preamble). */
export function cspPlugin(): Plugin {
  return {
    name: "csp",
    apply: "build",
    transformIndexHtml() {
      return [{ tag: "meta", attrs: { "http-equiv": "Content-Security-Policy", content: CSP }, injectTo: "head-prepend" }];
    },
  };
}
