#!/usr/bin/env -S deno run -A
// Builds the Rust core's module (crates/pxl/wasm) and puts it where
// src/kernel/module.ts loads it from. Run by every task that loads the module —
// dev, build, test — so a checkout never runs against a stale or missing
// one; cargo's own cache makes a run with nothing changed cost a second.
//
// Usage (from apps/web):
//   deno run -A script/build-wasm.ts               # the module anyone works on
//   deno run -A script/build-wasm.ts --published   # what `build` bundles

import { fileURLToPath } from "node:url";

const CRATES = new URL("../../../crates/", import.meta.url);
const TARGET = "wasm32-unknown-unknown";
const BUILT = new URL(`target/${TARGET}/release/pxl_wasm.wasm`, CRATES);
const OUT = new URL("../src/kernel/kernel.wasm", import.meta.url);

/**
 * The paths a panic carries, cut back to nothing that names this machine.
 *
 * `strip` (crates/Cargo.toml) takes the name section off, but a panic's
 * location is not debug information — it is a `&'static str` the panic
 * machinery reads, so it stays, and with it wherever the file was compiled
 * from. The standard library's and the registry's are absolute and carry the
 * home directory, which is a user's name published in the deployed module.
 *
 * Computed here rather than written into a config file, since the prefixes
 * are this machine's and belong in no repository. The repo goes first: it
 * lies under the home directory and would otherwise be rewritten by it.
 */
function remapped(): string {
  const home = Deno.env.get("HOME");
  const root = new URL("../", CRATES).pathname.replace(/\/$/, "");
  const prefixes = [root, home].filter((p): p is string => !!p);
  const flags = prefixes.map((p) => `--remap-path-prefix=${p}=`);
  return [Deno.env.get("RUSTFLAGS"), ...flags].filter(Boolean).join(" ");
}

/** Whether this is the module in its published shape: what `deno task
 * build` bundles, whoever runs it. What a published module leaves out — the
 * dev-only settings (the crate's `dev` feature), its panic paths (below) —
 * is gated on this and on nothing else, so a local build and the deploy's
 * are the same bytes; the build number the deploy hands out is a stamp on
 * the build info, not a change to the module. */
const published = Deno.args.includes("--published");

const cargo = new Deno.Command("cargo", {
  args: ["build", "--release", "--package", "pxl-wasm", "--target", TARGET, ...(published ? ["--no-default-features"] : [])],
  cwd: CRATES,
  env: { RUSTFLAGS: remapped() },
  stdout: "inherit",
  stderr: "inherit",
});

/**
 * Binaryen's finish over the linker's output: the sections a published module
 * has no use for, and the names of everything it says to the shell.
 *
 * `--minify-imports-and-exports` is what renames the host ABI, and it is the
 * only thing that can: the symbols embassy and critical-section require are
 * fixed by those crates, so nothing in the Rust can shorten them, while this
 * runs after the link and does not care where a name came from. It answers
 * with the mapping, which is written beside the module for the shell to
 * import — the letters are assigned by the optimizer's own order and change
 * whenever the module does, so neither side may spell them by hand.
 *
 * The strips run here rather than through Cargo's `strip`, so that the target
 * features are still declared when wasm-opt reads them; taken off in the same
 * pass, they are never asked for and never guessed at.
 */
const WASM_OPT = [
  // Over the whole module, which is more than the compiler saw: LTO stops at
  // the crate graph, and this inlines across it and merges what came out the
  // same. It runs before the strips so that the target features are still
  // declared while it works.
  "-O3",
  "--strip-debug",
  "--strip-producers",
  "--strip-target-features",
  "--minify-imports-and-exports",
];

type Abi = { imports: [string, string, string][]; exports: [string, string][] };

async function minified(): Promise<Abi> {
  const out = new URL("kernel.opt.wasm", OUT);
  const wasmOpt = new Deno.Command("wasm-opt", {
    args: [...WASM_OPT, fileURLToPath(OUT), "-o", fileURLToPath(out)],
    stdout: "piped",
    stderr: "inherit",
  });
  let done: Deno.CommandOutput;
  try {
    done = await wasmOpt.output();
  } catch {
    console.error("[build-wasm] wasm-opt did not run. The module is finished by Binaryen: brew install binaryen (or your platform's package).");
    Deno.exit(1);
  }
  if (!done.success) Deno.exit(done.code || 1);
  await Deno.rename(fileURLToPath(out), fileURLToPath(OUT));
  return JSON.parse(new TextDecoder().decode(done.stdout));
}

