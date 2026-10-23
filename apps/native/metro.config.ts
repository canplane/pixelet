// @pxl/native/metro.config.ts

/**
 * The client is apps/client/src, the same files Vite bundles for the web,
 * with a device's own beside a default by React Native's suffixes (pill.ios.tsx
 * beside pill.tsx). Every prefix is the one the Deno import maps give the
 * importing file (apps/client/script/import-map.ts), so the client is read
 * exactly as the type-check reads it.
 */

import { execSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import process from "node:process";
import { getDefaultConfig } from "expo/metro-config";
import { mapped, ROOT } from "../client/script/import-map.ts";

// The app's directory, from the root the import maps are read under: Expo
// loads this file through its own transform, which takes no `import.meta`.
const here = path.join(ROOT, "apps/native");

/** Packages there may be only one of in an app: React Native and React are
 * the host the app runs on, and a library's copy of them is a second host. A
 * package manager may link a library's peer to another version it has on
 * disk, so they are found from this app wherever they are imported. */
const SINGLETONS = ["react", "react-native"];
const FROM_APP = path.join(here, "index.ts");

/**
 * What this build is — when, from which source, under which number — as the
 * module `virtual:pxl-build` (config/deploy.ts reads it). The source is git's
 * short hash, `-dirty` where the tree had changes; the number is the one CI
 * handed the build, or else `#<last>-<id>` from the last one this checkout saw
 * published (apps/client/.build_number), since a build CI did not number is not
 * that build. Written once, when Metro starts: a dev server's stamp is the
 * one it started under.
 */
function buildStamp(): string {
  let id = "unknown";
  try {
    const git = (args: string) => execSync(`git ${args}`, { cwd: ROOT, stdio: ["ignore", "pipe", "ignore"] }).toString().trim();
    id = `${git("rev-parse --short HEAD")}${git("status --porcelain") !== "" ? "-dirty" : ""}`;
  } catch { /* not a checkout */ }
  let number = id;
  if (process.env.PXL_BUILD_NUMBER) number = `#${process.env.PXL_BUILD_NUMBER}`;
  else {
    try {
      const last = fs.readFileSync(path.join(ROOT, "apps/client/.build_number"), "utf8").trim();
      if (/^\d+$/.test(last)) number = `#${last}-${id}`;
    } catch { /* none seen yet */ }
  }
  const file = path.join(here, ".expo/pxl-build.js");
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(
    file,
    `export const time = ${JSON.stringify(new Date().toISOString())};\n` +
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
 * default is the default, which it extends. */
function ownPlatform(file: string, platform: string | null, from: string): string {
  if (!platform) return file;
  const ext = path.extname(file);
  const stem = file.slice(0, -ext.length);
  if (from.startsWith(`${stem}.`)) return file;
  for (const which of [platform, "native"]) {
    const own = `${stem}.${which}${ext}`;
    if (fs.existsSync(own)) return own;
  }
  return file;
}

const config = getDefaultConfig(here);

// The shell's files and the packages Deno installed are both outside this
// app's directory.
config.watchFolders = [ROOT];
config.resolver.nodeModulesPaths = [path.join(ROOT, "node_modules")];

config.resolver.resolveRequest = (context, name, platform) => {
  if (SINGLETONS.some((s) => name === s || name.startsWith(`${s}/`))) {
    return context.resolveRequest({ ...context, originModulePath: FROM_APP }, name, platform);
  }
  if (name === "virtual:pxl-build") return { type: "sourceFile", filePath: BUILD_STAMP };
  // A prefix is the importing file's own; a file it names, mapped or
  // relative, is taken as this platform has it.
  const from = context.originModulePath;
  const hit = mapped(from, name);
  const file = hit ? hit.path : name.startsWith(".") && path.extname(name) !== "" ? path.resolve(path.dirname(from), name) : null;
  if (file) return context.resolveRequest(context, ownPlatform(file, platform, from), platform);
  return context.resolveRequest(context, name, platform);
};

export default config;
