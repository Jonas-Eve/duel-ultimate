// Pure helpers of the WASM build chain (no I/O): the CLI version is read from Cargo.lock,
// the single source of truth, and compared strictly with what `wasm-bindgen --version` prints.

const VERSION = /^\d+\.\d+\.\d+$/;

/**
 * The command to run to install the CLI at the given version.
 * @param {string} version
 */
export function installCommand(version) {
  return `cargo install wasm-bindgen-cli --version ${version} --locked`;
}

/**
 * The version of the single `wasm-bindgen` package of a Cargo.lock.
 * `wasm-bindgen-macro` and the other `wasm-bindgen-*` packages have another name: ignored.
 * @param {string} lockText
 * @returns {string}
 */
export function lockedVersion(lockText) {
  const versions = [];
  for (const block of lockText.split(/^\[\[package\]\]\s*$/m).slice(1)) {
    const name = /^name = "([^"\n]*)"\s*$/m.exec(block)?.[1];
    if (name !== "wasm-bindgen") continue;
    versions.push(/^version = "([^"\n]*)"\s*$/m.exec(block)?.[1]);
  }
  if (versions.length !== 1) {
    throw new Error(`Cargo.lock must have exactly one "wasm-bindgen" package, found ${versions.length}`);
  }
  const [version] = versions;
  if (typeof version !== "string" || !VERSION.test(version)) {
    throw new Error("the wasm-bindgen version of Cargo.lock is not a plain X.Y.Z version");
  }
  return version;
}

/**
 * Fails unless the installed CLI is exactly the expected version.
 * @param {string} expected a version already validated by `lockedVersion`
 * @param {{ error?: Error, status?: number | null, stdout?: string | Buffer | null }} result a `spawnSync` result
 */
export function checkCliVersion(expected, result) {
  const fix = `install it with: ${installCommand(expected)}`;
  if (result.error) {
    throw new Error(`wasm-bindgen could not be run (${result.error.message}); ${fix}`);
  }
  if (result.status !== 0) {
    throw new Error(`\`wasm-bindgen --version\` exited with status ${result.status}; ${fix}`);
  }
  const out = String(result.stdout ?? "").trim();
  if (out !== `wasm-bindgen ${expected}`) {
    throw new Error(`wasm-bindgen CLI version mismatch: expected exactly ${expected}, got "${out}"; ${fix}`);
  }
}
