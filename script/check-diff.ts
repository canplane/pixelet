#!/usr/bin/env -S deno run -A
// The gate, held to what the working tree changed against HEAD: the checks
// `deno task check` runs, each run only where a changed file can break it.
//
// A crate is checked when it or a crate it depends on changed, since an edit
// to pxl-spec can break pxl-kernel without touching it. The clippy runs are
// check:rust's own, read out of deno.jsonc and narrowed to those crates, so
// the targets and features each crate is checked under have one source. The
// TypeScript follows what imports what: the shell is the device app's too,
// and @xpute/ is under both.
//
// Run when asked; the whole gate is still what runs before a deploy.
//
//   deno run -A script/check-diff.ts

import { parse } from "@std/jsonc";
import { dirname, fromFileUrl, join } from "@std/path";

const root = dirname(dirname(fromFileUrl(import.meta.url)));

function git(...args: string[]): string[] {
  const out = new Deno.Command("git", { args, cwd: root, stdout: "piped" }).outputSync();
  if (!out.success) throw new Error(`git ${args.join(" ")} failed`);
  return new TextDecoder().decode(out.stdout).split("\n").filter(Boolean);
}

const changed = [...new Set([...git("diff", "--name-only", "HEAD"), ...git("ls-files", "--others", "--exclude-standard")])];
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

// The deepest crate a file is under is its own: pxl/server holds
// pxl/server/geo's directory too.
const crateDirs = packages
  .map((p) => ({ name: p.name, dir: dirname(p.manifest_path).slice(root.length + 1) + "/" }))
  .sort((a, b) => b.dir.length - a.dir.length);
const touched = new Set<string>();
for (const path of changed) {
  const own = crateDirs.find((c) => path.startsWith(c.dir));
  if (own) touched.add(own.name);
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

const tasks = (parse(await Deno.readTextFile(join(root, "deno.jsonc"))) as { tasks: Record<string, string> }).tasks;

/** check:rust's commands, each kept for the crates of it that are touched. */
function rust(): string[][] {
  const out: string[][] = [];
  for (const command of tasks["check:rust"].split("&&")) {
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
        // cargo fmt over the touched crates rather than the workspace.
        for (const name of touched) kept.push("-p", name);
        any = touched.size > 0;
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

const formatted = present.filter((path) => /\.(ts|tsx|js|json|jsonc|md)$/.test(path));
if (formatted.length) runs.push({ label: "check:fmt", argv: ["deno", "fmt", "--check", ...formatted] });
if (changed.some((path) => /^(rust-toolchain\.toml|\.tool-versions|crates\/.*Dockerfile)$/.test(path))) task("check:toolchain");
task("check:rules");
const xpute = under("packages/xpute/");
if (xpute || under("apps/web/")) task("check:web");
if (xpute || under("apps/web/") || under("apps/native/")) task("check:native");
if (xpute) task("check:xpute");
for (const argv of rust()) runs.push({ label: argv.slice(0, 2).join(" "), argv });
if (under("spec/") || changed.some((path) => path.includes(".spec.")) || touched.has("pxl-tool")) task("check:spec");

console.log(`check-diff: ${changed.length} changed file(s); crates ${[...touched].join(", ") || "none"}`);
for (const { label, argv } of runs) {
  console.log(`check-diff: ${label}`);
  const status = await new Deno.Command(argv[0], { args: argv.slice(1), cwd: root }).spawn().status;
  if (!status.success) {
    console.error(`check-diff: ${label} failed`);
    Deno.exit(status.code || 1);
  }
}
