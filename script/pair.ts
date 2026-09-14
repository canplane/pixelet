// script/pair.ts
//
// The kits are the same kit twice — once in TypeScript for the host, once
// in Rust for the module — and this holds the two to each other. Run as
// `deno task check:pair`, part of `deno task check`.
//
// The TypeScript side is the source and is read-only here: nothing is
// generated into it or removed from it. For each pair of trees below:
//
// - Every `.ts` under a required subtree has a `.rs` at the same path
//   (`mod.ts` ↔ `mod.rs`), and every `.rs` under the crate has a `.ts` —
//   a Rust module that needs one thing of a TypeScript file is that whole
//   file, at that path.
// - Every export of the `.ts` has a `pub` item of the same name in the
//   `.rs`: a function, a constant, a type, a namespace (`pub mod` / `pub
//   use`), and each public method of an exported class a `pub fn`.
// - Every `Deno.test("name")` of a `foo.test.ts` is a `#[test] fn` in the
//   sibling `foo.test.rs` (`#[cfg(test)] #[path = "foo.test.rs"] mod test;`
//   in foo.rs), named by the test's slug (lower-cased, runs of anything
//   but letters and digits as one underscore).
// - Every vector generator under script/vector/ (a `.ts` at the mirrored
//   path, exporting `vectors()`) writes what the TypeScript computes for a
//   fixed input set, as `path<TAB>value` lines; the file it writes under the crate's `vector/` (crates/xpute/core/vector, crates/pxl/shared/vector)
//   must be committed as written (`deno task vector` writes them) and read
//   by a Rust test through `include_str!`.
//
// A difference the languages force is exempted by name in
// script/pair.exempt.json, with its reason, and an exemption nothing
// needs any more fails as loudly as a missing pair.

import { walk } from "@std/fs/walk";
import { dirname, join, relative } from "@std/path";

type Tree = {
  /** The TypeScript tree, the source. */
  ts: string;
  /** The Rust tree that mirrors it. */
  rs: string;
  /** Subtrees (relative to `ts`) every file of which must be paired;
   * files outside them are paired only once a Rust file claims them. */
  required: string[];
  /** Where a generator's output lands, relative to the crate. */
  vector: string;
};

const TREES: Tree[] = [
  { ts: "packages/xpute/core/src", rs: "crates/xpute/core/src", required: [""], vector: "crates/xpute/core/vector" },
  { ts: "packages/pxl/shared/src", rs: "crates/pxl/shared/src", required: ["spec/space"], vector: "crates/pxl/shared/vector" },
];

const EXEMPT_PATH = "script/pair.exempt.json";
const VECTOR_ROOT = "script/vector";

const root = dirname(dirname(new URL(import.meta.url).pathname));
const exempt: Record<string, string> = JSON.parse(await Deno.readTextFile(join(root, EXEMPT_PATH)));
const used = new Set<string>();
const problems: string[] = [];

function exempted(key: string): boolean {
  if (!(key in exempt)) return false;
  if (typeof exempt[key] !== "string" || exempt[key].trim() === "") problems.push(`${EXEMPT_PATH}: "${key}" has no reason`);
  used.add(key);
  return true;
}

function fail(key: string, message: string): void {
  if (!exempted(key)) problems.push(message);
}

async function exists(path: string): Promise<boolean> {
  try {
    await Deno.stat(path);
    return true;
  } catch {
    return false;
  }
}

async function files(dir: string, ext: string): Promise<string[]> {
  const out: string[] = [];
  if (!(await exists(dir))) return out;
  for await (const e of walk(dir, { includeDirs: false, exts: [ext] })) out.push(relative(dir, e.path));
  return out.sort();
}

/** The `pub` items of a Rust file by name, and its `#[test]` functions. A
 * `pub use m::*` re-exports everything `m.rs` beside the file exports, as
 * `export * from "./m.ts"` does. */
async function rust_items_at(path: string): Promise<{ pub: Set<string>; tests: Set<string> }> {
  const items = rust_items(await Deno.readTextFile(path));
  for (const glob of items.globs) {
    const sibling = join(dirname(path), glob.replace(/^(super|self)::/, "").replace(/::/g, "/") + ".rs");
    if (await exists(sibling)) { for (const name of (await rust_items_at(sibling)).pub) items.pub.add(name); }
  }
  return items;
}

