#!/usr/bin/env -S deno run -A
// Builds the Rust core's module (crates/pxl/web/wasm) to where
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

const EXPORTS = new Set(["interrupt"]);

/** embassy and critical-section export their `#[no_mangle]` symbols; only EXPORTS stays. */
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

/** Rust embeds caller paths for `#[track_caller]` panics with no stable way
 * out. They are overwritten in place, same length, as `...#n`; a `&'static
 * str` keeps its length apart, so the module runs the same. */
const SOURCE_ROOT = /(?:^|[^A-Za-z0-9_])(?:pxl|xpute)\/|\/library\/|\/registry\/|^\/rustc\/|^\/rust\/deps\/|\.rustup\//;

function numbered(module: URL): number {
  const bytes = Deno.readFileSync(module);
  const text = new TextDecoder("latin1").decode(bytes);
  const numbers = new Map<string, number>();
  let n = 0;
  for (const m of text.matchAll(/[A-Za-z0-9_/.-]*\.rs\b/g)) {
    if (!m[0].includes("/")) continue;
    if (!SOURCE_ROOT.test(m[0])) {
      // Never overwrite on a guess: it could be a format string.
      throw new Error(`[build-wasm] ${JSON.stringify(m[0])} ends in .rs but is under no source root — widen SOURCE_ROOT or leave it alone`);
    }
    if (!numbers.has(m[0])) numbers.set(m[0], numbers.size);
    // The number goes last: the linker merges a string that is another's tail.
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
exportedOnly(linkedFile);
await optimized(linkedFile, done);
const paths = published ? numbered(done) : 0;
await Deno.rename(done, OUT);
const said = (published ? ", without the dev-only settings" : "") + (paths > 0 ? `, and ${paths} source paths numbered` : "");
console.log(`[build-wasm] ${linked} -> ${(await Deno.stat(OUT)).size} bytes, stripped${said} -> src/kernel/kernel.wasm`);
