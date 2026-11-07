// @pxl/client/metro.config.ts

/**
 * The client is ts/client/src, for a device and for the page, with a
 * device's own beside a default by React Native's suffixes (pill.ios.tsx
 * beside pill.tsx), the page taking the default. Every alias is tsconfig.json's and every workspace package
 * resolves to its source (ts/build/paths.ts), so the client is read exactly as
 * the type-check reads it.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import { getDefaultConfig } from "expo/metro-config";
import { REPO, treeFile, WORKSPACE } from "@pxl/build/paths.ts";
import { CONSOLE_PATH, consoleHandler } from "@pxl/build/console.ts";
import { stamp } from "@pxl/build/stamp.ts";
import { forward, type Middleware, route_target } from "./script/proxy.ts";

// The app's directory, from the workspace the import maps are read under: Expo
// loads this file through its own transform, which takes no `import.meta`.
const here = path.join(WORKSPACE, "client");

/** Packages there may be only one of in an app: React Native and React are
 * the host the app runs on, and a library's copy of them is a second host. A
 * package manager may link a library's peer to another version it has on
 * disk, so they are found from this app wherever they are imported. */
const SINGLETONS = ["react", "react-native"];
const FROM_APP = path.join(here, "index.ts");

/** The build's stamp (ts/build/stamp.ts) as the module `virtual:pxl-build`,
 * written once, when Metro starts: a dev server's stamp is the one it started
 * under. */
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

/** A file as the system being built has it: `pill.ios.tsx`, else
 * `motion.native.tsx`, beside the default it names, and the default where
 * neither is. Every import
 * names the default with its extension, which Metro's own platform lookup
 * does not take, so it is looked for here. A variant's own import of its
 * default is the default, which it extends. Metro keeps a relative name's
 * answer by folder and an alias's for all: a variant names its default by `./`
 * and nothing else in its folder does, or it is handed the other's answer, itself. */
function ownPlatform(
  file: string,
  platform: string | null,
  from: string,
): string {
  if (!platform) return file;
  const ext = path.extname(file);
  const stem = file.slice(0, -ext.length);
  if (from.startsWith(`${stem}.`)) return file;
  // The page takes the default; a device its own system's, else `native`.
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
  // A file an import names, through an alias, a workspace package or
  // relative, is taken as this platform has it.
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

/** A device's console, appended to the app's `.dev-console.log`. */
const devConsole = consoleHandler(path.join(here, ".dev-console.log"));

export default {
  ...base,
  // The UI's files, xpute's and pnpm's store are all outside this app's
  // directory; the bundle reads nothing else.
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
