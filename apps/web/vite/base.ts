/**
 * Sub-path a static deployment is served under (`VITE_BASE`, #80), validated strictly: the
 * root "/", or "/a/b/" (leading and trailing slash, segments of [A-Za-z0-9._-]+, never "."
 * or ".."). A relative base ("./") is refused on purpose: the wasm-bindgen glue resolves the
 * `.wasm` with `new URL(..., import.meta.url)`, which Vite emits under an absolute base.
 * Read in the Node process of the config, never through `import.meta.env`.
 */
const BASE_PATTERN = /^\/([A-Za-z0-9._-]+\/)*$/;

export function resolveBase(raw: string | undefined): string {
  if (raw === undefined || raw === "") return "/";
  const valid = BASE_PATTERN.test(raw) && raw.split("/").every((segment) => segment !== "." && segment !== "..");
  if (!valid) {
    throw new Error(
      "VITE_BASE must be \"/\" or an absolute sub-path like \"/duel-ultimate/\" " +
        "(leading and trailing slash, segments of A-Z a-z 0-9 . _ -, no \".\" or \"..\" segment); received " +
        JSON.stringify(raw),
    );
  }
  return raw;
}
