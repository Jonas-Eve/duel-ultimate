// The build identifier shown on the home screen (#80): the first 7 characters of the commit SHA
// the CI gives to `VITE_BUILD_SHA`, `dev` otherwise. Read by `App` alone, like `data/starter.ts`.
// Only `VITE_`-prefixed variables reach the bundle; the value is validated before the interface.

/** 7 to 40 lowercase hexadecimal characters: a (possibly abbreviated) commit SHA. */
const SHA = /^[0-9a-f]{7,40}$/;

export function buildIdFrom(raw: unknown): string {
  return typeof raw === "string" && SHA.test(raw) ? raw.slice(0, 7) : "dev";
}

export const BUILD_ID: string = buildIdFrom(import.meta.env.VITE_BUILD_SHA);
