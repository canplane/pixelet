// @pxl/client/script/import-map.ts

/**
 * The prefixes the Deno import maps give the tree's own directories, as the
 * type-check reads them: a module resolves through its own workspace
 * member's map (`<member>/deno.jsonc`) over the root's (deno.jsonc), each
 * directory from the file that names it, so `@/` is the importing file's own
 * package. What reads the tree beside Deno — Vite's and Metro's resolvers,
 * the const-enum table — resolves through these and holds no list of its
 * own. Plain TypeScript over `node:` modules: Deno imports it, and Node, from
 * 22.18, loads it under a bundler's config (apps/native/package.json
 * `engines`).
 */

import * as fs from "node:fs";
import * as path from "node:path";

/** The tree's root. */
export const ROOT: string = path.resolve(import.meta.dirname!, "../../..");

/** JSONC as JSON: comments and trailing commas out, strings untouched. */
function jsonc(text: string): unknown {
  let out = "";
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      const start = i;
      for (i++; i < text.length && text[i] !== '"'; i++) if (text[i] === "\\") i++;
      out += text.slice(start, i + 1);
    } else if (c === "/" && text[i + 1] === "/") {
      while (i < text.length && text[i] !== "\n") i++;
      out += "\n";
    } else if (c === "/" && text[i + 1] === "*") {
      const end = text.indexOf("*/", i + 2);
      i = end < 0 ? text.length : end + 1;
    } else if (c === "}" || c === "]") {
      // A comma before a close, outside strings and comments, is trailing.
      out = out.replace(/,\s*$/, "") + c;
    } else out += c;
  }
  return JSON.parse(out);
}

type Config = { workspace?: string[]; imports?: Record<string, string> };

function read(file: string): Config {
  return jsonc(fs.readFileSync(file, "utf8")) as Config;
}

/** A config's directory prefixes, each its absolute directory with a trailing separator. */
function prefixesOf(file: string): [string, string][] {
  return Object.entries(read(file).imports ?? {})
    .filter(([prefix, target]) => prefix.endsWith("/") && /^\.\.?\//.test(target))
    .map(([prefix, target]) => [prefix, path.resolve(path.dirname(file), target) + path.sep]);
}

/** Longest first, as an import map takes the longest prefix that matches. */
function longestFirst(prefixes: [string, string][]): [string, string][] {
  return prefixes.sort(([a], [b]) => b.length - a.length);
}

const ROOT_CONFIG = path.join(ROOT, "deno.jsonc");
const MEMBERS = (read(ROOT_CONFIG).workspace ?? []).map((member) => path.join(ROOT, member) + path.sep);
const ROOT_PREFIXES = longestFirst(prefixesOf(ROOT_CONFIG));
const BY_MEMBER = new Map<string, [string, string][]>();

/** The prefixes a module at `file` resolves through: its workspace member's
 * own over the root's, longest first; the root's alone outside every
 * member. */
export function prefixesFor(file: string): [string, string][] {
  const member = MEMBERS.find((dir) => file.startsWith(dir));
  if (member === undefined) return ROOT_PREFIXES;
  let all = BY_MEMBER.get(member);
  if (all === undefined) {
    const own = prefixesOf(path.join(member, "deno.jsonc"));
    const named = new Set(own.map(([prefix]) => prefix));
    all = longestFirst([...own, ...ROOT_PREFIXES.filter(([prefix]) => !named.has(prefix))]);
    BY_MEMBER.set(member, all);
  }
  return all;
}

/** Where `spec`, imported from `file`, names a directory of the tree through
 * the import maps: the path past its prefix, and the prefix; none for a
 * relative or a package specifier. */
export function mapped(file: string, spec: string): { path: string; prefix: string } | null {
  const hit = prefixesFor(file).find(([prefix]) => spec.startsWith(prefix));
  return hit ? { path: hit[1] + spec.slice(hit[0].length), prefix: hit[0] } : null;
}
