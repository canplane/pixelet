// @pxl/build/import-map.ts

/**
 * The prefixes the Deno import maps give the tree's own directories, as the
 * type-check reads them: a module resolves through its own workspace
 * member's map (`<member>/deno.jsonc`) over the workspace's (ts/deno.jsonc), each
 * directory from the file that names it, so `@/` is the importing file's own
 * package. What reads the tree beside Deno and has no Deno resolver of its
 * own — Metro's, and the const-enum table both bundlers share — resolves
 * through these and holds no list of its own; Vite resolves through Deno
 * itself (@deno/vite-plugin), member maps included. Plain TypeScript over `node:` modules: Deno imports it, and Node, from
 * 22.18, loads it under a bundler's config (ts/app/native/package.json
 * `engines`).
 */

import * as fs from "node:fs";
import * as path from "node:path";

/** JSONC as JSON: comments and trailing commas out, strings untouched. */
function jsonc(text: string): unknown {
  let out = "";
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (c === '"') {
      const start = i;
      for (i++; i < text.length && text[i] !== '"'; i++) {
        if (text[i] === "\\") i++;
      }
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

/** The TypeScript workspace's root: the nearest directory up from here whose
 * deno.jsonc lists members, as Deno itself finds it (a root's own
 * `"workspace": []` lists none). Every other path a config names is found
 * from this or from `REPO`, never counted out from where the config is. */
export const WORKSPACE: string = (() => {
  for (let dir = import.meta.dirname!;; dir = path.dirname(dir)) {
    const file = path.join(dir, "deno.jsonc");
    if (fs.existsSync(file) && (read(file).workspace ?? []).length > 0) {
      return dir;
    }
    if (path.dirname(dir) === dir) {
      throw new Error(
        `import-map: no workspace's deno.jsonc over ${import.meta.dirname}`,
      );
    }
  }
})();

/** The repository's root, which holds the workspace (ts/) beside rust/ and xpute/. */
export const REPO: string = path.dirname(WORKSPACE);

/** A config's directory prefixes, each its absolute directory with a trailing separator. */
function prefixesOf(file: string): [string, string][] {
  return Object.entries(read(file).imports ?? {})
    .filter(([prefix, target]) =>
      prefix.endsWith("/") && /^\.\.?\//.test(target)
    )
    .map((
      [prefix, target],
    ) => [prefix, path.resolve(path.dirname(file), target) + path.sep]);
}

/** Longest first, as an import map takes the longest prefix that matches. */
function longestFirst(prefixes: [string, string][]): [string, string][] {
  return prefixes.sort(([a], [b]) => b.length - a.length);
}

const ROOT_CONFIG = path.join(WORKSPACE, "deno.jsonc");
const MEMBERS = (read(ROOT_CONFIG).workspace ?? []).map((member) =>
  path.join(WORKSPACE, member) + path.sep
);
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
    all = longestFirst([
      ...own,
      ...ROOT_PREFIXES.filter(([prefix]) => !named.has(prefix)),
    ]);
    BY_MEMBER.set(member, all);
  }
  return all;
}

/** Where `spec`, imported from `file`, names a directory of the tree through
 * the import maps: the path past its prefix, and the prefix; none for a
 * relative or a package specifier. */
export function mapped(
  file: string,
  spec: string,
): { path: string; prefix: string } | null {
  const hit = prefixesFor(file).find(([prefix]) => spec.startsWith(prefix));
  return hit
    ? { path: hit[1] + spec.slice(hit[0].length), prefix: hit[0] }
    : null;
}
