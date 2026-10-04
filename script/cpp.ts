#!/usr/bin/env -S deno run -A
// Builds xpute's C++ with the platform's compiler (`$CXX` or `c++`), and
// the wasm guest with the WASI SDK's clang (`$WASI_SDK_PATH` or
// /opt/wasi-sdk). Output goes under target/cpp/.
//
//   ./script/cpp.ts test | check | guest | wasm

import { dirname, fromFileUrl, join, relative } from "@std/path";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");
const TREE = join(ROOT, "cpp/xpute");
const OUT = join(ROOT, "target/cpp");
const CXX = Deno.env.get("CXX") ?? "c++";
const WASI_CXX = join(Deno.env.get("WASI_SDK_PATH") ?? "/opt/wasi-sdk", "bin/clang++");
// `-ffp-contract=off`: a fused multiply-add rounds once where Rust and JS
// round twice; `lerp` fails golden/math/scalar.tsv with contraction on.
// `-Wno-missing-field-initializers`: a designated initializer leaves the rest
// at their defaults, which GCC's -Wextra calls missing and clang's does not.
const FLAGS = ["-std=c++20", "-fno-exceptions", "-fno-rtti", "-ffp-contract=off", "-Wall", "-Wextra", "-Wno-missing-field-initializers", "-Werror", "-g"];
// 1 MiB, the Rust build's number.
const STACK_BYTES = 1 << 20;
const WASM_LINK = [
  "--target=wasm32-wasip1",
  "-nostartfiles",
  "-Wl,--no-entry",
  "-Wl,--import-memory",
  "-Wl,--stack-first",
  `-Wl,-z,stack-size=${STACK_BYTES}`,
  "-Wl,--export=xpute_conformance_boot",
  "-Wl,--export=xpute_conformance_interrupt",
];

function sources(dir: string): string[] {
  const out: string[] = [];
  for (const e of Deno.readDirSync(dir)) {
    const path = join(dir, e.name);
    if (e.isDirectory) out.push(...sources(path));
    else if (e.name.endsWith(".cpp")) out.push(relative(ROOT, path));
  }
  return out.sort();
}

function testOnly(path: string): boolean {
  return path.endsWith(".test.cpp") || path === "cpp/xpute/core/test.cpp" || path === "cpp/xpute/core/golden.cpp";
}

async function run(cmd: string, args: string[]): Promise<void> {
  const out = await new Deno.Command(cmd, { args, cwd: ROOT, stdout: "inherit", stderr: "inherit" }).output();
  if (!out.success) Deno.exit(out.code || 1);
}

async function compile(cxx: string, files: string[], dir: string, extra: string[]): Promise<string[]> {
  await Deno.mkdir(dir, { recursive: true });
  const objects = files.map((f) => join(dir, f.replaceAll("/", "__") + ".o"));
  const queue = files.map((f, i) => [f, objects[i]] as const);
  const workers = Array.from({ length: Math.min(8, queue.length) }, async () => {
    for (let job = queue.shift(); job !== undefined; job = queue.shift()) {
      await run(cxx, [...FLAGS, ...extra, "-c", job[0], "-o", job[1]]);
    }
  });
  await Promise.all(workers);
  return objects;
}

const LIBRARY = Deno.build.os === "darwin" ? "libxpute_conformance.dylib" : Deno.build.os === "windows" ? "xpute_conformance.dll" : "libxpute_conformance.so";

async function main(mode: string | undefined): Promise<void> {
  const all = sources(TREE);
  switch (mode) {
    case "check":
      await compile(CXX, all, join(OUT, "check"), []);
      console.log(`cpp: ${all.length} files compile`);
      return;
    case "test": {
      const objects = await compile(CXX, all, join(OUT, "test"), []);
      const binary = join(OUT, "test", "xpute-test");
      await run(CXX, [...objects, "-o", binary]);
      await run(binary, []);
      return;
    }
    case "guest": {
      const objects = await compile(CXX, all.filter((f) => !testOnly(f)), join(OUT, "guest"), ["-fPIC"]);
      const library = join(OUT, LIBRARY);
      await run(CXX, ["-shared", ...objects, "-o", library]);
      console.log(`cpp: ${relative(ROOT, library)}`);
      return;
    }
    case "wasm": {
      try {
        Deno.statSync(WASI_CXX);
      } catch {
        console.error(`cpp: the WASI SDK's clang++ is not at ${WASI_CXX}; install the SDK (github.com/WebAssembly/wasi-sdk) at /opt/wasi-sdk, or set WASI_SDK_PATH to where it is`);
        Deno.exit(1);
      }
      const objects = await compile(WASI_CXX, all.filter((f) => !testOnly(f)), join(OUT, "wasm"), ["--target=wasm32-wasip1"]);
      const module = join(OUT, "xpute_conformance.wasm");
      await run(WASI_CXX, [...WASM_LINK, ...objects, "-o", module]);
      console.log(`cpp: ${relative(ROOT, module)}`);
      return;
    }
    default:
      console.error("usage: script/cpp.ts test | check | guest | wasm");
      Deno.exit(2);
  }
}

if (import.meta.main) await main(Deno.args[0]);
