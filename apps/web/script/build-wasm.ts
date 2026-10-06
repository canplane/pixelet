#!/usr/bin/env -S deno run -A
// Builds the Rust core's module (crates/client/wasm) to where
// src/platform/web/core.ts loads it.
//
//   deno run -A script/build-wasm.ts [--published]

import { fileURLToPath } from "node:url";

const CRATES = new URL("../../../crates/", import.meta.url);
const TARGET = "wasm32-unknown-unknown";
const BUILT = new URL(`target/${TARGET}/release/pxl_wasm.wasm`, CRATES);
const OUT = new URL("../src/kernel/kernel.wasm", import.meta.url);
/** The source path each number in a published module's panics stands for; kept by the deploy, never bundled. */
const PATHS = new URL("../src/kernel/kernel.paths.json", import.meta.url);

/** Panic locations survive `strip` and carry the home directory, so the
 * prefixes are remapped. The repo goes first since it lies under home. */
function remapped(): string {
  const home = Deno.env.get("HOME");
  const root = new URL("../", CRATES).pathname.replace(/\/$/, "");
  const prefixes = [root, home].filter((p): p is string => !!p);
  const flags = prefixes.map((p) => `--remap-path-prefix=${p}=`);
  return [Deno.env.get("RUSTFLAGS"), ...flags].filter(Boolean).join(" ");
}

/** What `deno task build` bundles: no dev features, numbered panic paths. */
const published = Deno.args.includes("--published");

const cargo = new Deno.Command("cargo", {
  args: ["build", "--release", "--package", "pxl-wasm", "--target", TARGET, ...(published ? ["--no-default-features"] : [])],
  cwd: CRATES,
  env: { RUSTFLAGS: remapped() },
  stdout: "inherit",
  stderr: "inherit",
});

/** Strips run here, not via Cargo, so target features are still declared while wasm-opt runs. */
const WASM_OPT = [
  // Inlines across the crate graph, which LTO does not.
  "-O3",
  "--strip-debug",
  "--strip-producers",
  "--strip-target-features",
];

/** Per-run files, renamed into place at the end: a dev server's build and a check's ran at once. */
function staged(step: string): URL {
  return new URL(`kernel.${Deno.pid}.${step}.wasm`, OUT);
}

async function optimized(from: URL, out: URL): Promise<void> {
  const wasmOpt = new Deno.Command("wasm-opt", {
    args: [...WASM_OPT, fileURLToPath(from), "-o", fileURLToPath(out)],
    stdout: "inherit",
    stderr: "inherit",
  });
  let done: Deno.CommandOutput;
  try {
    done = await wasmOpt.output();
  } catch {
    console.error("[build-wasm] wasm-opt did not run. The module is finished by Binaryen: brew install binaryen (or your platform's package).");
    Deno.exit(1);
  }
  await Deno.remove(from);
  if (!done.success) Deno.exit(done.code || 1);
}

/** embassy and critical-section export their `#[no_mangle]` symbols; only these stay. */
const EXPORTS = ["interrupt"];

/** The module is rewritten by pxl-tool's finish_wasm, which owns the binary. */
async function finish(...args: string[]): Promise<string> {
  const done = await new Deno.Command("cargo", {
    args: ["run", "-q", "--package", "pxl-tool", "--bin", "finish_wasm", "--", ...args],
    cwd: CRATES,
    stdout: "piped",
    stderr: "inherit",
  }).output();
  if (!done.success) Deno.exit(done.code || 1);
  return new TextDecoder().decode(done.stdout).trim();
}

const modified = (url: URL) => Deno.statSync(url).mtime?.getTime() ?? NaN;
const exists = (url: URL) => {
  try {
    return Deno.statSync(url).isFile;
  } catch {
    return false;
  }
};
const before = exists(BUILT) ? modified(BUILT) : NaN;

let status: Deno.CommandStatus;
try {
  status = await cargo.output();
} catch (err) {
  console.error(`[build-wasm] cargo did not run (${err}). The core needs a Rust toolchain with the ${TARGET} target: rustup target add ${TARGET}`);
  Deno.exit(1);
}
if (!status.success) Deno.exit(status.code);

// Finishing an unchanged module again would reload every open dev page.
if (modified(BUILT) === before && exists(OUT) && modified(OUT) >= before) {
  console.log("[build-wasm] up to date -> src/kernel/kernel.wasm");
  Deno.exit(0);
}

const linkedFile = staged("linked");
const done = staged("done");
await Deno.copyFile(BUILT, linkedFile);
const linked = (await Deno.stat(linkedFile)).size;
await finish("exports", fileURLToPath(linkedFile), ...EXPORTS);
await optimized(linkedFile, done);
const paths = published ? Number(await finish("number", fileURLToPath(done), fileURLToPath(PATHS))) : 0;
await Deno.rename(done, OUT);
const said = (published ? ", without the dev-only settings" : "") + (paths > 0 ? `, and ${paths} source paths numbered` : "");
console.log(`[build-wasm] ${linked} -> ${(await Deno.stat(OUT)).size} bytes, stripped${said} -> src/kernel/kernel.wasm`);
