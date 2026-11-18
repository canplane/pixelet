// @pxl/client/script/generated.ts

/**
 * The stamp of what `expo prebuild` reads, and a generated project held to it
 * (plugin/generated.ts).
 *
 *   node script/generated.ts --write <ios|android>   prebuild's: stamps the project
 *   node script/generated.ts <ios|android>           a build's: stops where it differs
 *
 * The stamp is of the app's config, the config plugins it names and the
 * files they take, a font or an icon, as Expo's fingerprint finds them. What
 * a build reads for itself, a module's native sources or the bundle, is not
 * in it: a change there needs no project made again.
 */

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import fingerprint from "expo/fingerprint";

/** Why Expo's fingerprint holds a source that prebuild reads. */
const READ = ["expoConfig", "expoConfigPlugins", "expoConfigExternalFile"];

const args = process.argv.slice(2);
const platform = args.at(-1);
if (platform !== "ios" && platform !== "android") {
  console.error("generated.ts [--write] <ios|android>");
  process.exit(2);
}
process.chdir(fileURLToPath(new URL("..", import.meta.url)));
const at = `${platform}/.generated-from`;

const { sources } = await fingerprint.createFingerprintAsync(process.cwd());
const hash = createHash("sha256");
for (const h of sources
  .filter((s) => s.hash && s.reasons.some((r) => READ.includes(r)))
  .map((s) => s.hash)
  .sort()) {
  hash.update(`${h}\n`);
}
const stamp = hash.digest("hex");

if (args[0] === "--write") {
  writeFileSync(at, `${stamp}\n`);
} else {
  let was = "";
  try {
    was = readFileSync(at, "utf8").trim();
  } catch {
    // A project with no stamp was generated before there was one.
  }
  if (was !== stamp) {
    console.error(
      `error: ${platform}/ was generated from another config than the app's now. ` +
        `Generate it again: pnpm exec expo prebuild --platform ${platform} --no-install` +
        (platform === "ios" ? ", then pod install in ios/" : ""),
    );
    process.exit(1);
  }
}
