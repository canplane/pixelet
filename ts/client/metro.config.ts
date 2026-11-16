// @pxl/client/metro.config.ts

/**
 * Metro bundles the client, ts/client/src, for a device and for the page. A
 * platform's own file is named by React Native's suffixes: the page's
 * motion.web.tsx beside a device's motion.native.tsx, and an override beside
 * its default (pill.ios.tsx beside pill.tsx). Which file each takes, every
 * alias, which is tsconfig.json's, and every workspace package, which
 * resolves to its source, are ts/build/path.ts's, so that Metro reads the
 * client exactly as the type-check reads it.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { getDefaultConfig } from "expo/metro-config";
import { platform_file, REPO, WORKSPACE } from "@pxl/build/path.ts";
import { digest } from "@pxl/build/constant.ts";
import { stamp } from "@pxl/build/stamp.ts";
import {
  forward,
  type Middleware,
  route_target,
  tile_source,
} from "./script/proxy.ts";

// The app's directory is found from the workspace that the aliases are read
// under, because Expo loads this file through its own transform, which does
// not allow `import.meta`.
const APP = path.join(WORKSPACE, "client");

/** Packages an app may hold only one copy of. React Native, React and Expo are
 * the host the app runs on, and a library's own copy of them would be a second
 * host. A package manager may link a library's peer to another version it has
 * on disk, or to another set of its own peers (Expo without the app's
 * @expo/metro-runtime, for a package that does not take it), so these are
 * resolved from this app wherever they are imported. */
const SINGLETONS = ["react", "react-native", "expo"];
const FROM_APP = path.join(APP, "index.ts");

/** The stamp is written once, when Metro starts, so a dev server's stamp is
 * the one it started under. */
function build_stamp(): string {
  const { time, id, number } = stamp();
  const file = path.join(APP, ".expo/pxl-build.js");
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(
    file,
    `export const time = ${JSON.stringify(time)};\n` +
      `export const id = ${JSON.stringify(id)};\n` +
      `export const number = ${JSON.stringify(number)};\n`,
  );
  return file;
}
const BUILD_STAMP = build_stamp();

// A release build drops what is imported and not used, as a bundler of ES
// modules does. Expo reads the switches from the environment.
process.env.EXPO_UNSTABLE_METRO_OPTIMIZE_GRAPH = "1";
process.env.EXPO_UNSTABLE_TREE_SHAKING = "1";

const BASE = getDefaultConfig(APP);

/** xpute's packages, as xpute's pnpm-workspace.yaml lists them: one
 * `  - <dir>` a line under `packages:`, up to a blank line, each with a named
 * package.json and a src/. Anything else there refuses the file. */
function xpute_packages(): string[] {
  const file = path.join(REPO, "xpute/pnpm-workspace.yaml");
  const after = fs.readFileSync(file, "utf8").split("\npackages:\n")[1];
  if (after === undefined) throw new Error(`${file}: no \`packages:\``);
  const lines = after.split("\n");
  const end = lines.indexOf("");
  const dirs = (end < 0 ? lines : lines.slice(0, end)).map((l) => {
    const m = l.match(/^ {2}- ([A-Za-z0-9_./-]+)$/);
    if (!m) throw new Error(`${file}: \`${l}\` is not \`  - <dir>\``);
    const dir = path.join(REPO, "xpute", m[1]);
    const manifest = JSON.parse(
      fs.readFileSync(path.join(dir, "package.json"), "utf8"),
    );
    if (typeof manifest.name !== "string")
      throw new Error(`${dir}/package.json: no name`);
    if (!fs.statSync(path.join(dir, "src")).isDirectory())
      throw new Error(`${dir}: no src/`);
    return dir;
  });
  if (dirs.length === 0) throw new Error(`${file}: lists no package`);
  return dirs;
}
const XPUTE = xpute_packages();

/** A release build compiles each exported literal into the files that read
 * it (babel.config.ts), so the transforms it may reuse depend on them. */
const LITERALS =
  process.env.NODE_ENV === "production"
    ? digest([path.join(APP, "src"), ...XPUTE])
    : "";

const resolve_request: NonNullable<typeof BASE.resolver.resolveRequest> = (
  ctx,
  name,
  platform,
) => {
  if (SINGLETONS.some((s) => name === s || name.startsWith(`${s}/`))) {
    return ctx.resolveRequest(
      { ...ctx, originModulePath: FROM_APP },
      name,
      platform,
    );
  }
  if (name === "virtual:pxl-build") {
    return { type: "sourceFile", filePath: BUILD_STAMP };
  }
  const file = platform_file(ctx.originModulePath, name, platform);
  return ctx.resolveRequest(ctx, file ?? name, platform);
};

/** Metro's caches, its transforms' and its file map's, lie in the checkout's
 * tmp/ (AGENTS.md), each tree's its own, where Expo would put them in the
 * system's temporary folder. The store is the kind Expo makes, rooted there;
 * Expo does not export its class, so it is taken from Expo's own store. */
const CACHE = path.join(REPO, "tmp/metro");
fs.mkdirSync(CACHE, { recursive: true });
const Store = BASE.cacheStores[0].constructor as new (options: {
  root: string;
}) => (typeof BASE.cacheStores)[number];

export default {
  ...BASE,
  // The UI's files, xpute's and pnpm's store all lie outside this app's
  // directory. The bundle reads nothing outside these folders.
  watchFolders: [WORKSPACE, ...XPUTE],
  cacheStores: [new Store({ root: CACHE })],
  // A build made for measuring, or for another tile source, differs in what
  // its modules compile to (babel.config.ts's defines), so it shares no
  // transform with any other.
  cacheVersion: `${BASE.cacheVersion ?? ""}${process.env.PXL_PERF_DRIVE ?? ""}${tile_source()}${LITERALS}`,
  fileMapCacheDirectory: CACHE,
  resolver: { ...BASE.resolver, resolveRequest: resolve_request },
  server: {
    ...BASE.server,
    enhanceMiddleware:
      (metro: Middleware): Middleware =>
      (req, res, next) => {
        const url = req.url ?? "/";
        const target = route_target(url);
        if (target) return forward(target, req, res);
        metro(req, res, next);
      },
  },
};
