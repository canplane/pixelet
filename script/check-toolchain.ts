#!/usr/bin/env -S deno run -A
// Holds the Rust version in the server images' base tags to
// rust-toolchain.toml: a drifted image builds a binary the gate never
// compiled.
//
//   deno run -A script/check-toolchain.ts

import { dirname, fromFileUrl, join } from "@std/path";

const root = dirname(dirname(fromFileUrl(import.meta.url)));

const toolchain = await Deno.readTextFile(join(root, "rust-toolchain.toml"));
const channel = toolchain.match(/^\s*channel\s*=\s*"([^"]+)"/m)?.[1];
if (!channel) throw new Error("rust-toolchain.toml: no [toolchain] channel to check anything against");

const IMAGES = [
  "crates/pxl/server/geo/Dockerfile",
  "crates/pxl/server/api/Dockerfile",
];

const wrong: string[] = [];
for (const path of IMAGES) {
  const text = await Deno.readTextFile(join(root, path));
  const tag = text.match(/^FROM\s+rust:([^\s-]+)-/m)?.[1];
  if (!tag) {
    wrong.push(`${path}: no \`FROM rust:<version>-\` line, so nothing here is pinned to anything`);
    continue;
  }
  if (tag !== channel) wrong.push(`${path}: builds on rust:${tag} where rust-toolchain.toml says ${channel}`);
}

if (wrong.length > 0) {
  console.error("[check-toolchain] the tree does not agree on which Rust it is:");
  for (const line of wrong) console.error(`  ${line}`);
  console.error("\nrust-toolchain.toml is the one that decides; an image tag follows it.");
  Deno.exit(1);
}

// Deno against .tool-versions: a warning only, since CI installs from the
// file anyway.
const tools = await Deno.readTextFile(join(root, ".tool-versions"));
const want = tools.match(/^deno\s+v?(\S+)$/m)?.[1];
if (!want) throw new Error(".tool-versions: no `deno <version>` line for the gate to hold anything to");

if (want !== Deno.version.deno) {
  console.warn(`[check-toolchain] deno ${Deno.version.deno} here where .tool-versions says ${want} — \`deno upgrade --version ${want}\``);
} else {
  console.log(`[check-toolchain] ok — deno ${want}, as .tool-versions says`);
}

console.log(`[check-toolchain] ok — rust ${channel}, in rust-toolchain.toml and in ${IMAGES.length} image(s)`);
