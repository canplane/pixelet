// @pxl/client/metro.config.ts

/**
 * Metro bundles the client, ts/client/src, for a device and for the page. A
 * device's own file sits beside a default and is named by React Native's
 * suffixes (pill.ios.tsx beside pill.tsx); the page takes the default. Every
 * alias is tsconfig.json's, and every workspace package resolves to its source
 * (ts/build/paths.ts), so that Metro reads the client exactly as the
 * type-check reads it.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { getDefaultConfig } from "expo/metro-config";
import { REPO, treeFile, WORKSPACE } from "@pxl/build/paths.ts";
import { CONSOLE_PATH, consoleHandler } from "@pxl/build/console.ts";
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

/** Writes the build's stamp (ts/build/stamp.ts) as the module
 * `virtual:pxl-build`. It is written once, when Metro starts, so a dev
 * server's stamp is the one it started under. */
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
  // The page takes a `.web` file, or else the default. A device takes its own
  // system's file, or else the `.native` one.
  for (const which of platform === "web" ? ["web"] : [platform, "native"]) {
    const own = `${stem}.${which}${ext}`;
    if (fs.existsSync(own)) return own;
  }
  return file;
}

const base = getDefaultConfig(here);

const resolveRequest: NonNullable<typeof base.resolver.resolveRequest> = (
  context,
  name,
  platform,
) => {
  if (SINGLETONS.some((s) => name === s || name.startsWith(`${s}/`))) {
    return context.resolveRequest(
      { ...context, originModulePath: FROM_APP },
      name,
      platform,
    );
  }
  if (name === "virtual:pxl-build") {
    return { type: "sourceFile", filePath: BUILD_STAMP };
  }
  // A file that an import names, through an alias, a workspace package or a
  // relative path, is resolved to this platform's own file.
  const from = context.originModulePath;
  const file = name.startsWith(".")
    ? path.extname(name) !== ""
      ? path.resolve(path.dirname(from), name)
      : null
    : treeFile(from, name);
  if (file) {
    return context.resolveRequest(
      context,
      ownPlatform(file, platform, from),
      platform,
    );
  }
  return context.resolveRequest(context, name, platform);
};

/** A device's console lines, appended to the app's `.dev-console.log`. */
const devConsole = consoleHandler(path.join(here, ".dev-console.log"));

export default {
  ...base,
  // The UI's files, xpute's and pnpm's store all lie outside this app's
  // directory. The bundle reads nothing outside these folders.
  watchFolders: [WORKSPACE, path.join(REPO, "xpute/ts")],
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
