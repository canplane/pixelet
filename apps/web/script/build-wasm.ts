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
/** What each number a published module reports a place by stands for: its
 * source path. Beside the module and never bundled with it; the deploy keeps
 * it under the build's number (script/deploy.ts), the way a symbol file is
 * kept beside a release rather than inside it. */
const PATHS = new URL("../src/kernel/kernel.paths.json", import.meta.url);

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
 * has no use for.
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
];

/** A file of this run's own beside the module. Every step works on these and
 * the module lands in one rename at the end: a dev server's build and a
 * check's run at once, and on shared names one read what the other had
 * half written, or found it already renamed away. */
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

/** What the module says to the shell: the doorbell and nothing else. */
const EXPORTS = new Set(["interrupt"]);

/**
 * The export section cut to EXPORTS, before the optimizer runs.
 *
 * The linker exports every `#[no_mangle]` function in the crate graph, and
 * embassy and critical-section name theirs so (`__pender`,
 * `_critical_section_1_0_acquire` and kin) for one another to find at link
 * time, not for any host. Nothing in the Rust can keep them out of the
 * section, so they are taken out of it here; what calls them inside the
 * module is untouched, and the optimizer drops whatever only the export kept.
 */
function exportedOnly(module: URL): void {
  const bytes = Deno.readFileSync(module);
  let at = 8;
  const leb = (): number => {
    let [n, shift] = [0, 0];
    for (;;) {
      const b = bytes[at++];
      n |= (b & 0x7f) << shift;
      if ((b & 0x80) === 0) return n >>> 0;
      shift += 7;
    }
  };
  const put = (n: number): number[] => {
    const out: number[] = [];
    do {
      let b = n & 0x7f;
      n >>>= 7;
      if (n !== 0) b |= 0x80;
      out.push(b);
    } while (n !== 0);
    return out;
  };
  const parts: Uint8Array[] = [bytes.subarray(0, 8)];
  let kept = 0;
  while (at < bytes.length) {
    const start = at;
    const id = bytes[at++];
    const size = leb();
    const end = at + size;
    if (id !== 7) {
      parts.push(bytes.subarray(start, end));
      at = end;
      continue;
    }
    const entries: number[] = [];
    for (let n = leb(); n > 0; n--) {
      const from = at;
      const len = leb();
      const name = new TextDecoder().decode(bytes.subarray(at, at + len));
      at += len + 1;
      leb();
      if (!EXPORTS.has(name)) continue;
      entries.push(...bytes.subarray(from, at));
      kept++;
    }
    const body = [...put(kept), ...entries];
    parts.push(new Uint8Array([7, ...put(body.length), ...body]));
    at = end;
  }
  if (kept !== EXPORTS.size) throw new Error(`[build-wasm] the module exports ${kept} of ${[...EXPORTS].join(", ")}`);
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let k = 0;
  for (const p of parts) {
    out.set(p, k);
    k += p.length;
  }
  Deno.writeFileSync(module, out);
}

/**
 * The source paths a report carries, replaced where they lie by a number —
 * the published shape only.
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
 * the text is unchanged. What goes in its place is `#n` at the end, dots
 * before it: a report names the file by `n`, and PATHS turns `n` back into
 * the path.
 */
/** Where a source path can have come from: this workspace, the toolchain, or
 * a dependency. Anything else ending in `.rs` is not one, and is refused
 * rather than guessed at. */
const SOURCE_ROOT = /(?:^|[^A-Za-z0-9_])(?:pxl|xpute)\/|\/library\/|\/registry\/|^\/rustc\/|^\/rust\/deps\/|\.rustup\//;

function numbered(module: URL): number {
  const bytes = Deno.readFileSync(module);
  const text = new TextDecoder("latin1").decode(bytes);
  const numbers = new Map<string, number>();
  let n = 0;
  for (const m of text.matchAll(/[A-Za-z0-9_/.-]*\.rs\b/g)) {
    // A word that merely ends in those two letters is not a path.
    if (!m[0].includes("/")) continue;
    if (!SOURCE_ROOT.test(m[0])) {
      // Never overwritten on a guess: a `.rs` from somewhere this does not
      // know could be a format string or something a user typed, and
      // overwriting it would be a bug nobody sees until it ships.
      throw new Error(`[build-wasm] ${JSON.stringify(m[0])} ends in .rs but is under no source root — widen SOURCE_ROOT or leave it alone`);
    }
    if (!numbers.has(m[0])) numbers.set(m[0], numbers.size);
    // The number goes at the end and dots before it. The linker merges a
    // string that is another's tail, so a shorter path can lie inside a
    // longer one's bytes and end where it ends: it reads the same number.
    const tag = new TextEncoder().encode(`#${numbers.get(m[0])}`);
    const end = m.index + m[0].length;
    const at = Math.max(m.index, end - tag.length);
    bytes.fill(0x2e, m.index, at);
    bytes.set(tag.subarray(tag.length - (end - at)), at);
    n++;
  }
  if (n > 0) Deno.writeFileSync(module, bytes);
  Deno.writeTextFileSync(PATHS, JSON.stringify(Object.fromEntries([...numbers].map(([path, k]) => [`#${k}`, path])), null, 2) + "\n");
  return n;
}

let status: Deno.CommandStatus;
try {
  status = await cargo.output();
} catch (err) {
  console.error(`[build-wasm] cargo did not run (${err}). The core needs a Rust toolchain with the ${TARGET} target: rustup target add ${TARGET}`);
  Deno.exit(1);
}
if (!status.success) Deno.exit(status.code);

const linkedFile = staged("linked");
const done = staged("done");
await Deno.copyFile(BUILT, linkedFile);
const linked = (await Deno.stat(linkedFile)).size;
exportedOnly(linkedFile);
await optimized(linkedFile, done);
// Only what is published has its paths numbered.
const paths = published ? numbered(done) : 0;
await Deno.rename(done, OUT);
const said = (published ? ", without the dev-only settings" : "") + (paths > 0 ? `, and ${paths} source paths numbered` : "");
console.log(`[build-wasm] ${linked} -> ${(await Deno.stat(OUT)).size} bytes, stripped${said} -> src/kernel/kernel.wasm`);
