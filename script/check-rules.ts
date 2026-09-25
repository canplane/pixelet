#!/usr/bin/env -S deno run -A
// Holds the tree to the rules in AGENTS.md that a machine can decide, so that
// what drifts without anyone deciding it is caught where it drifts.
//
// Three of them, each a rule that went wrong by accident more than once and
// none a judgement about design:
//
// - A source file opens with its own logical path. The comment is how a file
//   is named in every other comment, so a wrong one sends readers elsewhere.
// - A source path a comment or a document names exists. A file renamed or
//   deleted leaves its citations behind; eleven still named the terrain
//   level module weeks after the pyramid window replaced it.
// - The lattice's arithmetic is the space model's. A level's cells, chunks
//   and pages are 2^level of the leaf's, and every size is a `Scale` stepped
//   and counted in `pxl-spec space/`; a shift by a tier or by a scale's own
//   exponent written in a layer is a second owner of the lattice, and every
//   one found had drifted or duplicated a constant. So is a cell's place in
//   its page worked out by hand: the lattice numbers a page's cells, and
//   that number is the index into every lane a page has (`cell_index`,
//   `in_page`), where seven layers had each spelled it out again.
//
// `issue/` is exempt from the second: a decision record names the files of
// its day.
//
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

/** Each crate's directory by its package name, as Cargo.toml spells it. */
const crates = new Map<string, string>();
for (const f of files.filter((f) => f.endsWith("Cargo.toml"))) {
  const name = read(f)?.match(/^\[package\][^[]*?^name\s*=\s*"([^"]+)"/ms)?.[1];
  if (name) crates.set(name, f.slice(0, -"Cargo.toml".length));
}

const problems: string[] = [];

// ---- the header ----

/** The path a file's first line names, or null where the rule has none. */
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
  if (!/\.tsx?$/.test(f) || f.includes("/shadcn/")) return null;
  if (f.startsWith("apps/web/src/")) return `@pxl/web/${f.slice("apps/web/src/".length)}`;
  if (f.startsWith("apps/web/")) return `@pxl/web/${f.slice("apps/web/".length)}`;
  const pkg = f.match(/^packages\/xpute\/(\w+)\/src\/(.*)$/);
  return pkg ? `@xpute/${pkg[1]}/${pkg[2]}` : null;
}

for (const f of files) {
  const want = header(f);
  const text = want && read(f);
  if (!text || text.startsWith("#!")) continue; // an executable script opens on its interpreter
  const first = text.slice(0, text.indexOf("\n"));
  if (first !== `// ${want}`) problems.push(`${f}:1: opens with "${first}", not "// ${want}"`);
}

// ---- the paths a comment names ----

const suffixes = new Set<string>();
for (const f of files) {
  const parts = f.split("/");
  for (let i = 0; i < parts.length; i++) suffixes.add(parts.slice(i).join("/"));
}

/** Whether `path`, as `from` cites it, names a file of the tree. */
function exists(path: string, from: string): boolean {
  // Relative to the citing file, or, in a document or a config, to the root.
  if (path.startsWith("../")) return suffixes.has(normalize(join(dirname(from), path)));
  if (path.startsWith("./")) return suffixes.has(normalize(join(dirname(from), path))) || suffixes.has(path.slice(2));
  if (suffixes.has(path)) return true;
  const [head, ...rest] = path.split("/");
  const tail = rest.join("/");
  if (head === "@pxl") return suffixes.has(`apps/web/src/${tail.replace(/^web\//, "")}`) || suffixes.has(`apps/web/${tail.replace(/^web\//, "")}`);
  if (head === "@") return suffixes.has(`apps/web/src/${tail}`);
  if (head === "@xpute") return suffixes.has(`packages/xpute/${rest[0]}/src/${rest.slice(1).join("/")}`);
  const dir = crates.get(head);
  if (dir && (suffixes.has(`${dir}src/${tail}`) || suffixes.has(`${dir}${tail}`))) return true;
  // What a build writes and git ignores, such as the ABI's symbol table: named
  // by what git ignores rather than by what is on disk, since a fresh checkout
  // — the gate's, in CI — runs this before any build has written it.
  return [join(dirname(from), path), path, `apps/web/src/${path}`].some((p) => {
    try {
      if (Deno.statSync(join(root, p)).isFile) return true;
    } catch { /* not built here */ }
    return new Deno.Command("git", { args: ["check-ignore", "-q", p], cwd: root }).outputSync().code === 0;
  });
}

const CITED = /(?<![\w/.:@-])((?:\.{1,2}\/)*@?(?:[\w.@-]+\/)*[\w-]+(?:\.[\w-]+)*\.(?:rs|ts|tsx|wgsl))(?![\w/])/g;
const PROSE = /\.(rs|tsx?|wgsl|md|jsonc?)$/;

for (const f of files) {
  if (f.startsWith("issue/") || !PROSE.test(f)) continue;
  const text = read(f);
  if (!text) continue;
  const doc = f.endsWith(".md") || f.endsWith(".json") || f.endsWith(".jsonc");
  text.split("\n").forEach((line, n) => {
    if (!doc && !/^\s*(\/\/|\/?\*)/.test(line)) return;
    if (line.includes("://")) return;
    for (const [, path] of line.matchAll(CITED)) {
      if (!exists(path, f)) problems.push(`${f}:${n + 1}: names ${path}, which is no file here`);
    }
  });
}

// ---- the lattice ----

/** Where a level's shift of the hierarchy is written: the space model, and
 * the elevation format's sampling, which is derived from a page's span. */
const LATTICE_OWNERS = ["crates/pxl/spec/src/space/", "crates/pxl/spec/src/bake/constant.rs"];
const SHIFT = /(\b(L[1-4]|L[2-4]_RANGE|PAGE_CELLS|CHUNK_CELLS)\b|\.shamt\(\))[^;]*(<<|>>)|(<<|>>)[^;]*(\bL[1-4]\b|\.shamt\(\))/;
/** A page's cell index split into its column and row, or put together from
 * them, by the page's width. */
const IN_PAGE = /[%/]\s*PAGE_CELLS\b|(rem|div)_euclid\(\s*PAGE_CELLS\b|\*\s*PAGE_CELLS\b(\s+as\s+\w+)?\s*\+/;

for (const f of files) {
  if (!f.endsWith(".rs") || LATTICE_OWNERS.some((o) => f.startsWith(o))) continue;
  read(f)?.split("\n").forEach((line, n) => {
    if (/^\s*\/\//.test(line)) return;
    if (SHIFT.test(line)) problems.push(`${f}:${n + 1}: shifts the lattice outside pxl-spec space/: ${line.trim()}`);
    if (IN_PAGE.test(line)) problems.push(`${f}:${n + 1}: numbers a page's cells outside pxl-spec space/: ${line.trim()}`);
  });
}

if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`\ncheck-rules: ${problems.length} (AGENTS.md says why each rule is there)`);
  Deno.exit(1);
}
console.log("check-rules: headers, cited paths and the lattice hold");
