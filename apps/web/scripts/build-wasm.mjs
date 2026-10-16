// Builds engine-wasm and generates the JS package in apps/web/src/wasm/ (git-ignored).
// The only place that knows how: called by the npm hooks predev/prebuild/pretest and by CI.
// `--print-version` only prints the wasm-bindgen version of Cargo.lock (CI installs the CLI with it).
import { spawnSync } from "node:child_process";
import { readFileSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { checkCliVersion, installCommand, lockedVersion } from "./wasm-version.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const outDir = resolve(repoRoot, "apps/web/src/wasm");
const targetDir = resolve(repoRoot, "target");
const wasmFile = resolve(targetDir, "wasm32-unknown-unknown/release/duel_engine_wasm.wasm");

function fail(message) {
  console.error(`build-wasm: ${message}`);
  process.exit(1);
}

/** Runs a command without a shell; exits with its status on failure. */
function run(cmd, args, hint) {
  console.log(`build-wasm: ${cmd} ${args.join(" ")}`);
  const r = spawnSync(cmd, args, { stdio: "inherit", shell: false, cwd: repoRoot });
  if (r.error) fail(`${cmd} could not be run (${r.error.message}). ${hint}`);
  if (r.status !== 0) process.exit(r.status ?? 1);
}

let version;
try {
  version = lockedVersion(readFileSync(resolve(repoRoot, "Cargo.lock"), "utf8"));
} catch (e) {
  fail(e.message);
}

if (process.argv.includes("--print-version")) {
  console.log(version);
  process.exit(0);
}

try {
  checkCliVersion(version, spawnSync("wasm-bindgen", ["--version"], { encoding: "utf8", shell: false }));
} catch (e) {
  fail(e.message);
}

run(
  "cargo",
  ["build", "--locked", "--target", "wasm32-unknown-unknown", "-p", "duel-engine-wasm", "--release", "--target-dir", targetDir],
  "Install Rust (https://rustup.rs) and the target: rustup target add wasm32-unknown-unknown",
);
rmSync(outDir, { recursive: true, force: true });
run("wasm-bindgen", ["--target", "web", "--out-dir", outDir, wasmFile], installCommand(version));
console.log(`build-wasm: generated ${outDir}`);
