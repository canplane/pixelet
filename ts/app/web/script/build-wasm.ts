#!/usr/bin/env -S deno run -A
// Builds the guest's module (rust/client-bracket/wasm) to where
// src/guest.ts loads it.
//
//   deno run -A script/build-wasm.ts [--published]

import { fileURLToPath } from "node:url";

import { REPO } from "@pxl/build/import-map.ts";

const RUST = new URL(`file://${REPO}/rust/`);
const TARGET = "wasm32-unknown-unknown";
// Cargo's own rule: CARGO_TARGET_DIR, else the workspace's target/.
const TARGET_DIR = Deno.env.get("CARGO_TARGET_DIR") ? new URL(`file://${Deno.env.get("CARGO_TARGET_DIR")}/`) : new URL("target/", RUST);
const BUILT = new URL(`${TARGET}/release/pxl_wasm.wasm`, TARGET_DIR);
const OUT = new URL("../src/guest.wasm", import.meta.url);
/** The source path each number in a published module's panics stands for; kept by the deploy, never bundled. */
const PATHS = new URL("../src/guest.paths.json", import.meta.url);

/** Panic locations survive `strip` and carry the home directory, so the
 * prefixes are remapped. The repo goes first since it lies under home. */
function remapped(): string {
  const home = Deno.env.get("HOME");
  const root = new URL("../", RUST).pathname.replace(/\/$/, "");
  const prefixes = [root, home].filter((p): p is string => !!p);
  const flags = prefixes.map((p) => `--remap-path-prefix=${p}=`);
  return [Deno.env.get("RUSTFLAGS"), ...flags].filter(Boolean).join(" ");
}

/** What `deno task build` bundles: no dev features, numbered panic paths. */
const published = Deno.args.includes("--published");

const cargo = new Deno.Command("cargo", {
  args: ["build", "--release", "--package", "pxl-wasm", "--target", TARGET, ...(published ? ["--no-default-features"] : [])],
  cwd: RUST,
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

/** A step that failed, and the code the build exits with. */
class Failed extends Error {
  constructor(readonly code: number, message = "") {
    super(message);
  }
}

async function optimized(from: string, out: string): Promise<void> {
  const wasmOpt = new Deno.Command("wasm-opt", {
    args: [...WASM_OPT, from, "-o", out],
    stdout: "inherit",
    stderr: "inherit",
  });
  let done: Deno.CommandOutput;
  try {
    done = await wasmOpt.output();
  } catch {
    throw new Failed(1, "[build-wasm] wasm-opt did not run. The module is finished by Binaryen: brew install binaryen (or your platform's package).");
  }
  if (!done.success) throw new Failed(done.code || 1);
}

/** embassy and critical-section export their `#[no_mangle]` symbols; only these stay. */
const EXPORTS = ["interrupt"];

/** The module is rewritten by pxl-tool's finish_wasm, which owns the binary. */
async function finish(...args: string[]): Promise<string> {
  const done = await new Deno.Command("cargo", {
    args: ["run", "-q", "--package", "pxl-tool", "--bin", "finish_wasm", "--", ...args],
    cwd: RUST,
    stdout: "piped",
    stderr: "inherit",
  }).output();
  if (!done.success) throw new Failed(done.code || 1);
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
  console.error(`[build-wasm] cargo did not run (${err}). The guest needs a Rust toolchain with the ${TARGET} target: rustup target add ${TARGET}`);
  Deno.exit(1);
}
if (!status.success) Deno.exit(status.code);

// Finishing an unchanged module again would reload every open dev page.
if (modified(BUILT) === before && exists(OUT) && modified(OUT) >= before) {
  console.log("[build-wasm] up to date -> src/guest.wasm");
  Deno.exit(0);
}

// One directory a run under cargo's target, removed whole: a failed run leaves
// nothing beside the module, a killed one leaves it where cargo clean takes
// it, and a dev server's build and a check's never share a file. On the
// module's volume, so what it holds is renamed into place.
const work = await Deno.makeTempDir({ dir: fileURLToPath(TARGET_DIR), prefix: "pxl-build-wasm-" });
try {
  const linkedFile = `${work}/linked.wasm`;
  const done = `${work}/done.wasm`;
  await Deno.copyFile(BUILT, linkedFile);
  const linked = (await Deno.stat(linkedFile)).size;
  await finish("exports", linkedFile, ...EXPORTS);
  await optimized(linkedFile, done);
  const numbered = `${work}/paths.json`;
  const paths = published ? Number(await finish("number", done, numbered)) : 0;
  // The module last: it is what a reader keys on.
  if (published) await Deno.rename(numbered, PATHS);
  await Deno.rename(done, OUT);
  const said = (published ? ", without the dev-only settings" : "") + (paths > 0 ? `, and ${paths} source paths numbered` : "");
  console.log(`[build-wasm] ${linked} -> ${(await Deno.stat(OUT)).size} bytes, stripped${said} -> src/guest.wasm`);
} catch (err) {
  if (!(err instanceof Failed)) throw err;
  if (err.message) console.error(err.message);
  Deno.exitCode = err.code;
} finally {
  await Deno.remove(work, { recursive: true });
}