function rust_items(source: string): { pub: Set<string>; tests: Set<string>; globs: string[] } {
  const pub = new Set<string>();
  const tests = new Set<string>();
  const globs: string[] = [];
  // A raw identifier (`r#mod`) is the TypeScript name that is a Rust keyword.
  const item = /^\s*pub\s+(?:unsafe\s+)?(?:const\s+)?(?:async\s+)?(?:extern\s+"C"\s+)?(?:fn|const|static|struct|enum|type|trait|union|mod)\s+(?:r#)?([A-Za-z_][A-Za-z0-9_]*)/gm;
  for (const m of source.matchAll(item)) pub.add(m[1]);
  // `pub use a::b::{C, d as e};` names every re-exported item.
  const reexport = /^\s*pub\s+use\s+([^;]+);/gm;
  for (const m of source.matchAll(reexport)) {
    const names = m[1].replace(/[{}]/g, " ").split(/[\s,]+/).filter(Boolean);
    for (let i = 0; i < names.length; i++) {
      if (names[i] === "as") continue;
      const leaf = names[i].split("::").pop()!;
      if (names[i + 1] === "as") {
        pub.add(names[i + 2]);
        i += 2;
      } else if (leaf === "*") globs.push(names[i].replace(/::\*$/, ""));
      else if (leaf !== "self") pub.add(leaf);
    }
  }
  const macro = /^\s*#\[macro_export\]\s*\n\s*macro_rules!\s+([A-Za-z_][A-Za-z0-9_]*)/gm;
  for (const m of source.matchAll(macro)) pub.add(m[1]);
  // A public trait's methods are public without the keyword.
  for (const m of source.matchAll(/^pub trait [^{]*\{([\s\S]*?)^\}/gm)) {
    for (const f of m[1].matchAll(/^\s+(?:unsafe\s+)?fn\s+(?:r#)?([A-Za-z_][A-Za-z0-9_]*)/gm)) pub.add(f[1]);
  }
  // `[Symbol.iterator]` is `impl Iterator for`, or an `iter` method.
  if (/impl(?:<[^>]*>)?\s+Iterator\s+for/.test(source)) pub.add("[Symbol.iterator]");
  if (pub.has("iter")) pub.add("[Symbol.iterator]");
  const test = /#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+([A-Za-z_][A-Za-z0-9_]*)/g;
  for (const m of source.matchAll(test)) tests.add(m[1]);
  return { pub, tests, globs };
}

/** A Deno test's name as a Rust function name. */
function slug(name: string): string {
  const s = name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "");
  return /^[0-9]/.test(s) ? `_${s}` : s;
}

type Symbol = { name: string; kind: string; origin: string; methods: string[] };

/** The exports of every TypeScript file given, through `deno doc`. */
async function ts_exports(paths: string[]): Promise<Map<string, Symbol[]>> {
  const out = new Map<string, Symbol[]>();
  if (paths.length === 0) return out;
  const cmd = new Deno.Command(Deno.execPath(), { args: ["doc", "--json", ...paths], cwd: root, stdout: "piped", stderr: "piped" });
  const { code, stdout, stderr } = await cmd.output();
  if (code !== 0) throw new Error(`deno doc: ${new TextDecoder().decode(stderr)}`);
  const doc = JSON.parse(new TextDecoder().decode(stdout));
  for (const [url, node] of Object.entries<{ symbols: DocSymbol[] }>(doc.nodes)) {
    const file = relative(root, new URL(url).pathname);
    const symbols: Symbol[] = [];
    for (const s of node.symbols) {
      for (const d of s.declarations) {
        if (d.declarationKind !== "export") continue;
        const methods = d.kind === "class" ? (d.def?.methods ?? []).filter((m) => m.accessibility !== "private" && m.accessibility !== "protected" && !m.name.startsWith("_")).map((m) => m.name) : [];
        symbols.push({ name: s.name, kind: d.kind, origin: relative(root, new URL(d.location.filename).pathname), methods });
      }
    }
    out.set(file, symbols);
  }
  return out;
}

type DocSymbol = {
  name: string;
  declarations: {
    declarationKind: string;
    kind: string;
    location: { filename: string };
    def?: { methods?: { name: string; accessibility?: string }[] };
  }[];
};

async function check_tree(tree: Tree): Promise<void> {
  const ts_dir = join(root, tree.ts);
  const rs_dir = join(root, tree.rs);
  const ts_all = await files(ts_dir, ".ts");
  const rs_all = await files(rs_dir, ".rs");
  const ts_src = ts_all.filter((f) => !f.endsWith(".test.ts"));
  const ts_test = ts_all.filter((f) => f.endsWith(".test.ts"));
  const pair_of = (ts: string) => ts.replace(/\.test\.ts$/, ".ts").replace(/\.ts$/, ".rs");

  // Files: the required subtrees, whole; and every Rust file claims one.
  const required = (f: string) => tree.required.some((r) => r === "" || f === r || f.startsWith(r + "/"));
  const paired = new Set<string>();
  for (const f of ts_src) {
    const rs = pair_of(f);
    if (rs_all.includes(rs)) paired.add(f);
    else if (required(f)) fail(`${tree.ts}/${f}`, `${tree.ts}/${f} has no pair at ${tree.rs}/${rs}`);
  }
  for (const f of rs_all) {
    const ts = f.replace(/\.rs$/, ".ts");
    if (f === "lib.rs" || ts_src.includes(ts) || (f.endsWith(".test.rs") && ts_src.includes(ts.replace(/\.test\.ts$/, ".ts")))) continue;
    // A directory's module file where TypeScript has no mod.ts: the
    // language's own, allowed when it only declares the tree.
    if (/(^|\/)mod\.rs$/.test(f) || (await exists(join(ts_dir, f.replace(/\.rs$/, ""))))) {
      const body = (await Deno.readTextFile(join(rs_dir, f))).replace(/^\s*(\/\/.*|pub mod [a-z_0-9]+;|#\[path = "[^"]*"\]|#!\[.*\])?\s*$/gm, "").trim();
      if (body === "") continue;
    }
    // A test file belongs to its module, and so does the module's exemption.
    fail(`${tree.rs}/${f.replace(/\.test\.rs$/, ".rs")}`, `${tree.rs}/${f} has no TypeScript at ${tree.ts}/${ts} — a Rust module is a whole TypeScript file, at its path`);
  }

  // Exports and tests, for every paired file.
  const exports = await ts_exports([...paired].map((f) => join(tree.ts, f)));
  for (const f of paired) {
    const rs_path = join(tree.rs, pair_of(f));
    const items = await rust_items_at(join(root, rs_path));
    for (const s of exports.get(join(tree.ts, f)) ?? []) {
      const key = `${tree.ts}/${f}#${s.name}`;
      if (!items.pub.has(s.name)) fail(key, `${rs_path} lacks \`pub ${s.name}\` (${tree.ts}/${f} exports it as a ${s.kind})`);
      for (const m of s.methods) {
        if (!items.pub.has(m)) fail(`${key}.${m}`, `${rs_path} lacks \`pub fn ${m}\` (${s.name}'s method in ${tree.ts}/${f})`);
      }
    }
    const test = f.replace(/\.ts$/, ".test.ts");
    if (!ts_test.includes(test)) continue;
    const rs_test = join(tree.rs, test.replace(/\.test\.ts$/, ".test.rs"));
    const tests = (await exists(join(root, rs_test))) ? rust_items(await Deno.readTextFile(join(root, rs_test))).tests : new Set<string>();
    const source = await Deno.readTextFile(join(ts_dir, test));
    for (const m of source.matchAll(/Deno\.test\(\s*(["'`])((?:\\.|(?!\1).)*)\1/g)) {
      const name = m[2];
      if (!tests.has(slug(name))) fail(`${tree.ts}/${test}#${name}`, `${rs_test} lacks \`#[test] fn ${slug(name)}\` (${tree.ts}/${test}: "${name}")`);
    }
  }
  for (const t of ts_test) {
    const src = t.replace(/\.test\.ts$/, ".ts");
    if (!paired.has(src) && required(src) && !ts_src.includes(src)) fail(`${tree.ts}/${t}`, `${tree.ts}/${t} tests nothing paired`);
  }

  // Vectors: what the generators write is what is committed, and read.
  const gen_dir = join(root, VECTOR_ROOT, tree.ts.replace(/^packages\//, "").replace(/\/src$/, ""));
  const vec_dir = join(root, tree.vector);
  const written = new Set<string>();
  for (const g of await files(gen_dir, ".ts")) {
    const target = g.replace(/\.ts$/, ".tsv");
    written.add(target);
    const mod = await import(join(gen_dir, g));
    const fresh = flatten(mod.vectors());
    const path = join(vec_dir, target);
    const on_disk = (await exists(path)) ? await Deno.readTextFile(path) : null;
    if (on_disk !== fresh) problems.push(`${tree.vector}/${target} is not what ${VECTOR_ROOT}/${relative(join(root, VECTOR_ROOT), join(gen_dir, g))} writes — run \`deno task vector\``);
    // Read by the paired module or its test file beside it.
    const readers = [g.replace(/\.ts$/, ".rs"), g.replace(/\.ts$/, ".test.rs")].filter((r) => rs_all.includes(r));
    let read = false;
    for (const r of readers) if ((await Deno.readTextFile(join(rs_dir, r))).includes(`vector/${target}`)) read = true;
    if (!read) problems.push(`${tree.vector}/${target} is read by no \`include_str!\` in ${tree.rs}/${g.replace(/\.ts$/, "{.rs,.test.rs}")}`);
  }
  for (const v of await files(vec_dir, ".tsv")) {
    if (!written.has(v)) problems.push(`${tree.vector}/${v} has no generator at ${VECTOR_ROOT}/…/${v.replace(/\.tsv$/, ".ts")}`);
  }
}

/**
 * A generator's vectors as lines of `path<TAB>value`, the path the keys and
 * indices down to the value joined by dots (`ivec.unary.3.not`): what a
 * Rust test reads with a split per line and no parser, so the reader is
 * not itself a thing to verify. A value is a string as it is, a BigInt or
 * an integer in decimal, a boolean as true/false; a double goes through
 * script/vector.ts's f64_bits first, and a string must not hold a tab or a
 * newline.
 */
function flatten(value: unknown): string {
  const lines: string[] = [];
  const walk = (v: unknown, path: string) => {
    if (Array.isArray(v)) v.forEach((e, i) => walk(e, path ? `${path}.${i}` : `${i}`));
    else if (v !== null && typeof v === "object") { for (const [k, e] of Object.entries(v)) walk(e, path ? `${path}.${k}` : k); }
    else {
      const text = String(v);
      if (/[\t\n]/.test(text)) throw new Error(`vector: ${path} holds a tab or a newline`);
      if (typeof v === "number" && !Number.isSafeInteger(v)) throw new Error(`vector: ${path} is a double; write it through f64_bits`);
      lines.push(`${path}\t${text}`);
    }
  };
  walk(value, "");
  return lines.join("\n") + "\n";
}

/** Writes every generator's vectors: `deno task vector`. */
async function write_vectors(): Promise<void> {
  for (const tree of TREES) {
    const gen_dir = join(root, VECTOR_ROOT, tree.ts.replace(/^packages\//, "").replace(/\/src$/, ""));
    for (const g of await files(gen_dir, ".ts")) {
      const mod = await import(join(gen_dir, g));
      const path = join(root, tree.vector, g.replace(/\.ts$/, ".tsv"));
      await Deno.mkdir(dirname(path), { recursive: true });
      await Deno.writeTextFile(path, flatten(mod.vectors()));
      console.log(`wrote ${relative(root, path)}`);
    }
  }
}

if (Deno.args.includes("--write")) {
  await write_vectors();
} else {
  for (const tree of TREES) await check_tree(tree);
  for (const key of Object.keys(exempt)) {
    if (!used.has(key)) problems.push(`${EXEMPT_PATH}: "${key}" exempts nothing — drop it`);
  }
  if (problems.length > 0) {
    for (const p of problems) console.error(`pair: ${p}`);
    console.error(`pair: ${problems.length} problem${problems.length === 1 ? "" : "s"}`);
    Deno.exit(1);
  }
  console.log("pair: every file, export, test and vector has its pair");
}
