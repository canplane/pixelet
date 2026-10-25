// @pxl/native/babel.config.ts

/**
 * Expo's preset, and the build's constants written into the code as values
 * — what vite.config.ts's `define` did — so a branch on one is folded where
 * it is compiled and what only a development build uses is not in a
 * production one at all. They are values and not globals for that reason:
 * `__IS_DEV__ ? EVERY : SHIPPED` read at run time carries both.
 */

import * as path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { enumAt, readEnums, stale as staleIn } from "@pxl/build/const-enum.ts";
import { REPO, WORKSPACE } from "@pxl/build/import-map.ts";

/**
 * The const enums of the shell, the xpute packages and this app, read by the
 * table both bundlers share (ts/build/const-enum.ts): a const enum is
 * a compile-time name for a number, and a bundle holds the number alone. A
 * transpiler that sees one file at a time cannot inline a name declared in
 * another, and Babel's TypeScript transform emits the enum as an object
 * instead — every command's and errno's name as a string, reverse mapping
 * and all. The table refuses an enum it cannot inline; `inlineEnums` below
 * does the rest.
 */
const TABLE = readEnums([path.join(WORKSPACE, "client/src"), path.join(REPO, "xpute/ts"), path.join(WORKSPACE, "app/native/src")]);
const { exported: EXPORTED, local: LOCAL } = TABLE;

/** The table is read once, when Metro loads this file, so an enum or a member
 * added since — a spec regenerated under a running Metro — is not in it. Left
 * alone, it bundles as a name nothing binds and throws on the device; said
 * here, it stops the build with what to do. */
const stale = (what: string) => staleIn(what, "restart Metro with --clear");

/** Replaces each `Enum.Member` with its value and strikes the declarations,
 * so the import that named the enum is left with no use and goes with the
 * TypeScript transform's other type-only imports. A file's own enums are
 * replaced in that file only. An enum that is not const, or a use of one as a
 * value that is left — `Command[x]`, the enum handed on whole — is a build
 * error, never a bundle that reaches a device and throws or carries a name. */
const inlineEnums = ({ types: t }) => ({
  visitor: {
    MemberExpression(p, state) {
      const { object, property, computed } = p.node;
      if (computed || !t.isIdentifier(object) || !t.isIdentifier(property)) return;
      // A struck declaration takes its binding with it, so a use below one
      // of the file's own is found by its name.
      const binding = p.scope.getBinding(object.name);
      const members = !binding
        ? state.own.get(object.name)
        : state.declared.has(binding.identifier)
        ? state.declared.get(binding.identifier)
        : binding.path.isImportSpecifier()
        ? state.imports.get(object.name)
        : undefined;
      if (!members) return;
      if (!members.has(property.name)) throw p.buildCodeFrameError(stale(`${object.name}.${property.name}`));
      p.replaceWith(t.valueToNode(members.get(property.name)));
    },
    TSEnumDeclaration(p, state) {
      const name = p.node.id.name;
      if (!p.node.const) throw p.buildCodeFrameError(`const-enum: enum ${name} is not const, so a bundle would carry its members' names; declare it \`const enum\``);
      if (!state.declared.has(p.node.id)) throw p.buildCodeFrameError(stale(`const enum ${name}`));
      // The declaration itself first: a path struck only through its parent
      // still reads as live to the plugins after this one on the same node,
      // and the TypeScript transform turns it into a declaration with no
      // parent.
      const parent = p.parentPath;
      p.remove();
      if (parent.isExportNamedDeclaration()) parent.remove();
    },
    Program: {
      // The file's own declarations, by the identifier that names them, read
      // before any is struck: the traversal reaches a declaration before the
      // uses below it, and a struck path answers no question about itself.
      enter(p, state) {
        state.declared = new Map();
        const local = LOCAL.get(state.filename) ?? new Map();
        const exported = EXPORTED.get(state.filename) ?? new Map();
        p.traverse({
          TSEnumDeclaration(d) {
            if (!d.node.const) return;
            const members = (d.parentPath.isExportNamedDeclaration() ? exported : local).get(d.node.id.name);
            if (members) state.declared.set(d.node.id, members);
          },
        });
        state.own = new Map([...state.declared].map(([id, members]) => [id.name, members]));
        // Every import that names a const enum, by its local name.
        state.imports = new Map();
        p.traverse({
          ImportSpecifier(s) {
            const members = enumAt(TABLE, state.filename, s.parentPath.node.source.value, s.node.imported.name ?? s.node.imported.value);
            if (members) state.imports.set(s.node.local.name, members);
          },
        });
        state.names = new Set([...state.imports.keys(), ...state.own.keys()]);
      },
      exit(p, state) {
        p.traverse({
          ReferencedIdentifier(r) {
            if (!state.names.has(r.node.name) || r.parentPath.isTSTypeReference() || r.parentPath.isTSQualifiedName()) return;
            if (r.findParent((a) => a.isTSType() || a.isTSTypeAnnotation() || a.isTSTypeParameterInstantiation())) return;
            const binding = r.scope.getBinding(r.node.name);
            if (binding && !binding.path.isImportSpecifier()) return;
            throw r.buildCodeFrameError(`const enum ${r.node.name} is used as a value in ${state.filename}; it cannot be inlined`);
          },
        });
      },
    },
  },
});

export default (api) => {
  const production = process.env.NODE_ENV === "production";
  api.cache.using(() => process.env.NODE_ENV);

  const DEFINE = {
    __IS_DEV__: !production,
    __DEV_BUNDLE__: !production,
    __APP_ENV__: production ? "production" : "development",
    __DEV_SOURCE__: "delivery",
  };

  /** Replaces each free name above with its value. */
  const define = ({ types: t }) => ({
    visitor: {
      Identifier(p) {
        const name = p.node.name;
        if (!Object.hasOwn(DEFINE, name) || !p.isReferencedIdentifier() || p.scope.hasBinding(name)) return;
        p.replaceWith(t.valueToNode(DEFINE[name]));
      },
    },
  });

  // Expo's own copy of the preset, beside Expo, which is what the app
  // depends on.
  const preset = fileURLToPath(new URL("../babel-preset-expo", import.meta.resolve("expo/package.json")));
  // The defines and the enums are this tree's: a package's own sources are
  // left as they are, where a name of theirs could be one of ours — @expo/ui
  // has a `Slot` of its own, which the enum pass took for the mode bar's.
  // A function and not a RegExp: Metro reads the config with no file for its
  // cache key, and Babel refuses a RegExp it has nothing to test against.
  return { presets: [preset], overrides: [{ exclude: (file) => /[\\/]node_modules[\\/]/.test(file ?? ""), plugins: [define, inlineEnums] }] };
};
