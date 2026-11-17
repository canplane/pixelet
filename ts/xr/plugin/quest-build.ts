// @pxl/xr/plugin/quest-build.ts

/**
 * The debug builds of both flavors that expo-horizon-core adds, `mobile` and
 * `quest`, load their JavaScript from Metro. React Native treats a variant as
 * a debug build only when its whole name is listed, which is `debug` and
 * `debugOptimized` by default, and bundles the JavaScript into the APK of
 * every other variant.
 */

import { withAppBuildGradle } from "expo/config-plugins";
import type { ConfigPlugin } from "expo/config-plugins";

const FLAVORS = ["mobile", "quest"];
const DEBUGGABLE = FLAVORS.flatMap((flavor) => [
  `${flavor}Debug`,
  `${flavor}DebugOptimized`,
]);
const MARK = "// pxl: the flavors' debug builds (plugin/quest-build.ts)";

const plugin: ConfigPlugin = (config) =>
  withAppBuildGradle(config, (config) => {
    const gradle = config.modResults.contents;
    if (gradle.includes(MARK)) return config;
    const at = gradle.search(/^react \{\n/m);
    if (at < 0) {
      throw new Error("quest-build: the app's build.gradle has no react block");
    }
    const end = gradle.indexOf("\n", at) + 1;
    const line = `    ${MARK}\n    debuggableVariants = ${JSON.stringify(
      DEBUGGABLE,
    )}\n`;
    config.modResults.contents =
      gradle.slice(0, end) + line + gradle.slice(end);
    return config;
  });

export default plugin;
