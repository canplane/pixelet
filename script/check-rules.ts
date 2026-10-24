#!/usr/bin/env -S deno run -A
// Holds the tree to what a machine decides exactly: no CommonJS file, and
// each generated shader made from the sources the tree holds now. Rust's
// rounding is clippy's (crates/clippy.toml); how the tree is written is
// CONVENTION.md's.
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
console.log("check-rules: ES modules and the generated shaders hold");
