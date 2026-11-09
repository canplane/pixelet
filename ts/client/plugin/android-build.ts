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
 * - A build's JavaScript is bundled again when `PXL_PERF_DRIVE` changes, so a
 *   build made for measuring and a normal one share no bundle.
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
  "android.enableMinifyInReleaseBuilds": "true",
  "android.enableShrinkResourcesInReleaseBuilds": "true",
};

const FLAVORS = ["mobile", "quest"];
const DEBUGGABLE = FLAVORS.flatMap((flavor) => [
  `${flavor}Debug`,
  `${flavor}DebugOptimized`,
]);
const MARK = "// pxl: the flavors' debug builds (plugin/android-build.ts)";
const DAEMON_JAVA = "17";
const BUNDLE_INPUT_MARK =
  "// pxl: the bundle's input (plugin/android-build.ts)";
const BUNDLE_INPUT = `
${BUNDLE_INPUT_MARK}
tasks.matching { it.name.startsWith("createBundle") }.configureEach {
    inputs.property("PXL_PERF_DRIVE", System.getenv("PXL_PERF_DRIVE") ?: "")
}

// pxl: the libraries' C++ names no directory of this machine
["\${rootDir.parentFile.parentFile.parentFile}", System.getenv("HOME")].each { dir ->
    android.defaultConfig.externalNativeBuild.cmake.cFlags("-ffile-prefix-map=\${dir}=")
    android.defaultConfig.externalNativeBuild.cmake.cppFlags("-ffile-prefix-map=\${dir}=")
}
`;

const plugin: ConfigPlugin = (cfg) => {
  cfg = withGradleProperties(cfg, (cfg) => {
    const items = cfg.modResults.filter(
      (item) => !(item.type === "property" && item.key in PROPERTIES),
    );
    for (const [key, value] of Object.entries(PROPERTIES)) {
      items.push({ type: "property", key, value });
    }
    cfg.modResults = items;
    return cfg;
  });
  cfg = withDangerousMod(cfg, [
    "android",
    (cfg) => {
      const file = path.join(
        cfg.modRequest.platformProjectRoot,
        "gradle",
        "gradle-daemon-jvm.properties",
      );
      fs.writeFileSync(
        file,
        `# plugin/android-build.ts\ntoolchainVersion=${DAEMON_JAVA}\n`,
      );
      return cfg;
    },
  ]);
  return withAppBuildGradle(cfg, (cfg) => {
    const gradle = cfg.modResults.contents;
    if (gradle.includes(MARK)) return cfg;
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
    cfg.modResults.contents =
      gradle.slice(0, end) + line + gradle.slice(end) + BUNDLE_INPUT;
    return cfg;
  });
};

export default plugin;
