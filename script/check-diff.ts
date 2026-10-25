#!/usr/bin/env -S deno run -A
// `deno task check` and `deno task test`, narrowed to the working tree's
// changes: a crate when it or a dependency changed, TypeScript by what
// imports what. One selection drives both.
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

// --no-renames: a file moved out of a crate touches the crate it left.
const changed = [...new Set([...git("diff", "--name-only", "--no-renames", "HEAD"), ...git("ls-files", "--others", "--exclude-standard")])];
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
  args: ["metadata", "--no-deps", "--format-version", "1", "--manifest-path", "rust/Cargo.toml"],
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
      } else if (args[i] === "--features" && args[i + 1]?.includes("/")) {
        // A selected package's feature, which goes when the package does.
        if (touched.has(args[i + 1].split("/")[0])) kept.push(args[i], args[i + 1]);
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
// ts/client imports xpute's TypeScript, and ts/app/web both.
const xpute = under("xpute/");
const client = under("ts/client/") || under("xpute/ts/");
const web = client || under("ts/app/web/");

// What ships.
if (web) task("check:web");
if (under("script/") || under("ts/app/web/script/")) task("check:script");
cargo("check:rust");
if (under("spec/") || changed.some((path) => path.includes(".spec.")) || touched.has("pxl-tool")) task("check:spec");
if (web || touched.has("pxl-wasm")) runs.push({ label: "check:build", argv: ["deno", "task", "--cwd", "ts/app/web", "check:build"] });

// The rest of `check`.
const formatted = present.filter((path) => /\.(ts|tsx|js|json|jsonc|md)$/.test(path));
if (formatted.length) runs.push({ label: "check:fmt", argv: ["deno", "fmt", "--check", ...formatted] });
cargo("check:fmt");
task("check:rules");
task("lint");
if (web || under("ts/app/native/")) runs.push({ label: "check:native", argv: ["deno", "check", "ts/app/native/src"] });
cargo("check:native");
cargo("check:host");
if (xpute) task("check:xpute");

// The tests of what the same selection reached.
if (client) task("test:client");
if (xpute) task("test:xpute");
cargo("test:rust");

console.log(`check-diff: ${changed.length} changed file(s); crates ${[...touched].join(", ") || "none"}`);
for (const { label, argv } of runs) {
  console.log(`check-diff: ${label}`);
  const status = await new Deno.Command(argv[0], { args: argv.slice(1), cwd: root }).spawn().status;
  if (!status.success) {
    console.error(`check-diff: ${label} failed`);
    Deno.exit(status.code || 1);
  }
}
