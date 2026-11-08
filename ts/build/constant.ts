// @pxl/build/constant.ts

/**
 * What a release build writes as values: each `export const` of a literal,
 * read through an import, so that the bundle holds the number and not its
 * name. Metro compiles one file at a time and keeps every export by name, so
 * a use is resolved here, through the import that names it. A file with a
 * platform's own variant beside it is not read, since which one a build takes
 * is the resolver's to say.
 */

import { createHash } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import { treeFile } from "./path.ts";

export type Literal = number | string | boolean;

/** What a file exports under one name: a literal, or a module read whole. */
export type Export = { val: Literal } | { file: string };

interface Module {
  exports: Map<string, Export>;
  /** Whether a literal is all the module exports. */
  pure: boolean;
}

const LITERAL =
  /^export const (\w+)(?:: [^=\n]+)? = (-?(?:0x[\da-fA-F]+|\d+(?:\.\d+)?(?:[eE][-+]?\d+)?)|true|false|"[^"\\\n]*");$/gm;
const NAMESPACE = /^import \* as (\w+) from "([^"]+)";$/gm;
const OWN_NAMES = /^export \{([^}]*)\};$/gm;
const FORWARD = /^export \{([^}]*)\} from "([^"]+)";$/gm;
const FORWARD_ALL = /^export \* from "([^"]+)";$/gm;
const FORWARD_AS = /^export \* as (\w+) from "([^"]+)";$/gm;
const PLATFORMS = ["native", "web", "ios", "android"];

const modules = new Map<string, Module>();

/** The file that `spec`, imported from `from`, names: relative, or through an
 * alias or a workspace package. */
export function file_of(from: string, spec: string): string | null {
  const base = spec.startsWith(".")
    ? path.resolve(path.dirname(from), spec)
    : treeFile(from, spec);
  if (base === null) return null;
  for (const at of [base, `${base}.ts`, `${base}.tsx`]) {
    if (fs.existsSync(at) && fs.statSync(at).isFile()) return at;
  }
  return null;
}

/** A clause's names that are values, each with what it is exported as. */
function names_of(clause: string): [name: string, as: string][] {
  return clause
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s !== "" && !s.startsWith("type "))
    .map((s) => {
      const [name, as] = s.split(/\s+as\s+/);
      return [name, as ?? name];
    });
}

function has_variant(file: string): boolean {
  const ext = path.extname(file);
  const stem = file.slice(0, -ext.length);
  return PLATFORMS.some((p) => fs.existsSync(`${stem}.${p}${ext}`));
}

function module_of(file: string): Module {
  const known = modules.get(file);
  if (known) return known;
  const exports = new Map<string, Export>();
  const m: Module = { exports, pure: false };
  // A cycle reads what is known so far.
  modules.set(file, m);
  if (has_variant(file)) return m;
  const text = fs.readFileSync(file, "utf8");
  const literals = [...text.matchAll(LITERAL)];
  for (const [, name, val] of literals) {
    exports.set(name, {
      val: val.startsWith('"') ? JSON.parse(val) : literal_of(val),
    });
  }
  m.pure =
    literals.length > 0 && literals.length === text.match(/^export /gm)?.length;
  const near = (spec: string) => file_of(file, spec);
  const spaces = new Map(
    [...text.matchAll(NAMESPACE)].map(([, name, spec]) => [name, spec]),
  );
  for (const [, clause] of text.matchAll(OWN_NAMES)) {
    for (const [name, as] of names_of(clause)) {
      const to = spaces.has(name) ? near(spaces.get(name)!) : null;
      if (to) exports.set(as, { file: to });
    }
  }
  for (const [, name, spec] of text.matchAll(FORWARD_AS)) {
    const to = near(spec);
    if (to) exports.set(name, { file: to });
  }
  for (const [, clause, spec] of text.matchAll(FORWARD)) {
    const to = near(spec);
    if (!to) continue;
    for (const [name, as] of names_of(clause)) {
      const found = module_of(to).exports.get(name);
      if (found) exports.set(as, found);
    }
  }
  for (const [, spec] of text.matchAll(FORWARD_ALL)) {
    const to = near(spec);
    if (!to) continue;
    for (const [name, found] of module_of(to).exports) {
      if (!exports.has(name)) exports.set(name, found);
    }
  }
  return m;
}

function literal_of(text: string): Literal {
  return text === "true" ? true : text === "false" ? false : Number(text);
}

export const exports_of = (file: string): Map<string, Export> =>
  module_of(file).exports;

export const is_pure = (file: string): boolean => module_of(file).pure;

/** Every literal the roots export and this reading of them, as one string,
 * which changes when either does. A file compiled against a literal does not
 * change with it, so this goes in the transform cache's key. */
export function digest(roots: string[]): string {
  const hash = createHash("sha256");
  hash.update(fs.readFileSync(import.meta.filename!));
  const walk = (dir: string): void => {
    const entries = fs
      .readdirSync(dir, { withFileTypes: true })
      .sort((a, b) => (a.name < b.name ? -1 : 1));
    for (const e of entries) {
      const at = path.join(dir, e.name);
      if (e.isDirectory()) {
        if (e.name !== "node_modules") walk(at);
      } else if (/\.tsx?$/.test(e.name)) {
        for (const [line] of fs.readFileSync(at, "utf8").matchAll(LITERAL)) {
          hash.update(line);
        }
      }
    }
  };
  roots.forEach(walk);
  return hash.digest("hex");
}
