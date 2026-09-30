#!/usr/bin/env -S deno run -A
// Builds xpute's C++ (cpp/xpute/) with the platform's compiler, as the other
// two languages are built with their own tools: nothing of it goes through
// Cargo, since the C++ is a third implementation beside the Rust and not a
// part of it.
//
//   ./script/cpp.ts test    the tests, built and run (deno task test:cpp)
//   ./script/cpp.ts check   every file compiled, warnings as errors
//                           (deno task check:cpp)
//   ./script/cpp.ts guest   the conformance guest, as the shared library
//                           the host opens (packages/xpute/conformance/)
//
// C++20 with exceptions and RTTI off, as AGENTS.md says a guest is written.
// The compiler is `$CXX`, or `c++`: whichever the platform has, since what
// the code asks of one is the language and not a vendor. Output goes under
// target/cpp/.

import { dirname, fromFileUrl, join, relative } from "@std/path";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");
const TREE = join(ROOT, "cpp/xpute");
const OUT = join(ROOT, "target/cpp");
const CXX = Deno.env.get("CXX") ?? "c++";
// `-ffp-contract=off`: a compiler may otherwise fuse `a + b * c` into one
// fused multiply-add, which rounds once where the source rounds twice. Rust
// and JavaScript never fuse, so a double computed here would stop being the
// bits the other two compute — `lerp` is not, against
// golden/math/scalar.tsv, with contraction on.
const FLAGS = ["-std=c++20", "-fno-exceptions", "-fno-rtti", "-ffp-contract=off", "-Wall", "-Wextra", "-Werror", "-g"];

/** Every `.cpp` under the tree, by its path from the root. */
function sources(dir: string): string[] {
  const out: string[] = [];
  for (const e of Deno.readDirSync(dir)) {
    const path = join(dir, e.name);
    if (e.isDirectory) out.push(...sources(path));
    else if (e.name.endsWith(".cpp")) out.push(relative(ROOT, path));
  }
  return out.sort();
}

/** A test's own file, or the harness the tests run in: neither is part of a
 * guest. */
function testOnly(path: string): boolean {
  return path.endsWith(".test.cpp") || path === "cpp/xpute/core/test.cpp" || path === "cpp/xpute/core/golden.cpp";
}

async function run(cmd: string, args: string[]): Promise<void> {
  const out = await new Deno.Command(cmd, { args, cwd: ROOT, stdout: "inherit", stderr: "inherit" }).output();
  if (!out.success) Deno.exit(out.code || 1);
}

/** Compiles `files` to objects under `dir`, a few at a time. */
async function compile(files: string[], dir: string, extra: string[]): Promise<string[]> {
  await Deno.mkdir(dir, { recursive: true });
  const objects = files.map((f) => join(dir, f.replaceAll("/", "__") + ".o"));
  const queue = files.map((f, i) => [f, objects[i]] as const);
  const workers = Array.from({ length: Math.min(8, queue.length) }, async () => {
    for (let job = queue.shift(); job !== undefined; job = queue.shift()) {
      await run(CXX, [...FLAGS, ...extra, "-c", job[0], "-o", job[1]]);
    }
  });
  await Promise.all(workers);
  return objects;
}

const LIBRARY = Deno.build.os === "darwin" ? "libxpute_conformance.dylib" : Deno.build.os === "windows" ? "xpute_conformance.dll" : "libxpute_conformance.so";

/** The conformance guest's shared library, for a host to open. */
export function guestLibrary(): string {
  return join(OUT, LIBRARY);
}

async function main(mode: string | undefined): Promise<void> {
  const all = sources(TREE);
  switch (mode) {
    case "check":
      await compile(all, join(OUT, "check"), []);
      console.log(`cpp: ${all.length} files compile`);
      return;
    case "test": {
      const objects = await compile(all, join(OUT, "test"), []);
      const binary = join(OUT, "test", "xpute-test");
      await run(CXX, [...objects, "-o", binary]);
      await run(binary, []);
      return;
    }
    case "guest": {
      const objects = await compile(all.filter((f) => !testOnly(f)), join(OUT, "guest"), ["-fPIC"]);
      await run(CXX, ["-shared", ...objects, "-o", guestLibrary()]);
      console.log(`cpp: ${relative(ROOT, guestLibrary())}`);
      return;
    }
    default:
      console.error("usage: script/cpp.ts test | check | guest");
      Deno.exit(2);
  }
}

if (import.meta.main) await main(Deno.args[0]);
