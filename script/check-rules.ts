#!/usr/bin/env -S deno run -A
// Holds the tree to what a reader cannot see and the code must keep: no raw
// pointer casts in offset crates, each rounding half to even in every
// language, no CommonJS file, each generated shader made from the sources the
// tree holds now. A rule Rust alone keeps is clippy's (crates/clippy.toml);
// how the tree is written is the convention skill's.
//   deno run -A script/check-rules.ts

import { dirname, fromFileUrl, join } from "@std/path";

import { GENERATED, INPUTS, inputsDigest } from "./shader-wgsl.ts";

const root = dirname(dirname(fromFileUrl(import.meta.url)));

const listed = new TextDecoder().decode(
  (await new Deno.Command("git", { args: ["ls-files", "-co", "--exclude-standard"], cwd: root }).output()).stdout,
);
// The tree as it stands: a file deleted and not yet committed is gone, so
// a removal is checked before it is committed.
const files = listed.split("\n").filter((f) => f && !f.includes("node_modules/") && onDisk(f));

function onDisk(f: string): boolean {
  try {
    return Deno.statSync(join(root, f)).isFile;
  } catch {
    return false;
  }
}

function read(f: string): string | null {
  try {
    return Deno.readTextFileSync(join(root, f));
  } catch {
    return null;
  }
}

const problems: string[] = [];

const OFFSET_CRATES = ["xpute/rust/guest/src/", "crates/client/kernel/src/", "crates/client/render/src/", "crates/client-bracket/wasm/src/", "crates/client-bracket/native/src/"];
const CAST_TO_POINTER = /\bas\s+\*(const|mut)\b/;

for (const f of files) {
  if (!f.endsWith(".rs") || !OFFSET_CRATES.some((c) => f.startsWith(c))) continue;
  read(f)?.split("\n").forEach((line, n) => {
    if (/^\s*\/\//.test(line)) return;
    if (CAST_TO_POINTER.test(line)) problems.push(`${f}:${n + 1}: casts to a pointer where an offset goes through mem::base::ptr_at: ${line.trim()}`);
  });
}

// One rounding: half to even (ARCHITECTURE.md, Terrain). A call that
// rounds otherwise, or a floor of a half added, is refused in pixelet's code
// and xpute's; xpute's scalar keeps its named variants, round_half_up and
// round_half_away, as options a library offers.
const ROUNDING_ROOTS = ["crates/", "apps/", "xpute/"];
const ROUNDING_NAMED = [
  "xpute/rust/kit/src/math/scalar.rs",
  "xpute/rust/kit/src/math/scalar.test.rs",
  "xpute/ts/kit/src/math/scalar.ts",
  "xpute/ts/kit/src/math/scalar.test.ts",
  "xpute/cpp/kit/math/scalar.hpp",
  "xpute/cpp/kit/math/scalar.test.cpp",
];
const NOT_EVEN = [
  /\.round\(\)/,
  /\bf(?:32|64)::round\b/,
  /\blibm::roundf?\(/,
  /\bMath\.round\(/,
  /\bstd::round\b/,
  // A number's text rounds its last place half up: `fixed` (apps/client shell/fixed.ts) rounds it half to even.
  /\.toFixed\(/,
  /\.toPrecision\(/,
  // A half added inside a floor's own parentheses, or before `.floor()`.
  /\bfloor\((?:[^()]|\((?:[^()]|\([^()]*\))*\))*\+\s*0\.5\s*\)/,
  /\([^()]*\+\s*0\.5\s*\)\.floor\(\)/,
];
for (const f of files) {
  if (!ROUNDING_ROOTS.some((r) => f.startsWith(r)) || ROUNDING_NAMED.includes(f) || !/\.(rs|tsx?|wgsl|hpp|cpp)$/.test(f) || f.endsWith(".gen.wgsl")) continue;
  const text = read(f);
  if (text === null) continue;
  text.split("\n").forEach((line, n) => {
    if (NOT_EVEN.some((r) => r.test(line))) problems.push(`${f}:${n + 1}: rounds other than half to even: ${line.trim()}`);
  });
}

// The tree's scripts are ES modules, TypeScript where a loader takes it, the
// bundlers' configs among them: Node runs only the native toolchain's own
// scripts (CocoaPods, Xcode's and Gradle's phases).
for (const f of files) {
  if (/\.c[jt]s$/.test(f)) problems.push(`${f}: CommonJS; the tree's scripts are ES modules`);
}

// A generated shader names the digest of the sources it was made from
// (shader-wgsl.ts `inputsDigest`); one the tree has moved past is stale.
const digest = await inputsDigest(root);
for (const f of GENERATED) {
  const made = read(f)?.split("\n").find((l) => l.startsWith(INPUTS))?.slice(INPUTS.length);
  if (made !== digest) problems.push(`${f}: stale against pxl-spec-core's sources; run script/shader-wgsl.ts`);
}

if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`\ncheck-rules: ${problems.length}`);
  Deno.exit(1);
}
console.log("check-rules: the memory's offsets, one rounding, ES modules and the generated shaders hold");
