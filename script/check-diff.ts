#!/usr/bin/env -S deno run -A
// `deno task check`, narrowed to what changed: a crate when it or a
// dependency changed, TypeScript by what imports what. Against HEAD, the
// working tree's changes; against a base, the commits since it, with only
// what a deploy waits on and the tests, which is how CI gates one.
//
//   deno run -A script/check-diff.ts
//   deno run -A script/check-diff.ts --base <sha>

import { parse } from "@std/jsonc";
import { dirname, fromFileUrl, join } from "@std/path";

const root = dirname(dirname(fromFileUrl(import.meta.url)));

function git(...args: string[]): string[] {
  const out = new Deno.Command("git", { args, cwd: root, stdout: "piped" }).outputSync();
  if (!out.success) throw new Error(`git ${args.join(" ")} failed`);
  return new TextDecoder().decode(out.stdout).split("\n").filter(Boolean);
}

const base = Deno.args[0] === "--base" ? Deno.args[1] : undefined;
// --no-renames: a file moved out of a crate touches the crate it left.
const changed = base
  ? git("diff", "--name-only", "--no-renames", base, "HEAD")
  : [...new Set([...git("diff", "--name-only", "--no-renames", "HEAD"), ...git("ls-files", "--others", "--exclude-standard")])];
const present = changed.filter((path) => {
  try {
    return Deno.statSync(join(root, path)).isFile;
  } catch {
    return false;
  }
});
const under = (prefix: string) => changed.some((path) => path.startsWith(prefix));

interface Package {
  name: string;
  manifest_path: string;
  dependencies: { name: string; path?: string }[];
}

const metadata = new Deno.Command("cargo", {
  args: ["metadata", "--no-deps", "--format-version", "1", "--manifest-path", "crates/Cargo.toml"],
  cwd: root,
  stdout: "piped",
}).outputSync();
if (!metadata.success) throw new Error("cargo metadata failed");
const packages: Package[] = JSON.parse(new TextDecoder().decode(metadata.stdout)).packages;

// The deepest crate a file is under is its own.
const crateDirs = packages
  .map((p) => ({ name: p.name, dir: dirname(p.manifest_path).slice(root.length + 1) + "/" }))
  .sort((a, b) => b.dir.length - a.dir.length);
const touched = new Set<string>();
for (const path of changed) {
  const own = crateDirs.find((c) => path.startsWith(c.dir));
  if (own) touched.add(own.name);
}
// xpute's crates are no member here, but its path dependencies are.
for (const p of packages) {
  for (const d of p.dependencies) {
    if (d.path && changed.some((path) => path.startsWith(d.path!.slice(root.length + 1) + "/"))) touched.add(d.name);
  }
}
let grew = true;
while (grew) {
  grew = false;
  for (const p of packages) {
    if (!touched.has(p.name) && p.dependencies.some((d) => d.path && touched.has(d.name))) {
      touched.add(p.name);
      grew = true;
    }
  }
}

const members = new Set(packages.map((p) => p.name));
const tasks = (parse(await Deno.readTextFile(join(root, "deno.jsonc"))) as { tasks: Record<string, string> }).tasks;

function rust(name: string): string[][] {
  const out: string[][] = [];
  for (const command of tasks[name].split("&&")) {
    const args = command.trim().split(/\s+/);
    const kept: string[] = [];
    let any = false;
    for (let i = 0; i < args.length; i++) {
      if (args[i] === "-p") {
        if (touched.has(args[i + 1])) {
          kept.push("-p", args[i + 1]);
          any = true;
        }
        i++;
      } else if (args[i] === "--all") {
        for (const name of touched) if (members.has(name)) kept.push("-p", name);
        any = kept.includes("-p");
      } else {
        kept.push(args[i]);
      }
    }
    if (any) out.push(kept);
  }
  return out;
}

const runs: { label: string; argv: string[] }[] = [];
const task = (name: string) => runs.push({ label: name, argv: ["deno", "task", name] });

const cargo = (name: string) => {
  for (const argv of rust(name)) runs.push({ label: `${name}: ${argv.slice(0, 2).join(" ")}`, argv });
};
const xpute = under("xpute/ts/");
const web = xpute || under("apps/web/");

// A deploy's gate narrows only over files it can place: one of a crate, the
// web's, xpute's or spec/'s, or one nothing ships. Anything else — a lockfile,
// the workspace's manifest, the toolchain, the import map — runs all of it.
const placed = (path: string) =>
  crateDirs.some((c) => path.startsWith(c.dir)) ||
  /^(apps\/web\/|apps\/native\/|xpute\/(ts|rust)\/[^/]+\/|spec\/|decision\/|\.agents\/|\.github\/|infra\/)/.test(path) ||
  path.endsWith(".md");
if (base && !changed.every(placed)) {
  console.log(`check-diff: ${changed.filter((path) => !placed(path)).join(", ")} reaches everything; the whole gate`);
  for (const name of ["check:ship", "test"]) {
    const status = await new Deno.Command("deno", { args: ["task", name], cwd: root }).spawn().status;
    if (!status.success) Deno.exit(status.code || 1);
  }
  Deno.exit(0);
}

// What a deploy ships.
if (changed.some((path) => /^(rust-toolchain\.toml|\.tool-versions|crates\/.*Dockerfile)$/.test(path))) task("check:toolchain");
if (web) task("check:web");
cargo("check:rust");
if (under("spec/") || changed.some((path) => path.includes(".spec.")) || touched.has("pxl-tool")) task("check:spec");

if (base) {
  // A deploy's tests, of what changed since the last one.
  if (web) task("test:web");
  if (under("xpute/")) task("test:xpute");
  cargo("test:rust");
} else {
  // The rest of `check`, local alone.
  const formatted = present.filter((path) => /\.(ts|tsx|js|json|jsonc|md)$/.test(path));
  if (formatted.length) runs.push({ label: "check:fmt", argv: ["deno", "fmt", "--check", ...formatted] });
  cargo("check:fmt");
  task("check:rules");
  if (web || under("apps/native/")) runs.push({ label: "check:native", argv: ["deno", "check", "apps/native/src"] });
  cargo("check:native");
  cargo("check:host");
  if (xpute) task("check:xpute");
}

console.log(`check-diff: ${changed.length} changed file(s); crates ${[...touched].join(", ") || "none"}`);
for (const { label, argv } of runs) {
  console.log(`check-diff: ${label}`);
  const status = await new Deno.Command(argv[0], { args: argv.slice(1), cwd: root }).spawn().status;
  if (!status.success) {
    console.error(`check-diff: ${label} failed`);
    Deno.exit(status.code || 1);
  }
}
