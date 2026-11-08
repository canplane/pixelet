// @pxl/client/plugin/android-build.ts

/**
 * Writes what an Android build of this app needs into the files `expo
 * prebuild` generates: the Gradle properties, the app's build script and the
 * Gradle daemon's criteria. android/ is generated and not kept, so the
 * choices are made here.
 *
 * - One ABI, arm64-v8a. The core is built for aarch64-linux-android alone
 *   (@xpute/native script/core-build.py), and an app that packs another ABI
 *   without the core dies loading its library on the phones that take that
 *   ABI.
 * - The SDK floor is set by the Vulkan that this build needs for wgpu.
 * - The debug builds of both flavors, a phone's and a Quest's (the `mobile`
 *   and `quest` flavors that expo-horizon-core adds), load their JavaScript
 *   from Metro. React Native treats a variant as a debug build only when its
 *   whole name is listed, which is `debug` by default, and bundles the
 *   JavaScript into the APK of every other variant.
 * - Gradle's daemon runs on Java 17, whatever launches it, because the NDK
 *   build of expo-modules-core fails under a newer Java ("A restricted method
 *   in java.lang.System has been called"). Android Studio's sync writes the
 *   daemon's criteria for its own bundled Java 25, and the criteria outrank
 *   `JAVA_HOME`, so this plugin writes them.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import {
  withAppBuildGradle,
  withDangerousMod,
  withGradleProperties,
} from "expo/config-plugins";
import type { ConfigPlugin } from "expo/config-plugins";

const PROPERTIES = {
  reactNativeArchitectures: "arm64-v8a",
  "android.minSdkVersion": "29",
};

const FLAVORS = ["mobile", "quest"];
const DEBUGGABLE = FLAVORS.flatMap((flavor) => [
  `${flavor}Debug`,
  `${flavor}DebugOptimized`,
]);
const MARK = "// pxl: the flavors' debug builds (plugin/android-build.ts)";
const DAEMON_JAVA = "17";

const plugin: ConfigPlugin = (config) => {
  config = withGradleProperties(config, (config) => {
    const items = config.modResults.filter(
      (item) => !(item.type === "property" && item.key in PROPERTIES),
    );
    for (const [key, value] of Object.entries(PROPERTIES)) {
      items.push({ type: "property", key, value });
    }
    config.modResults = items;
    return config;
  });
  config = withDangerousMod(config, [
    "android",
    (config) => {
      const file = path.join(
        config.modRequest.platformProjectRoot,
        "gradle",
        "gradle-daemon-jvm.properties",
      );
      fs.writeFileSync(
        file,
        `# plugin/android-build.ts\ntoolchainVersion=${DAEMON_JAVA}\n`,
      );
      return config;
    },
  ]);
  return withAppBuildGradle(config, (config) => {
    const gradle = config.modResults.contents;
    if (gradle.includes(MARK)) return config;
    const at = gradle.search(/^react \{\n/m);
    if (at < 0) {
      throw new Error(
        "android-build: the app's build.gradle has no react block",
      );
    }
    const end = gradle.indexOf("\n", at) + 1;
    const line = `    ${MARK}\n    debuggableVariants = ${JSON.stringify(
      DEBUGGABLE,
    )}\n`;
    config.modResults.contents =
      gradle.slice(0, end) + line + gradle.slice(end);
    return config;
  });
};

export default plugin;
