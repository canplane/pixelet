// @pxl/client/metro.config.ts

/**
 * Metro bundles the client, ts/client/src, for a device and for the page. A
 * device's own file sits beside a default and is named by React Native's
 * suffixes (pill.ios.tsx beside pill.tsx); the page takes the default. Every
 * alias is tsconfig.json's, and every workspace package resolves to its source
 * (ts/build/path.ts), so that Metro reads the client exactly as the
 * type-check reads it.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { getDefaultConfig } from "expo/metro-config";
import { REPO, treeFile, WORKSPACE } from "@pxl/build/path.ts";
import { CONSOLE_PATH, consoleHandler } from "@pxl/build/console.ts";
import { digest } from "@pxl/build/constant.ts";
import { stamp } from "@pxl/build/stamp.ts";
import { forward, type Middleware, route_target } from "./script/proxy.ts";

// The app's directory is found from the workspace that the aliases are read
// under, because Expo loads this file through its own transform, which does
// not allow `import.meta`.
const here = path.join(WORKSPACE, "client");

/** Packages an app may hold only one copy of. React Native and React are the
 * host the app runs on, and a library's own copy of them would be a second
 * host. A package manager may link a library's peer to another version it has
 * on disk, so these are resolved from this app wherever they are imported. */
const SINGLETONS = ["react", "react-native"];
const FROM_APP = path.join(here, "index.ts");

/** The stamp is written once, when Metro starts, so a dev server's stamp is
 * the one it started under. */
function buildStamp(): string {
  const { time, id, number } = stamp();
  const file = path.join(here, ".expo/pxl-build.js");
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(
    file,
    `export const time = ${JSON.stringify(time)};\n` +
      `export const id = ${JSON.stringify(id)};\n` +
      `export const number = ${JSON.stringify(number)};\n`,
  );
  return file;
}
const BUILD_STAMP = buildStamp();

/** The file that the platform being built takes for `file`: `pill.ios.tsx`,
 * or else `motion.native.tsx`, beside the default it names, or the default
 * when neither exists. Every import names the default with its extension,
 * which Metro's own platform lookup does not handle, so the lookup is done
 * here. A variant's import of its own default gets the default, which the
 * variant extends. Metro caches the answer for a relative name by folder, and
 * for an alias across the whole bundle. So a variant must name its default by
 * `./`, and nothing else in its folder may: otherwise the variant is handed
 * the other file's cached answer, which is the variant itself. */
function ownPlatform(
  file: string,
  platform: string | null,
  from: string,
): string {
  if (!platform) return file;
  const ext = path.extname(file);
  const stem = file.slice(0, -ext.length);
  if (from.startsWith(`${stem}.`)) return file;
  for (const which of platform === "web" ? ["web"] : [platform, "native"]) {
    const own = `${stem}.${which}${ext}`;
    if (fs.existsSync(own)) return own;
  }
  return file;
}

// A release build drops what is imported and not used, as a bundler of ES
// modules does. Expo reads the switches from the environment.
process.env.EXPO_UNSTABLE_METRO_OPTIMIZE_GRAPH = "1";
process.env.EXPO_UNSTABLE_TREE_SHAKING = "1";

const base = getDefaultConfig(here);

/** A release build compiles each exported literal into the files that read
 * it (babel.config.ts), so the transforms it may reuse depend on them. */
const LITERALS =
  process.env.NODE_ENV === "production"
    ? digest([path.join(here, "src"), path.join(REPO, "xpute/ts")])
    : "";

const resolveRequest: NonNullable<typeof base.resolver.resolveRequest> = (
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
  const from = ctx.originModulePath;
  const file = name.startsWith(".")
    ? path.extname(name) !== ""
      ? path.resolve(path.dirname(from), name)
      : null
    : treeFile(from, name);
  if (file) {
    return ctx.resolveRequest(ctx, ownPlatform(file, platform, from), platform);
  }
  return ctx.resolveRequest(ctx, name, platform);
};

const devConsole = consoleHandler(path.join(here, ".dev-console.log"));

/** Metro's caches, its transforms' and its file map's, lie in the checkout's
 * tmp/ (AGENTS.md), each tree's its own, where Expo would put them in the
 * system's temporary folder. The store is the kind Expo makes, rooted there;
 * Expo does not export its class, so it is taken from Expo's own store. */
const CACHE = path.join(REPO, "tmp/metro");
fs.mkdirSync(CACHE, { recursive: true });
const Store = base.cacheStores[0].constructor as new (options: {
  root: string;
}) => (typeof base.cacheStores)[number];

export default {
  ...base,
  // The UI's files, xpute's and pnpm's store all lie outside this app's
  // directory. The bundle reads nothing outside these folders.
  watchFolders: [WORKSPACE, path.join(REPO, "xpute/ts")],
  cacheStores: [new Store({ root: CACHE })],
  // A build made for measuring differs in what its modules compile to
  // (babel.config.ts), so it shares no transform with any other.
  cacheVersion: `${base.cacheVersion ?? ""}${process.env.PXL_PERF_DRIVE ?? ""}${LITERALS}`,
  fileMapCacheDirectory: CACHE,
  resolver: { ...base.resolver, resolveRequest },
  server: {
    ...base.server,
    enhanceMiddleware:
      (metro: Middleware): Middleware =>
      (req, res, next) => {
        const url = req.url ?? "/";
        if (url.startsWith(CONSOLE_PATH)) return devConsole(req, res);
        const target = route_target(url);
        if (target) return forward(target, req, res);
        metro(req, res, next);
      },
  },
};
