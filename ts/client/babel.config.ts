// @pxl/client/babel.config.ts

/**
 * Expo's preset, plus the build's constants written into the code as values.
 * A branch on a constant is folded where it is compiled, so what only a
 * development build uses is not in a production build at all. A release build
 * also writes each exported literal where it is read (`inline`).
 */

import * as path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import type {
  ConfigAPI,
  NodePath,
  PluginObj,
  TransformOptions,
  types as BabelTypes,
} from "@babel/core";
import {
  type Export,
  exports_of,
  file_of,
  is_pure,
} from "@pxl/build/constant.ts";
import { tile_source } from "./script/proxy.ts";

/** The part of Babel a plugin is handed. */
type Babel = { types: typeof BabelTypes };

/** A TypeScript node that holds a value, which any other holds none of. */
const TS_VALUE =
  /^TS(?:As|Satisfies|NonNull|Instantiation)Expression$|^TSTypeAssertion$/;

const in_type = (p: NodePath): boolean =>
  p.findParent(
    (a) => a.node.type.startsWith("TS") && !TS_VALUE.test(a.node.type),
  ) !== null;

/** Replaces each literal read through an import, or read in the module that
 * exports it, with its value, and strikes what that leaves with no use: the
 * import, and a module of literals named again by the file that imports it
 * (`export { ToastTone }`). A module read whole keeps its names. An export
 * no longer read by name is left for tree shaking to drop. */
const inline = ({ types: t }: Babel): PluginObj => ({
  visitor: {
    Program(p, state) {
      const from = state.filename ?? "";
      /** Writes what each read of `local`, bound to `at`, comes to, and says
       * whether a read is left that still needs the name. */
      const write = (local: string, at: Export): boolean => {
        let by_name = false;
        for (const ref of p.scope.getBinding(local)!.referencePaths) {
          if (in_type(ref)) continue;
          // A JSX tag, or the declaration's own export, takes no value.
          if (!ref.isIdentifier()) {
            by_name = true;
            continue;
          }
          const named = ref.parentPath;
          if (named?.isExportSpecifier()) {
            const { exported } = named.node;
            const listed = exports_of(from).get(
              t.isIdentifier(exported) ? exported.name : exported.value,
            );
            // Struck only where a reader of the name is written through it.
            if (
              listed &&
              "file" in listed &&
              "file" in at &&
              listed.file === at.file &&
              is_pure(at.file)
            ) {
              const clause = named.parentPath;
              named.remove();
              if (
                clause.isExportNamedDeclaration() &&
                clause.node.specifiers.length === 0
              ) {
                clause.remove();
              }
            } else by_name = true;
            continue;
          }
          let read: NodePath = ref;
          let e: Export = at;
          while ("file" in e) {
            const up: NodePath | null = read.parentPath;
            if (
              !up?.isMemberExpression({ object: read.node, computed: false })
            ) {
              break;
            }
            const next: Export | undefined = exports_of(e.file).get(
              (up.node.property as BabelTypes.Identifier).name,
            );
            if (next === undefined) break;
            e = next;
            read = up;
          }
          if ("val" in e) read.replaceWith(t.valueToNode(e.val));
          else if (read !== ref && is_pure(e.file)) {
            throw read.buildCodeFrameError(
              "a module of literals read whole through another module; import it itself",
            );
          } else by_name = true;
        }
        return by_name;
      };
      for (const decl of p.get("body")) {
        if (!decl.isImportDeclaration() || decl.node.importKind === "type") {
          continue;
        }
        const target = file_of(from, decl.node.source.value);
        if (target === null) continue;
        let struck = false;
        for (const s of decl.get("specifiers")) {
          if (s.isImportDefaultSpecifier()) continue;
          let at: Export | undefined;
          if (s.isImportSpecifier()) {
            if (s.node.importKind === "type") continue;
            const { imported } = s.node;
            at = exports_of(target).get(
              t.isIdentifier(imported) ? imported.name : imported.value,
            );
          } else {
            at = { file: target };
          }
          if (at === undefined) continue;
          const local = s.node.local.name;
          const by_name = write(local, at);
          if (
            by_name &&
            s.isImportSpecifier() &&
            "file" in at &&
            is_pure(at.file)
          ) {
            // The name it came through is struck where it was exported.
            const relative = path.relative(path.dirname(from), at.file);
            decl.insertBefore(
              t.importDeclaration(
                [t.importNamespaceSpecifier(t.identifier(local))],
                t.stringLiteral(`./${relative}`),
              ),
            );
          } else if (by_name) continue;
          s.remove();
          struck = true;
        }
        if (struck && decl.node.specifiers.length === 0) decl.remove();
      }
      for (const decl of p.get("body")) {
        if (!decl.isExportNamedDeclaration()) continue;
        const d = decl.get("declaration");
        if (!d.isVariableDeclaration()) continue;
        for (const v of d.get("declarations")) {
          const { id } = v.node;
          if (!t.isIdentifier(id)) continue;
          const at = exports_of(from).get(id.name);
          if (at !== undefined && "val" in at) write(id.name, at);
        }
      }
      p.scope.crawl();
    },
  },
});

export default (api: ConfigAPI): TransformOptions => {
  const prod = process.env.NODE_ENV === "production";
  const source = tile_source();
  // Whether this is a build made for measuring, which runs the drive at boot
  // (ui/layout.tsx).
  const drive = process.env.PXL_PERF_DRIVE === "1";
  api.cache.using(() => `${process.env.NODE_ENV} ${source} ${drive}`);

  const DEFINE = {
    __TILE_SOURCE__: source,
    __PERF_DRIVE__: drive,
  };

  /** Replaces each name in DEFINE with its value, wherever the name is not
   * bound in its own scope. */
  const define = ({ types: t }: Babel): PluginObj => ({
    visitor: {
      Identifier(p) {
        const name = p.node.name;
        if (
          !Object.hasOwn(DEFINE, name) ||
          !p.isReferencedIdentifier() ||
          p.scope.hasBinding(name)
        )
          return;
        p.replaceWith(t.valueToNode(DEFINE[name as keyof typeof DEFINE]));
      },
    },
  });

  // The preset is Expo's own copy, beside Expo, because Expo is what the app
  // depends on.
  const preset = fileURLToPath(
    new URL("../babel-preset-expo", import.meta.resolve("expo/package.json")),
  );
  // The defines apply to this tree's sources alone. A package's sources are
  // left as they are, because a name of theirs could be one of ours (@expo/ui's
  // `Slot`), and so is a module Expo makes up for a package, whose name holds
  // a NUL and is no file (`\0shim:react-native-web/...`). `exclude` is a
  // function rather than a RegExp: Metro reads the config with no file to make
  // its cache key, and Babel refuses a RegExp that it has nothing to test
  // against.
  return {
    presets: [preset],
    overrides: [
      {
        exclude: (file) => /[\\/]node_modules[\\/]|\0/.test(file ?? ""),
        plugins: prod ? [define, inline] : [define],
      },
    ],
  };
};