/**
 * The source paths a panic carries, blanked where they lie — the published
 * shape only.
 *
 * Rust embeds them and offers no way not to. `unwrap`, `expect` and an index
 * are `#[track_caller]`, so the location a panic reports is the caller's, and
 * the caller is a file of this repository: 87 of them are named in the module,
 * which is the crate tree printed out a second time after the name section was
 * stripped. `--remap-path-prefix` only rewrites a prefix, so the tail — the
 * part that says `io/fetch.rs` — always survives it, and the switch that would
 * drop the location outright (`-Z location-detail`) is nightly's.
 *
 * A `&'static str` is a pointer and a length held apart, so overwriting the
 * bytes in place, to the same length, leaves a module that runs exactly as it
 * did: nothing compares a path at runtime, and the length nothing reads from
 * the text is unchanged. What is lost is the file in a panic's message, which
 * is why this is for what ships and not for what anyone debugs.
 */
/** Where a source path can have come from: this workspace, the toolchain, or
 * a dependency. Anything else ending in `.rs` is not one, and is refused
 * rather than guessed at. */
const SOURCE_ROOT = /(?:^|[^A-Za-z0-9_])(?:pxl|xpute)\/|\/library\/|\/registry\/|^\/rustc\/|^\/rust\/deps\/|\.rustup\//;

function blanked(): number {
  const bytes = Deno.readFileSync(OUT);
  const text = new TextDecoder("latin1").decode(bytes);
  let n = 0;
  for (const m of text.matchAll(/[A-Za-z0-9_/.-]*\.rs\b/g)) {
    // A word that merely ends in those two letters is not a path.
    if (!m[0].includes("/")) continue;
    if (!SOURCE_ROOT.test(m[0])) {
      // Never blanked on a guess: a `.rs` from somewhere this does not know
      // could be a format string or something a user typed, and overwriting
      // it would be a bug nobody sees until it ships.
      throw new Error(`[build-wasm] ${JSON.stringify(m[0])} ends in .rs but is under no source root — widen SOURCE_ROOT or leave it alone`);
    }
    // Every match is filled with the same byte, which is what makes an
    // overlap safe: the linker merges a string that is another's tail, so two
    // matches can cover the same bytes, and filling both with the one
    // character leaves the same result either way.
    bytes.fill(0x2e, m.index, m.index + m[0].length);
    n++;
  }
  if (n > 0) Deno.writeFileSync(OUT, bytes);
  return n;
}

/** The mapping as the shell reads it: a logical name to the one letter the
 * module now answers to. */
async function writeAbi(abi: Abi): Promise<void> {
  const rows = (pairs: [string, string][]) => pairs.map(([name, short]) => `  ${JSON.stringify(name)}: ${JSON.stringify(short)},`).join("\n");
  await Deno.writeTextFile(
    new URL("../src/kernel/abi/symbol.ts", import.meta.url),
    `// @pxl/web/kernel/abi/symbol.ts
//
// GENERATED by script/build-wasm.ts from wasm-opt's mapping — do not edit,
// and do not commit: the letters are the optimizer's and change with the
// module. Every build writes it before anything reads it.

/** What the module calls the host's imports. */
export const IMPORT: Record<string, string> = {
${rows(abi.imports.map(([, name, short]) => [name, short]))}
};

/** What the module calls its own exports. */
export const EXPORT: Record<string, string> = {
${rows(abi.exports)}
};
`,
  );
}

let status: Deno.CommandStatus;
try {
  status = await cargo.output();
} catch (err) {
  console.error(`[build-wasm] cargo did not run (${err}). The core needs a Rust toolchain with the ${TARGET} target: rustup target add ${TARGET}`);
  Deno.exit(1);
}
if (!status.success) Deno.exit(status.code);

await Deno.copyFile(BUILT, OUT);
const linked = (await Deno.stat(OUT)).size;
await writeAbi(await minified());
// Only what is published loses its panic paths.
const paths = published ? blanked() : 0;
const said = (published ? ", without the dev-only settings" : "") + (paths > 0 ? `, and ${paths} source paths blanked` : "");
console.log(`[build-wasm] ${linked} -> ${(await Deno.stat(OUT)).size} bytes, stripped and its names minified${said} -> src/kernel/kernel.wasm`);
