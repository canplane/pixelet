// @pxl/client/babel.config.ts

/**
 * Expo's preset, plus the build's constants written into the code as values.
 * A branch on a constant is folded where it is compiled, so what only a
 * development build uses is not in a production build at all.
 */

import process from "node:process";
import { fileURLToPath } from "node:url";
import type {
  ConfigAPI,
  PluginObj,
  TransformOptions,
  types as BabelTypes,
} from "@babel/core";

/** The part of Babel a plugin is handed. */
type Babel = { types: typeof BabelTypes };

export default (api: ConfigAPI): TransformOptions => {
  const production = process.env.NODE_ENV === "production";
  // Where a dev server's tiles come from (script/proxy.ts `route_target`).
  const source = process.env.PXL_DEV_SOURCE || "delivery";
  api.cache.using(() => `${process.env.NODE_ENV} ${source}`);

  const DEFINE = {
    __IS_DEV__: !production,
    __DEV_BUNDLE__: !production,
    __APP_ENV__: production ? "production" : "development",
    __DEV_SOURCE__: source,
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
        plugins: [define],
      },
    ],
  };
};
