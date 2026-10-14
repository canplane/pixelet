#!/usr/bin/env -S deno run -A
// Holds the tree to the rules a machine can decide: the header
// path, cited paths and names existing, the lattice's arithmetic only in
// `space/`, no raw pointer casts in offset crates, `@expo/ui` only in the
// sheet. `doc/decision/`, `doc/experiment/` and `doc/discussion/` are exempt
// from cited paths: each names the files of its day.
//   deno run -A script/check-rules.ts

import { dirname, fromFileUrl, join, normalize } from "@std/path";

const root = dirname(dirname(fromFileUrl(import.meta.url)));

const listed = new TextDecoder().decode(
  (await new Deno.Command("git", { args: ["ls-files", "-co", "--exclude-standard"], cwd: root }).output()).stdout,
);
const files = listed.split("\n").filter((f) => f && !f.includes("node_modules/"));

function read(f: string): string | null {
  try {
    return Deno.readTextFileSync(join(root, f));
  } catch {
    return null; // listed but deleted in the working tree
  }
}

const crates = new Map<string, string>();
for (const f of files.filter((f) => f.endsWith("Cargo.toml"))) {
  const name = read(f)?.match(/^\[package\][^[]*?^name\s*=\s*"([^"]+)"/ms)?.[1];
  if (name) crates.set(name, f.slice(0, -"Cargo.toml".length));
}

const problems: string[] = [];

function header(f: string): string | null {
  if (f.endsWith(".rs")) {
    for (let dir = dirname(f); dir !== "."; dir = dirname(dir)) {
      const name = [...crates].find(([, d]) => d === `${dir}/`)?.[0];
      if (!name) continue;
      const src = `${dir}/src/`;
      return f.startsWith(src) ? `${name}/${f.slice(src.length)}` : `${name}/${f.slice(dir.length + 1)}`;
    }
    return null;
  }
  if (/^xpute\/cpp\/.*\.(cpp|hpp)$/.test(f)) return f.slice("xpute/".length);
  if (!/\.tsx?$/.test(f)) return null;
  const app = f.match(/^apps\/([\w-]+)\/(?:src\/)?(.*)$/);
  if (app) return `@pxl/${app[1]}/${app[2]}`;
  const pkg = f.match(/^xpute\/ts\/(\w+)\/src\/(.*)$/);
  return pkg ? `@xpute/${pkg[1]}/${pkg[2]}` : null;
}

for (const f of files) {
  const want = header(f);
  const text = want && read(f);
  if (!text || text.startsWith("#!")) continue;
  const first = text.slice(0, text.indexOf("\n"));
  if (first !== `// ${want}`) problems.push(`${f}:1: opens with "${first}", not "// ${want}"`);
}

const suffixes = new Set<string>();
for (const f of files) {
  const parts = f.split("/");
  for (let i = 0; i < parts.length; i++) suffixes.add(parts.slice(i).join("/"));
}

function exists(path: string, from: string): boolean {
  if (path.startsWith("../")) return suffixes.has(normalize(join(dirname(from), path)));
  if (path.startsWith("./")) return suffixes.has(normalize(join(dirname(from), path))) || suffixes.has(path.slice(2));
  if (suffixes.has(path)) return true;
  const [head, ...rest] = path.split("/");
  const tail = rest.join("/");
  if (head === "@pxl") {
    const [app, ...within] = rest;
    return suffixes.has(`apps/${app}/src/${within.join("/")}`) || suffixes.has(`apps/${app}/${within.join("/")}`);
  }
  if (head === "@") return suffixes.has(`apps/web/src/${tail}`);
  if (head === "@xpute") return suffixes.has(`xpute/ts/${rest[0]}/src/${rest.slice(1).join("/")}`);
  const dir = crates.get(head);
  if (dir && (suffixes.has(`${dir}src/${tail}`) || suffixes.has(`${dir}${tail}`))) return true;
  // By what git ignores, not disk: CI runs this before any build.
  return [join(dirname(from), path), path, `apps/web/src/${path}`].some((p) => {
    try {
      if (Deno.statSync(join(root, p)).isFile) return true;
    } catch { /* not built here */ }
    return new Deno.Command("git", { args: ["check-ignore", "-q", p], cwd: root }).outputSync().code === 0;
  });
}

const CITED = /(?<![\w/.:@-])((?:\.{1,2}\/)*@?(?:[\w.@-]+\/)*[\w-]+(?:\.[\w-]+)*\.(?:rs|ts|tsx|wgsl))(?![\w/])/g;

