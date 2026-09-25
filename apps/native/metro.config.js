// @pxl/native/metro.config.js

/**
 * The shell is apps/web/src, the same files the web bundles; what differs by
 * platform is behind `@platform/`, which here is this app's src/platform/.
 * Every other prefix is the one the Deno import maps give it, so the shell
 * is read exactly as the type-check reads it.
 */

const path = require("node:path");
const { getDefaultConfig } = require("expo/metro-config");

const root = path.resolve(__dirname, "../..");

/** An import-map prefix and the directory it names. */
const PREFIXES = [
  ["@platform/", path.join(__dirname, "src/platform/")],
  ["@/", path.join(root, "apps/web/src/")],
  ["@xpute/core/", path.join(root, "packages/xpute/core/src/")],
  ["@xpute/runtime/", path.join(root, "packages/xpute/runtime/src/")],
];

const config = getDefaultConfig(__dirname);

// The shell's files and the packages Deno installed are both outside this
// app's directory.
config.watchFolders = [root];
config.resolver.nodeModulesPaths = [path.join(root, "node_modules")];

/** Packages there may be only one of in an app: React Native and React are
 * the host the app runs on, and a library's copy of them is a second host. A
 * package manager may link a library's peer to another version it has on
 * disk, so they are found from this app wherever they are imported. */
const SINGLETONS = ["react", "react-native"];
const FROM_APP = path.join(__dirname, "index.ts");

config.resolver.resolveRequest = (context, name, platform) => {
  if (SINGLETONS.some((s) => name === s || name.startsWith(`${s}/`))) {
    return context.resolveRequest({ ...context, originModulePath: FROM_APP }, name, platform);
  }
  for (const [prefix, dir] of PREFIXES) {
    if (name.startsWith(prefix)) return context.resolveRequest(context, dir + name.slice(prefix.length), platform);
  }
  // What a build is, which Vite writes as a virtual module on the web.
  if (name === "virtual:pxl-build") return { type: "sourceFile", filePath: path.join(__dirname, "src/build.ts") };
  return context.resolveRequest(context, name, platform);
};

module.exports = config;
