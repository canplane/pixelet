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
import { type Export, exportsOf, fileOf, isPure } from "@pxl/build/constant.ts";
import { tileSource } from "./script/proxy.ts";

/** The part of Babel a plugin is handed. */
type Babel = { types: typeof BabelTypes };

/** A TypeScript node that holds a value, which any other holds none of. */
const TS_VALUE =
  /^TS(?:As|Satisfies|NonNull|Instantiation)Expression$|^TSTypeAssertion$/;

const inType = (p: NodePath): boolean =>
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
    Program(program, state) {
      const from = state.filename ?? "";
      /** Writes what each read of `local`, bound to `at`, comes to, and says
       * whether a read is left that still needs the name. */
      const write = (local: string, at: Export): boolean => {
        let byName = false;
        for (const ref of program.scope.getBinding(local)!.referencePaths) {
          if (inType(ref)) continue;
          // A JSX tag, or the declaration's own export, takes no value.
          if (!ref.isIdentifier()) {
            byName = true;
            continue;
          }
          const named = ref.parentPath;
          if (named?.isExportSpecifier()) {
            const { exported } = named.node;
            const listed = exportsOf(from).get(
              t.isIdentifier(exported) ? exported.name : exported.value,
            );
            // Struck only where a reader of the name is written through it.
            if (
              listed &&
              "file" in listed &&
              "file" in at &&
              listed.file === at.file &&
              isPure(at.file)
            ) {
              const clause = named.parentPath;
              named.remove();
              if (
                clause.isExportNamedDeclaration() &&
                clause.node.specifiers.length === 0
              ) {
                clause.remove();
              }
            } else byName = true;
            continue;
          }
          let read: NodePath = ref;
          let resolved: Export = at;
          while ("file" in resolved) {
            const up: NodePath | null = read.parentPath;
            if (
              !up?.isMemberExpression({ object: read.node, computed: false })
            ) {
              break;
            }
            const next: Export | undefined = exportsOf(resolved.file).get(
              (up.node.property as BabelTypes.Identifier).name,
            );
            if (next === undefined) break;
            resolved = next;
            read = up;
          }
          if ("value" in resolved)
            read.replaceWith(t.valueToNode(resolved.value));
          else if (read !== ref && isPure(resolved.file)) {
            throw read.buildCodeFrameError(
              "a module of literals read whole through another module; import it itself",
            );
          } else byName = true;
        }
        return byName;
      };
      for (const statement of program.get("body")) {
        if (
          !statement.isImportDeclaration() ||
          statement.node.importKind === "type"
        ) {
          continue;
        }
        const target = fileOf(from, statement.node.source.value);
        if (target === null) continue;
        let struck = false;
        for (const specifier of statement.get("specifiers")) {
          if (specifier.isImportDefaultSpecifier()) continue;
          let at: Export | undefined;
          if (specifier.isImportSpecifier()) {
            if (specifier.node.importKind === "type") continue;
            const { imported } = specifier.node;
            at = exportsOf(target).get(
              t.isIdentifier(imported) ? imported.name : imported.value,
            );
          } else {
            at = { file: target };
          }
          if (at === undefined) continue;
          const local = specifier.node.local.name;
          const byName = write(local, at);
          if (
            byName &&
            specifier.isImportSpecifier() &&
            "file" in at &&
            isPure(at.file)
          ) {
            // The name it came through is struck where it was exported.
            const relative = path.relative(path.dirname(from), at.file);
            statement.insertBefore(
              t.importDeclaration(
                [t.importNamespaceSpecifier(t.identifier(local))],
                t.stringLiteral(`./${relative}`),
              ),
            );
          } else if (byName) continue;
          specifier.remove();
          struck = true;
        }
        if (struck && statement.node.specifiers.length === 0)
          statement.remove();
      }
      for (const statement of program.get("body")) {
        if (!statement.isExportNamedDeclaration()) continue;
        const declaration = statement.get("declaration");
        if (!declaration.isVariableDeclaration()) continue;
        for (const v of declaration.get("declarations")) {
          const { id } = v.node;
          if (!t.isIdentifier(id)) continue;
          const at = exportsOf(from).get(id.name);
          if (at !== undefined && "value" in at) write(id.name, at);
        }
      }
      program.scope.crawl();
    },
  },
});

export default (api: ConfigAPI): TransformOptions => {
  const production = process.env.NODE_ENV === "production";
  const source = tileSource();
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
        plugins: production ? [define, inline] : [define],
      },
    ],
  };
};
