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
  args: ["metadata", "--no-deps", "--format-version", "1"],
  cwd: join(root, "rust"),
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

/** rust/.cargo/config.toml's aliases, each the arguments it stands for: one
 * array of strings a line, as the file writes them. */
const aliases = new Map<string, string[]>();
for (const line of (await Deno.readTextFile(join(root, "rust/.cargo/config.toml"))).split("\n")) {
  const m = line.match(/^([a-z-]+) = (\[.*\])$/);
  if (m) aliases.set(m[1], JSON.parse(m[2]));
}

/** A root task's cargo commands, each with an alias spelled out and its
 * packages narrowed to the touched ones; none that names no touched one. */
function rust(name: string): string[][] {
  const out: string[][] = [];
  for (const command of tasks[name].split("&&")) {
    let args = command.trim().split(/\s+/);
    if (args[0] !== "cargo") continue;
    const alias = aliases.get(args[1]);
    if (alias) args = ["cargo", ...alias, ...args.slice(2)];
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

const runs: { label: string; argv: string[]; cwd?: string }[] = [];
const task = (name: string) => runs.push({ label: name, argv: ["deno", "task", name] });

const cargo = (name: string) => {
  for (const argv of rust(name)) runs.push({ label: `${name}: ${argv.slice(0, 2).join(" ")}`, argv, cwd: join(root, "rust") });
};
// ts/client imports xpute's TypeScript, and every app and the Worker the client.
const xpute = under("xpute/");
const ts = under("ts/") || under("xpute/ts/");

if (ts) task("check:ts");
if (under("script/")) task("check:script");
cargo("check:rust");
if (under("spec/") || changed.some((path) => path.includes(".spec.")) || touched.has("pxl-tool")) task("check:spec");
if (under("ts/client/") || under("ts/app/web/") || under("ts/server/edge/") || under("xpute/ts/") || touched.has("pxl-wasm")) task("check:build");
// Each formatted file under the root that formats it: ts/ and script/ their own, the documents the root's.
const formatted = present.filter((path) => /\.(ts|tsx|js|json|jsonc|md)$/.test(path) && !path.startsWith("xpute/"));
for (const tree of ["ts/", "script/", ""]) {
  const own = formatted.filter((path) => path.startsWith(tree) && (tree !== "" || !/^(ts|script)\//.test(path)));
  if (own.length) runs.push({ label: `fmt ${tree || "doc"}`, argv: ["deno", "fmt", "--check", ...own.map((path) => path.slice(tree.length))], cwd: join(root, tree) });
}
task("check:rules");
if (xpute) task("check:xpute");

// The tests of what the same selection reached.
if (under("ts/client/") || under("xpute/ts/")) task("test:ts");
if (xpute) task("test:xpute");
cargo("test:rust");

console.log(`check-diff: ${changed.length} changed file(s); crates ${[...touched].join(", ") || "none"}`);
for (const { label, argv, cwd } of runs) {
  console.log(`check-diff: ${label}`);
  const status = await new Deno.Command(argv[0], { args: argv.slice(1), cwd: cwd ?? root }).spawn().status;
  if (!status.success) {
    console.error(`check-diff: ${label} failed`);
    Deno.exit(status.code || 1);
  }
}
