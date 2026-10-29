// @pxl/build/paths.ts

/**
 * The aliases tsconfig.json's `paths` give the tree's own directories, read
 * for what has no tsconfig resolver of its own: Metro, and the const-enum
 * table both bundlers share. Vite and vitest read the same paths themselves
 * (`resolve.tsconfigPaths`). Plain TypeScript over `node:` modules, which
 * Node loads (package.json `engines`).
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

/** The TypeScript workspace's root, the directory over this one. */
export const WORKSPACE: string = path.dirname(import.meta.dirname!);

/** The repository's root, which holds the workspace (ts/) beside rust/ and xpute/. */
export const REPO: string = path.dirname(WORKSPACE);

type Config = { compilerOptions?: { paths?: Record<string, string[]> } };

/** Each directory alias and its absolute directory, longest first, as
 * TypeScript takes the longest prefix that matches. */
const PREFIXES: [string, string][] = Object.entries(
  (
    jsonc(
      fs.readFileSync(path.join(WORKSPACE, "tsconfig.json"), "utf8"),
    ) as Config
  ).compilerOptions?.paths ?? {},
)
  .filter(([alias, [target]]) => alias.endsWith("/*") && target.endsWith("/*"))
  .map(([alias, [target]]): [string, string] => [
    alias.slice(0, -1),
    path.resolve(WORKSPACE, target.slice(0, -1)) + path.sep,
  ])
  .sort(([a], [b]) => b.length - a.length);

/** Where `spec` names a directory of the tree through an alias: the path past
 * its prefix, and the prefix; none for a relative or a package specifier. */
export function mapped(spec: string): { path: string; prefix: string } | null {
  const hit = PREFIXES.find(([prefix]) => spec.startsWith(prefix));
  return hit
    ? { path: hit[1] + spec.slice(hit[0].length), prefix: hit[0] }
    : null;
}
