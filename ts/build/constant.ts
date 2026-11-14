// @pxl/build/constant.ts

/**
 * What a release build writes as values: each `export const` that is a
 * literal or an expression of literals, read through an import or in its own
 * module, so that the bundle holds the number and not its name. Metro compiles
 * one file at a time, so a use is resolved here, through the import that names
 * it or the module's own binding, and an export no read names any more is left
 * for tree shaking to drop. A file with a platform's own variant beside it is
 * not read, since which one a build takes is the resolver's to say.
 */

import { createHash } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import { parse } from "@babel/parser";
import { treeFile } from "./path.ts";

export type Literal = number | string | boolean;

/** What a file exports under one name: a literal, or a module read whole. */
export type Export = { val: Literal } | { file: string };

interface Module {
  exports: Map<string, Export>;
  /** Whether a literal is all the module exports. */
  pure: boolean;
}

type Statement = ReturnType<typeof parse>["program"]["body"][number];
type Declaration = Extract<Statement, { type: "VariableDeclaration" }>;
type Init = Declaration["declarations"][number]["init"];
type Node = Exclude<Init, null | undefined> | PrivateName;
type PrivateName = { type: "PrivateName" };
type Name = { type: "Identifier"; name: string } | { value: string };

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

function has_variant(file: string): boolean {
  const ext = path.extname(file);
  const stem = file.slice(0, -ext.length);
  return PLATFORMS.some((p) => fs.existsSync(`${stem}.${p}${ext}`));
}

const name_of = (n: Name): string => ("name" in n ? n.name : n.value);

type Names = Map<string, Export | undefined>;

const BINARY: Record<string, (a: number, b: number) => number> = {
  "+": (a, b) => a + b,
  "-": (a, b) => a - b,
  "*": (a, b) => a * b,
  "/": (a, b) => a / b,
  "%": (a, b) => a % b,
  "**": (a, b) => a ** b,
  "<<": (a, b) => a << b,
  ">>": (a, b) => a >> b,
  ">>>": (a, b) => a >>> b,
  "&": (a, b) => a & b,
  "|": (a, b) => a | b,
  "^": (a, b) => a ^ b,
};

const UNARY: Record<string, (a: number) => number> = {
  "-": (a) => -a,
  "+": (a) => a,
  "~": (a) => ~a,
};

/** A value a literal can write. */
const finite = (v: number): number | null => (Number.isFinite(v) ? v : null);

/** What an expression evaluates to, where it is made of literals and of names
 * bound to literals in `names`; null where it is anything else. It is evaluated
 * as the program would, so a value holds whatever the program's would. */
function literal_of(e: Init | Node | undefined, names: Names): Literal | null {
  switch (e?.type) {
    case "NumericLiteral":
    case "StringLiteral":
    case "BooleanLiteral":
      return e.value;
    case "Identifier": {
      const at = names.get(e.name);
      return at !== undefined && "val" in at ? at.val : null;
    }
    case "MemberExpression": {
      if (e.computed || e.object.type !== "Identifier") return null;
      const at = names.get(e.object.name);
      if (at === undefined || "val" in at) return null;
      if (e.property.type !== "Identifier") return null;
      const found = module_of(at.file).exports.get(e.property.name);
      return found !== undefined && "val" in found ? found.val : null;
    }
    case "UnaryExpression": {
      const a = literal_of(e.argument, names);
      const f = UNARY[e.operator];
      return typeof a === "number" && f ? finite(f(a)) : null;
    }
    case "BinaryExpression": {
      const a = literal_of(e.left, names);
      const b = literal_of(e.right, names);
      if (
        typeof a === "string" &&
        typeof b === "string" &&
        e.operator === "+"
      ) {
        return a + b;
      }
      const f = BINARY[e.operator];
      return typeof a === "number" && typeof b === "number" && f
        ? finite(f(a, b))
        : null;
    }
    case "TSAsExpression":
    case "TSSatisfiesExpression":
      return literal_of(e.expression, names);
    default:
      return null;
  }
}

type Pattern = Declaration["declarations"][number]["id"];

