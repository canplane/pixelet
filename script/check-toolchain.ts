#!/usr/bin/env -S deno run -A
// Holds every hand-written Rust version in the tree to rust-toolchain.toml.
//
// rustup reads that file and nothing else needs to be told: a cargo command
// here uses it whatever `rustup default` is, and CI installs from it with no
// version of its own. A base image tag is the exception, because Docker
// chooses it before the build context exists, so the two server images spell
// the version out a second time. A second spelling drifts — that is what this
// is for. The failure it prevents is quiet: the image builds fine on the
// wrong compiler and publishes a binary nothing in the gate ever compiled.
//
// The same shape as script/spec.ts's checks of wrangler.jsonc and
// check-bundle.ts: read what was written by hand, compare it to the one
// source, and say which is which when they disagree.
//
//   deno run -A script/check-toolchain.ts

import { dirname, fromFileUrl, join } from "@std/path";

const root = dirname(dirname(fromFileUrl(import.meta.url)));

const toolchain = await Deno.readTextFile(join(root, "rust-toolchain.toml"));
const channel = toolchain.match(/^\s*channel\s*=\s*"([^"]+)"/m)?.[1];
if (!channel) throw new Error("rust-toolchain.toml: no [toolchain] channel to check anything against");

/** Where the version is written a second time, and what writes it there. */
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

console.log(`[check-toolchain] ok — rust ${channel}, in rust-toolchain.toml and in ${IMAGES.length} image(s)`);
