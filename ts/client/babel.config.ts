// @pxl/client/babel.config.ts

/**
 * Expo's preset, and the build's constants written into the code as values,
 * so a branch on one is folded where it is compiled and what only a
 * development build uses is not in a production one at all.
 */

import process from "node:process";
import { fileURLToPath } from "node:url";
import type {
  ConfigAPI,
  PluginObj,
  TransformOptions,
  types as BabelTypes,
} from "@babel/core";

/** What a plugin is handed of Babel. */
type Babel = { types: typeof BabelTypes };

export default (api: ConfigAPI): TransformOptions => {
  const production = process.env.NODE_ENV === "production";
  // Where a dev server's tiles come from (metro.config.ts `route_target`).
  const source = process.env.PXL_DEV_SOURCE || "delivery";
  api.cache.using(() => `${process.env.NODE_ENV} ${source}`);

  const DEFINE = {
    __IS_DEV__: !production,
    __DEV_BUNDLE__: !production,
    __APP_ENV__: production ? "production" : "development",
    __DEV_SOURCE__: source,
  };

  /** Replaces each free name above with its value. */
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

  // Expo's own copy of the preset, beside Expo, which is what the app
  // depends on.
  const preset = fileURLToPath(
    new URL("../babel-preset-expo", import.meta.resolve("expo/package.json")),
  );
  // The defines are this tree's: a package's own sources are left as they
  // are, where a name of theirs could be one of ours (@expo/ui's `Slot`).
  // A function and not a RegExp: Metro reads the config with no file for its
  // cache key, and Babel refuses a RegExp it has nothing to test against.
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
