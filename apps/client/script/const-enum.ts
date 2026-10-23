// @pxl/client/script/const-enum.ts

/**
 * The tree's const enums, read once for both bundlers (Vite and Metro's
 * Babel): a one-file-at-a-time transpiler emits an enum as an object with
 * member names as strings. A non-const enum or a non-literal member is an
 * error here. A use is resolved through its import, so two files exporting
 * one name are two enums, and an import resolves through the import maps as
 * the type-check reads them (import-map.ts).
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { mapped } from "./import-map.ts";

/** A const enum's members, each its value. */
export type Members = Map<string, number | string>;
/** Each file's enums, by name. */
export type Enums = Map<string, Map<string, Members>>;
export type Table = { exported: Enums; local: Enums };

export const DECL = /(export\s+)?const\s+enum\s+(\w+)\s*\{([^}]*)\}/g;
const PLAIN = /(?:^|[\n;])\s*(?:export\s+)?(?:declare\s+)?enum\s+(\w+)\s*\{/g;
const VALUE = /^(?:0x[0-9a-fA-F]+|\d+|"[^"\\]*"|'[^'\\]*'|[-+*/|&^~()<>\s])+$/;

function membersOf(body: string, where: string): Members {
  const members: Members = new Map();
  // Split on commas outside strings too, so a one-line enum reads the same.
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

/** Tests are not bundled and are not read. */
export function readEnums(roots: string[]): Table {
  const exported: Enums = new Map();
  const local: Enums = new Map();
  const walk = (dir: string) => {
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
        by.get(full)!.set(name, membersOf(body, `${full}: ${name}`));
      }
    }
  };
  for (const root of roots) walk(root);
  return { exported, local };
}

/** Where `spec`, imported from `from`, is: relative, or through the import
 * maps as `from`'s own package reads them (import-map.ts). `@platform/` is
 * the bundling app's, and holds no enum the shell imports. */
function fileOf(from: string, spec: string): string | null {
  let base: string | null = null;
  if (spec.startsWith(".")) base = path.resolve(path.dirname(from), spec);
  else {
    const hit = mapped(from, spec);
    if (hit && hit.prefix !== "@platform/") base = hit.path;
  }
  if (base === null) return null;
  for (const at of [base, base + ".ts", base + ".tsx", path.join(base, "index.ts")]) {
    if (fs.existsSync(at) && fs.statSync(at).isFile()) return at;
  }
  return null;
}

/** Follows `export { … } from` and `export * from` to the declaring file. */
export function enumAt(table: Table, from: string, spec: string, name: string, depth = 0): Members | null {
  const file = fileOf(from, spec);
  if (file === null || depth > 8) return null;
  const own = table.exported.get(file)?.get(name);
  if (own) return own;
  const src = fs.readFileSync(file, "utf8");
  for (const [, clause, next] of src.matchAll(/export\s*\{([^}]*)\}\s*from\s*["']([^"']+)["']/g)) {
    for (const item of clause.split(",").map((s) => s.trim()).filter(Boolean)) {
      const [orig, as] = item.replace(/^type\s+/, "").split(/\s+as\s+/);
      if ((as ?? orig) === name) return enumAt(table, file, next, orig, depth + 1);
    }
  }
  for (const [, next] of src.matchAll(/export\s*\*\s*from\s*["']([^"']+)["']/g)) {
    const found = enumAt(table, file, next, name, depth + 1);
    if (found) return found;
  }
  return null;
}

export function literal(value: number | string): string {
  return typeof value === "string" ? JSON.stringify(value) : String(value);
}

export function stale(what: string, restart: string): string {
  return `const-enum: ${what} is not in the enum table read when the bundler started; the enums changed since, so ${restart}`;
}
