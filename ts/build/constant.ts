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
export type Export = { value: Literal } | { file: string };

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
export function fileOf(from: string, spec: string): string | null {
  const base = spec.startsWith(".")
    ? path.resolve(path.dirname(from), spec)
    : treeFile(from, spec);
  if (base === null) return null;
  for (const at of [base, `${base}.ts`, `${base}.tsx`]) {
    if (fs.existsSync(at) && fs.statSync(at).isFile()) return at;
  }
  return null;
}

function hasVariant(file: string): boolean {
  const ext = path.extname(file);
  const stem = file.slice(0, -ext.length);
  return PLATFORMS.some((p) => fs.existsSync(`${stem}.${p}${ext}`));
}

const nameOf = (n: Name): string => ("name" in n ? n.name : n.value);

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
function literalOf(
  expression: Init | Node | undefined,
  names: Names,
): Literal | null {
  switch (expression?.type) {
    case "NumericLiteral":
    case "StringLiteral":
    case "BooleanLiteral":
      return expression.value;
    case "Identifier": {
      const at = names.get(expression.name);
      return at !== undefined && "value" in at ? at.value : null;
    }
    case "MemberExpression": {
      if (expression.computed || expression.object.type !== "Identifier")
        return null;
      const at = names.get(expression.object.name);
      if (at === undefined || "value" in at) return null;
      if (expression.property.type !== "Identifier") return null;
      const found = moduleOf(at.file).exports.get(expression.property.name);
      return found !== undefined && "value" in found ? found.value : null;
    }
    case "UnaryExpression": {
      const a = literalOf(expression.argument, names);
      const f = UNARY[expression.operator];
      return typeof a === "number" && f ? finite(f(a)) : null;
    }
    case "BinaryExpression": {
      const a = literalOf(expression.left, names);
      const b = literalOf(expression.right, names);
      if (
        typeof a === "string" &&
        typeof b === "string" &&
        expression.operator === "+"
      ) {
        return a + b;
      }
      const f = BINARY[expression.operator];
      return typeof a === "number" && typeof b === "number" && f
        ? finite(f(a, b))
        : null;
    }
    case "TSAsExpression":
    case "TSSatisfiesExpression":
      return literalOf(expression.expression, names);
    default:
      return null;
  }
}

/** A pattern a declaration binds through, at any depth, or an array's hole. */
type Pattern = Extract<
  Declaration["declarations"][number]["id"],
  { type: "ArrayPattern" }
>["elements"][number];

/** The names a declaration's pattern binds. */
function namesIn(p: Pattern): string[] {
  switch (p?.type) {
    case "Identifier":
      return [p.name];
    case "ObjectPattern":
      return p.properties.flatMap((q) =>
        namesIn(q.type === "RestElement" ? q.argument : (q.value as Pattern)),
      );
    case "ArrayPattern":
      return p.elements.flatMap(namesIn);
    case "AssignmentPattern":
      return namesIn(p.left);
    case "RestElement":
      return namesIn(p.argument);
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
  const near = (spec: string) => fileOf(file, spec);
  const named = (to: string | null, name: string) =>
    to === null ? undefined : moduleOf(to).exports.get(name);

  // What each name at the top of the file is bound to: what it imports, then
  // its constants in the order they are written, each able to read the last.
  const bound: Names = new Map();
  for (const statement of body) {
    if (
      statement.type !== "ImportDeclaration" ||
      statement.importKind === "type"
    )
      continue;
    const to = near(statement.source.value);
    for (const specifier of statement.specifiers) {
      if (specifier.type === "ImportNamespaceSpecifier") {
        bound.set(specifier.local.name, to === null ? undefined : { file: to });
      } else if (
        specifier.type === "ImportSpecifier" &&
        specifier.importKind !== "type"
      ) {
        bound.set(specifier.local.name, named(to, nameOf(specifier.imported)));
      }
    }
  }
  for (const statement of body) {
    const declaration =
      statement.type === "ExportNamedDeclaration"
        ? statement.declaration
        : statement;
    if (
      declaration?.type !== "VariableDeclaration" ||
      declaration.kind !== "const"
    )
      continue;
    for (const v of declaration.declarations) {
      const value = literalOf(v.init, bound);
      if (v.id.type === "Identifier" && value !== null) {
        bound.set(v.id.name, { value });
      }
    }
  }

  let literals = 0;
  let others = 0;
  // Every name the file exports itself.
  const own = new Set<string>();
  for (const statement of body) {
    if (statement.type === "ExportDefaultDeclaration") others++;
    if (
      statement.type !== "ExportNamedDeclaration" ||
      statement.exportKind === "type"
    ) {
      continue;
    }
    const declaration = statement.declaration;
    if (declaration?.type === "VariableDeclaration") {
      for (const v of declaration.declarations) {
        for (const name of namesIn(v.id)) own.add(name);
        const at =
          v.id.type === "Identifier" ? bound.get(v.id.name) : undefined;
        if (
          v.id.type === "Identifier" &&
          declaration.kind === "const" &&
          at &&
          "value" in at
        ) {
          exports.set(v.id.name, at);
          literals++;
        } else others++;
      }
    } else if (declaration) {
      if ("id" in declaration && declaration.id?.type === "Identifier")
        own.add(declaration.id.name);
      others++;
    }
    const to = statement.source ? near(statement.source.value) : null;
    for (const specifier of statement.specifiers) {
      own.add(nameOf(specifier.exported));
      others++;
      if (specifier.type === "ExportNamespaceSpecifier") {
        if (to) exports.set(nameOf(specifier.exported), { file: to });
      } else if (
        specifier.type === "ExportSpecifier" &&
        specifier.exportKind !== "type"
      ) {
        const local = nameOf(specifier.local);
        const at = statement.source ? named(to, local) : bound.get(local);
        if (at) exports.set(nameOf(specifier.exported), at);
      }
    }
  }
  // A name a file exports itself, whatever its value, hides one it forwards
  // whole.
  for (const statement of body) {
    if (
      statement.type !== "ExportAllDeclaration" ||
      statement.exportKind === "type"
    )
      continue;
    others++;
    const to = near(statement.source.value);
    if (!to) continue;
    for (const [name, found] of moduleOf(to).exports) {
      if (!own.has(name) && !exports.has(name)) exports.set(name, found);
    }
  }
  m.pure = literals > 0 && others === 0;
}

function moduleOf(file: string): Module {
  const known = modules.get(file);
  if (known) return known;
  const m: Module = { exports: new Map(), pure: false };
  // A cycle reads what is known so far.
  modules.set(file, m);
  if (!hasVariant(file)) read(file, m);
  return m;
}

export const exportsOf = (file: string): Map<string, Export> =>
  moduleOf(file).exports;

export const isPure = (file: string): boolean => moduleOf(file).pure;

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
        hash.update(JSON.stringify([at, ...exportsOf(at)]));
      }
    }
  };
  roots.forEach(walk);
  return hash.digest("hex");
}
