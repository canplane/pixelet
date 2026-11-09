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

/** The part of Babel a plugin is handed. */
type Babel = { types: typeof BabelTypes };

/** A TypeScript node that holds a value, which any other holds none of. */
const TS_VALUE =
  /^TS(?:As|Satisfies|NonNull|Instantiation)Expression$|^TSTypeAssertion$/;

const in_type = (p: NodePath): boolean =>
  p.findParent(
    (a) => a.node.type.startsWith("TS") && !TS_VALUE.test(a.node.type),
  ) !== null;

/** Replaces each literal read through an import with its value, and strikes
 * what that leaves with no use: the import, and a module of literals named
 * again by the file that imports it (`export { Major }`). A module read whole
 * keeps its names. */
const inline = ({ types: t }: Babel): PluginObj => ({
  visitor: {
    Program(p, state) {
      const from = state.filename ?? "";
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
          let kept = false;
          for (const ref of p.scope.getBinding(local)!.referencePaths) {
            if (in_type(ref)) continue;
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
              } else kept = true;
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
            } else kept = true;
          }
          if (
            kept &&
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
          } else if (kept) continue;
          s.remove();
          struck = true;
        }
        if (struck && decl.node.specifiers.length === 0) decl.remove();
      }
      p.scope.crawl();
    },
  },
});

export default (api: ConfigAPI): TransformOptions => {
  const prod = process.env.NODE_ENV === "production";
  // Where a dev server's tiles come from (script/proxy.ts `route_target`).
  const source = process.env.PXL_TILE_SOURCE || "delivery";
  // Whether this is a build made for measuring, which runs the drive at boot
  // (ui/layout.tsx).
  const drive = process.env.PXL_PERF_DRIVE === "1";
  api.cache.using(() => `${process.env.NODE_ENV} ${source} ${drive}`);

  const DEFINE = {
    __IS_DEV__: !prod,
    __DEV_BUNDLE__: !prod,
    __APP_ENV__: prod ? "prod" : "dev",
    __DEV_SOURCE__: source,
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
  // `Slot`). `exclude` is a function rather than a RegExp: Metro reads the
  // config with no file to make its cache key, and Babel refuses a RegExp that
  // it has nothing to test against.
  return {
    presets: [preset],
    overrides: [
      {
        exclude: (file) => /[\\/]node_modules[\\/]/.test(file ?? ""),
        plugins: prod ? [define, inline] : [define],
      },
    ],
  };
};
