// @pxl/web/script/const-enum.cjs

/**
 * The const enums of the tree, read once for both bundlers: the page's
 * (vite.config.ts `constEnum`) and the device's (apps/native/babel.config.js
 * `inlineEnums`). A const enum is a compile-time name for a number, and a
 * bundle holds the number alone — not one member's name as a string. A
 * transpiler that sees one file at a time cannot get there by itself: it
 * emits the enum as an object, reverse mapping and all
 * (`e[e.WORLD_SNAPSHOT=6933]="WORLD_SNAPSHOT"`), or erases the declaration and
 * leaves its importers bound to nothing. So the declarations are read up
 * front, here, and each bundler only replaces a use with its value and
 * strikes the declaration.
 *
 * Nothing is left alone. An enum that is not const, or a member whose value
 * is not a plain literal or arithmetic over literals, is an error here,
 * before either bundler runs, since either would put a name in a bundle as a
 * string. A use is the enum its import names, followed to the file that
 * declares it, so two files exporting one name are two enums. An enum that is
 * not exported is its file's alone and is replaced in that file only.
 *
 * CommonJS, because Metro requires the Babel config that reads this.
 */

const fs = require("node:fs");
const path = require("node:path");

/** A const enum's declaration, exported or not. */
const DECL = /(export\s+)?const\s+enum\s+(\w+)\s*\{([^}]*)\}/g;
/** An enum that is not const, which no bundler can strike. */
const PLAIN = /(?:^|[\n;])\s*(?:export\s+)?(?:declare\s+)?enum\s+(\w+)\s*\{/g;
/** Numbers, strings and arithmetic over them, and nothing that runs. */
const VALUE = /^(?:0x[0-9a-fA-F]+|\d+|"[^"\\]*"|'[^'\\]*'|[-+*/|&^~()<>\s])+$/;

/** A declaration's members by name, each its value: a number or a string. */
function membersOf(body, where) {
  const members = new Map();
  // A member ends at a line's end or at a comma outside a string, so an enum
  // written on one line reads as one written a member a line.
  const lines = body.replace(/\/\*[\s\S]*?\*\//g, "").split("\n").map((line) => line.replace(/\/\/.*/, ""));
  for (let line of lines.flatMap((line) => line.split(/,(?=(?:[^"']*["'][^"']*["'])*[^"']*$)/))) {
    line = line.trim();
    if (!line || line.startsWith("*")) continue;
    const at = line.indexOf("=");
    const key = at < 0 ? "" : line.slice(0, at).trim();
    const value = at < 0 ? "" : line.slice(at + 1).trim();
    if (!/^\w+$/.test(key) || !VALUE.test(value)) {
      throw new Error(`const-enum: ${where}: \`${line}\` is not a member with a literal value, so the enum cannot be inlined and would reach the bundle by name`);
    }
    members.set(key, /^["']/.test(value) ? value.slice(1, -1) : Function(`return (${value})`)());
  }
  return members;
}

/**
 * Every const enum under `roots`, by the file that declares it: `exported`
 * and `local` each by the file's path and then by name. Two files may export
 * one name — xpute's command table and pixelet's both export `Command` — so a
 * use is found through its import, never by its name alone (`enumAt`). Tests
 * are not bundled and are not read.
 */
function readEnums(roots) {
  const exported = new Map();
  const local = new Map();
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, e.name);
      if (e.isDirectory()) {
        if (e.name !== "node_modules") walk(full);
        continue;
      }
      if (!/\.tsx?$/.test(e.name) || e.name.includes(".test.")) continue;
      const src = fs.readFileSync(full, "utf8");
      for (const [, name] of src.replace(DECL, "").matchAll(PLAIN)) {
        throw new Error(`const-enum: ${full}: enum ${name} is not const, so a bundle would carry its members' names; declare it \`const enum\``);
      }
      for (const [, isExported, name, body] of src.matchAll(DECL)) {
        const by = isExported ? exported : local;
        if (!by.has(full)) by.set(full, new Map());
        by.get(full).set(name, membersOf(body, `${full}: ${name}`));
      }
    }
  };
  for (const root of roots) walk(root);
  return { exported, local };
}

/** The import map's prefixes, as the page's deno.jsonc and Metro's config
 * name them. */
function aliases(root) {
  return [
    ["@/", path.join(root, "apps/web/src/")],
    ["@xpute/core/", path.join(root, "packages/xpute/core/src/")],
    ["@xpute/runtime/", path.join(root, "packages/xpute/runtime/src/")],
    ["@xpute/conformance/", path.join(root, "packages/xpute/conformance/src/")],
  ];
}

/** The file `spec` names from `from`: relative to it, or through an alias;
 * none for a package or a file that is not there. */
function fileOf(from, spec, root) {
  let base = null;
  if (spec.startsWith(".")) base = path.resolve(path.dirname(from), spec);
  for (const [prefix, dir] of aliases(root)) {
    if (spec.startsWith(prefix)) base = dir + spec.slice(prefix.length);
  }
  if (base === null) return null;
  for (const at of [base, base + ".ts", base + ".tsx", path.join(base, "index.ts")]) {
    if (fs.existsSync(at) && fs.statSync(at).isFile()) return at;
  }
  return null;
}

/**
 * The members of the enum a file `from` imports as `name` from `spec`, or
 * none where that is no const enum: the file it names declares it, or
 * passes it on, one `export { … } from` or `export * from` at a time.
 */
function enumAt(table, root, from, spec, name, depth = 0) {
  const file = fileOf(from, spec, root);
  if (file === null || depth > 8) return null;
  const own = table.exported.get(file)?.get(name);
  if (own) return own;
  const src = fs.readFileSync(file, "utf8");
  for (const [, clause, next] of src.matchAll(/export\s*\{([^}]*)\}\s*from\s*["']([^"']+)["']/g)) {
    for (const item of clause.split(",").map((s) => s.trim()).filter(Boolean)) {
      const [orig, as] = item.replace(/^type\s+/, "").split(/\s+as\s+/);
      if ((as ?? orig) === name) return enumAt(table, root, file, next, orig, depth + 1);
    }
  }
  for (const [, next] of src.matchAll(/export\s*\*\s*from\s*["']([^"']+)["']/g)) {
    const found = enumAt(table, root, file, next, name, depth + 1);
    if (found) return found;
  }
  return null;
}

/** A member's value as source text. */
function literal(value) {
  return typeof value === "string" ? JSON.stringify(value) : String(value);
}

/** What a use or a declaration the table does not have means: the enums
 * changed after the table was read, and `restart` is how to read it again. */
function stale(what, restart) {
  return `const-enum: ${what} is not in the enum table read when the bundler started; the enums changed since, so ${restart}`;
}

module.exports = { DECL, readEnums, enumAt, literal, stale };