function resolve(path: string, from: string): string[] {
  const tails = new Set<string>();
  if (path.startsWith("./") || path.startsWith("../")) tails.add(normalize(join(dirname(from), path)));
  tails.add(path.replace(/^\.\//, ""));
  const [head, ...rest] = path.split("/");
  const tail = rest.join("/");
  if (head === "@pxl") {
    const [app, ...within] = rest;
    tails.add(`apps/${app}/src/${within.join("/")}`).add(`apps/${app}/${within.join("/")}`);
  }
  if (head === "@") tails.add(`apps/web/src/${tail}`);
  if (head === "@xpute") tails.add(`xpute/ts/${rest[0]}/src/${rest.slice(1).join("/")}`);
  const dir = crates.get(head);
  if (dir) tails.add(`${dir}src/${tail}`).add(`${dir}${tail}`);
  return files.filter((f) => [...tails].some((t) => f === t || f.endsWith(`/${t}`)));
}

const NAMED = /^`?\s+`([A-Za-z_][\w]*(?:::[A-Za-z_][\w]*)*)`/;
const PROSE = /\.(rs|tsx?|wgsl|md|jsonc?)$/;

for (const f of files) {
  if (/^doc\/(decision|experiment|discussion)\//.test(f) || !PROSE.test(f)) continue;
  const text = read(f);
  if (!text) continue;
  const doc = f.endsWith(".md") || f.endsWith(".json") || f.endsWith(".jsonc");
  text.split("\n").forEach((line, n) => {
    if (!doc && !/^\s*(\/\/|\/?\*)/.test(line)) return;
    if (line.includes("://")) return;
    for (const m of line.matchAll(CITED)) {
      const path = m[1];
      if (!exists(path, f)) {
        problems.push(`${f}:${n + 1}: names ${path}, which is no file here`);
        continue;
      }
      const name = line.slice(m.index! + path.length).match(NAMED)?.[1]?.split("::").pop();
      const within = name ? resolve(path, f) : [];
      if (name && within.length > 0 && !within.some((w) => new RegExp(`\\b${name}\\b`).test(read(w) ?? ""))) {
        problems.push(`${f}:${n + 1}: names ${name} in ${path}, which has no ${name}`);
      }
    }
  });
}

const LATTICE_OWNERS = ["crates/spec/src/space/", "crates/spec/src/bake/constant.rs"];
const SHIFT = /(\b(PAGE_CELLS|CHUNK_CELLS)\b|\bshamt\()[^;]*(<<|>>)|(<<|>>)[^;]*\bshamt\(/;
const IN_PAGE = /[%/]\s*PAGE_CELLS\b|(rem|div)_euclid\(\s*PAGE_CELLS\b|\*\s*PAGE_CELLS\b(\s+as\s+\w+)?\s*\+/;

for (const f of files) {
  if (!f.endsWith(".rs") || LATTICE_OWNERS.some((o) => f.startsWith(o))) continue;
  read(f)?.split("\n").forEach((line, n) => {
    if (/^\s*\/\//.test(line)) return;
    if (SHIFT.test(line)) problems.push(`${f}:${n + 1}: shifts the lattice outside pxl-spec space/: ${line.trim()}`);
    if (IN_PAGE.test(line)) problems.push(`${f}:${n + 1}: numbers a page's cells outside pxl-spec space/: ${line.trim()}`);
  });
}

const OFFSET_CRATES = ["xpute/rust/guest/src/", "crates/client/kernel/src/", "crates/client/render/src/", "crates/client-bracket/wasm/src/", "crates/client-bracket/native/src/"];
const CAST_TO_POINTER = /\bas\s+\*(const|mut)\b/;

for (const f of files) {
  if (!f.endsWith(".rs") || !OFFSET_CRATES.some((c) => f.startsWith(c))) continue;
  read(f)?.split("\n").forEach((line, n) => {
    if (/^\s*\/\//.test(line)) return;
    if (CAST_TO_POINTER.test(line)) problems.push(`${f}:${n + 1}: casts to a pointer where an offset goes through mem::base::ptr_at: ${line.trim()}`);
  });
}

const SWIFTUI_OWNERS = ["apps/native/src/platform/panel.tsx"];

for (const f of files) {
  if (!/\.tsx?$/.test(f) || !f.startsWith("apps/") || SWIFTUI_OWNERS.includes(f)) continue;
  read(f)?.split("\n").forEach((line, n) => {
    if (/^\s*import\b.*["']@expo\/ui/.test(line)) problems.push(`${f}:${n + 1}: imports @expo/ui outside the sheet, where a control that takes a touch is UIKit's`);
  });
}

if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`\ncheck-rules: ${problems.length}`);
  Deno.exit(1);
}
console.log("check-rules: headers, cited paths, the lattice, the memory's offsets and SwiftUI's place hold");
