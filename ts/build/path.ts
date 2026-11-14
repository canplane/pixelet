// @pxl/build/path.ts

/**
 * The aliases that tsconfig.json's `paths` give the tree's own directories,
 * and the sources of the workspace packages, read for the one tool with no
 * tsconfig resolver of its own: Metro. Vite and vitest read the same paths
 * themselves (`resolve.tsconfigPaths`) and resolve the packages as Node does.
 * This file is plain TypeScript over `node:` modules, so that Node loads it
 * directly (package.json `engines`).
 */

import * as fs from "node:fs";
import { createRequire } from "node:module";
import * as path from "node:path";

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
      out = out.replace(/,\s*$/, "") + c;
    } else out += c;
  }
  return JSON.parse(out);
}

export const WORKSPACE: string = path.dirname(import.meta.dirname!);

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

const MODULES = `${path.sep}node_modules${path.sep}`;

/** The file of the tree that `spec`, imported from the file `from`, names.
 * Through an alias, it is the path past the alias's prefix. Through a
 * workspace package (xpute's, `workspace:*`), it is the source file that the
 * package.json `exports` maps it to, as Node resolves it, including a
 * package's import of its own name. It is null for a relative specifier, for
 * an installed package, and for an import made from an installed package. */
export function tree_file(from: string, spec: string): string | null {
  const hit = PREFIXES.find(([prefix]) => spec.startsWith(prefix));
  if (hit) return hit[1] + spec.slice(hit[0].length);
  if (spec.startsWith(".") || path.isAbsolute(spec) || from.includes(MODULES)) {
    return null;
  }
  try {
    const file = createRequire(from).resolve(spec);
    return path.isAbsolute(file) && !file.includes(MODULES) ? file : null;
  } catch {
    return null;
  }
}