/** The names a declaration's pattern binds. */
function names_in(p: Pattern | null): string[] {
  switch (p?.type) {
    case "Identifier":
      return [p.name];
    case "ObjectPattern":
      return p.properties.flatMap((q) =>
        names_in(q.type === "RestElement" ? q.argument : (q.value as Pattern)),
      );
    case "ArrayPattern":
      return p.elements.flatMap((q) => names_in(q as Pattern | null));
    case "AssignmentPattern":
      return names_in(p.left);
    case "RestElement":
      return names_in(p.argument as Pattern);
    default:
      return [];
  }
}

function read(file: string, m: Module): void {
  const { body } = parse(fs.readFileSync(file, "utf8"), {
    sourceType: "module",
    plugins: file.endsWith(".tsx") ? ["typescript", "jsx"] : ["typescript"],
  }).program;
  const { exports } = m;
  const near = (spec: string) => file_of(file, spec);
  const named = (to: string | null, name: string) =>
    to === null ? undefined : module_of(to).exports.get(name);

  // What each name at the top of the file is bound to: what it imports, then
  // its constants in the order they are written, each able to read the last.
  const bound: Names = new Map();
  for (const s of body) {
    if (s.type !== "ImportDeclaration" || s.importKind === "type") continue;
    const to = near(s.source.value);
    for (const i of s.specifiers) {
      if (i.type === "ImportNamespaceSpecifier") {
        bound.set(i.local.name, to === null ? undefined : { file: to });
      } else if (i.type === "ImportSpecifier" && i.importKind !== "type") {
        bound.set(i.local.name, named(to, name_of(i.imported)));
      }
    }
  }
  for (const s of body) {
    const d = s.type === "ExportNamedDeclaration" ? s.declaration : s;
    if (d?.type !== "VariableDeclaration" || d.kind !== "const") continue;
    for (const v of d.declarations) {
      const val = literal_of(v.init, bound);
      if (v.id.type === "Identifier" && val !== null) {
        bound.set(v.id.name, { val });
      }
    }
  }

  let literals = 0;
  let others = 0;
  // Every name the file exports itself.
  const own = new Set<string>();
  for (const s of body) {
    if (s.type === "ExportDefaultDeclaration") others++;
    if (s.type !== "ExportNamedDeclaration" || s.exportKind === "type") {
      continue;
    }
    const d = s.declaration;
    if (d?.type === "VariableDeclaration") {
      for (const v of d.declarations) {
        for (const name of names_in(v.id)) own.add(name);
        const at =
          v.id.type === "Identifier" ? bound.get(v.id.name) : undefined;
        if (
          v.id.type === "Identifier" &&
          d.kind === "const" &&
          at &&
          "val" in at
        ) {
          exports.set(v.id.name, at);
          literals++;
        } else others++;
      }
    } else if (d) {
      if ("id" in d && d.id?.type === "Identifier") own.add(d.id.name);
      others++;
    }
    const to = s.source ? near(s.source.value) : null;
    for (const e of s.specifiers) {
      own.add(name_of(e.exported));
      others++;
      if (e.type === "ExportNamespaceSpecifier") {
        if (to) exports.set(e.exported.name, { file: to });
      } else if (e.type === "ExportSpecifier" && e.exportKind !== "type") {
        const at = s.source ? named(to, e.local.name) : bound.get(e.local.name);
        if (at) exports.set(name_of(e.exported), at);
      }
    }
  }
  // A name a file exports itself, whatever its value, hides one it forwards
  // whole.
  for (const s of body) {
    if (s.type !== "ExportAllDeclaration" || s.exportKind === "type") continue;
    others++;
    const to = near(s.source.value);
    if (!to) continue;
    for (const [name, found] of module_of(to).exports) {
      if (!own.has(name) && !exports.has(name)) exports.set(name, found);
    }
  }
  m.pure = literals > 0 && others === 0;
}

function module_of(file: string): Module {
  const known = modules.get(file);
  if (known) return known;
  const m: Module = { exports: new Map(), pure: false };
  // A cycle reads what is known so far.
  modules.set(file, m);
  if (!has_variant(file)) read(file, m);
  return m;
}

export const exports_of = (file: string): Map<string, Export> =>
  module_of(file).exports;

export const is_pure = (file: string): boolean => module_of(file).pure;

/** What every file under the roots exports and this reading of them, as one
 * string, which changes when either does. A file compiled against a literal
 * does not change with it, so this goes in the transform cache's key. */
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
        hash.update(JSON.stringify([at, ...exports_of(at)]));
      }
    }
  };
  roots.forEach(walk);
  return hash.digest("hex");
}
