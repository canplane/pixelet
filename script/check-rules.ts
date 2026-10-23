#!/usr/bin/env -S deno run -A
// Holds the tree to the rules a machine can decide: the header
// path, cited paths and names existing, the lattice's arithmetic only in
// `space/`, no raw pointer casts in offset crates, `@expo/ui` only in the
// sheet, each rounding half to even, no CommonJS, each generated shader made
// from the sources the tree holds now.
// `doc/minutes/` and `doc/experiment/` are exempt from cited paths: each
// names the files of its day.
//   deno run -A script/check-rules.ts

import { dirname, fromFileUrl, join, normalize } from "@std/path";

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

/** Where `@/` points from a file: its own package's source, or, cited from
 * outside the apps, any of theirs. */
function ownSources(from: string): string[] {
  const app = from.match(/^apps\/([\w-]+)\//);
  return app ? [`apps/${app[1]}/src/`] : ["apps/client/src/", "apps/web/src/", "apps/native/src/"];
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
  if (head === "@") return ownSources(from).some((src) => suffixes.has(`${src}${tail}`));
  if (head === "@xpute") return suffixes.has(`xpute/ts/${rest[0]}/src/${rest.slice(1).join("/")}`);
  const dir = crates.get(head);
  if (dir && (suffixes.has(`${dir}src/${tail}`) || suffixes.has(`${dir}${tail}`))) return true;
  // By what git ignores, not disk: CI runs this before any build.
  return [join(dirname(from), path), path, `apps/client/src/${path}`, `apps/web/src/${path}`].some((p) => {
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
  if (head === "@") { for (const src of ownSources(from)) tails.add(`${src}${tail}`); }
  if (head === "@xpute") tails.add(`xpute/ts/${rest[0]}/src/${rest.slice(1).join("/")}`);
  const dir = crates.get(head);
  if (dir) tails.add(`${dir}src/${tail}`).add(`${dir}${tail}`);
  return files.filter((f) => [...tails].some((t) => f === t || f.endsWith(`/${t}`)));
}

const NAMED = /^`?\s+`([A-Za-z_][\w]*(?:::[A-Za-z_][\w]*)*)`/;
const PROSE = /\.(rs|tsx?|wgsl|md|jsonc?)$/;

for (const f of files) {
  if (/^doc\/(minutes|experiment)\//.test(f) || !PROSE.test(f)) continue;
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
const SHIFT = /\bshamt\([^;]*(<<|>>)|(<<|>>)[^;]*\bshamt\(/;

for (const f of files) {
  if (!f.endsWith(".rs") || LATTICE_OWNERS.some((o) => f.startsWith(o))) continue;
  read(f)?.split("\n").forEach((line, n) => {
    if (/^\s*\/\//.test(line)) return;
    if (SHIFT.test(line)) problems.push(`${f}:${n + 1}: shifts the lattice outside pxl-spec space/: ${line.trim()}`);
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

const SWIFTUI_OWNERS = ["apps/client/src/shell/ui/panel.ios.tsx"];

for (const f of files) {
  if (!/\.tsx?$/.test(f) || !f.startsWith("apps/") || SWIFTUI_OWNERS.includes(f)) continue;
  read(f)?.split("\n").forEach((line, n) => {
    if (/^\s*import\b.*["']@expo\/ui/.test(line)) problems.push(`${f}:${n + 1}: imports @expo/ui outside the sheet, where a control that takes a touch is UIKit's`);
  });
}

// One rounding: half to even (ARCHITECTURE.md, Terrain). A call that
// rounds otherwise, or a floor of a half added, is refused in pixelet's code
// and xpute's; xpute's scalar keeps its named variants, round_half_up and
// round_half_away, as options a library offers.
const ROUNDING_ROOTS = ["crates/", "apps/", "crates/client/shader/", "xpute/"];
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
// scripts (CocoaPods, Xcode's and Gradle's phases), and what of the tree it
// loads there it loads as an ES module.
const COMMONJS = [
  /(?<![\w$.])require(?:\s*\.\s*resolve)?\s*\(/,
  /(?<![\w$.])module\s*(?:\.\s*exports\b|\[)/,
  /(?<![\w$.])exports\s*(?:\.\s*[\w$]+\s*=|\[)/,
  /\bdefineProperty\(\s*exports\b/,
  /\bcreateRequire\b/,
];
for (const f of files) {
  if (/\.c[jt]s$/.test(f)) problems.push(`${f}: CommonJS; the tree's scripts are ES modules`);
  // This file names the patterns it refuses.
  if (!/\.[mc]?[jt]sx?$/.test(f) || f === "script/check-rules.ts") continue;
  // Code alone: a comment's text and a string's are not CommonJS. A block
  // comment is followed across lines; a string or a template within one.
  let inComment = false;
  read(f)?.split("\n").forEach((line, n) => {
    let code = line;
    if (inComment) {
      const end = code.indexOf("*/");
      code = end < 0 ? "" : code.slice(end + 2);
      inComment = end < 0;
    }
    code = code.replace(/\/\*.*?\*\/|\/\/.*$|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`/g, "");
    const open = code.indexOf("/*");
    if (open >= 0) [code, inComment] = [code.slice(0, open), true];
    if (COMMONJS.some((r) => r.test(code))) problems.push(`${f}:${n + 1}: CommonJS where the tree's scripts are ES modules: ${line.trim()}`);
  });
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
console.log("check-rules: headers, cited paths, the lattice, the memory's offsets, SwiftUI's place, one rounding, ES modules and the generated shaders hold");
