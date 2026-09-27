// @pxl/native/plugin/android-build.js

/**
 * What an Android build of this app is for, written into the Gradle
 * properties and the app's build script `expo prebuild` generates (android/
 * is its, and not kept).
 *
 * - One ABI, arm64-v8a: the kernel is built for aarch64-linux-android alone
 *   (script/build-native.ts), and an ABI packed without it is an app that
 *   dies loading its library on the phones that take that ABI.
 * - Android 10 (API 29) and later: the device draws through wgpu on Vulkan,
 *   which is dependable from there down to no earlier.
 * - The debug builds of both flavors, a phone's and a Quest's (the `mobile`
 *   and `quest` expo-horizon-core adds), load their JavaScript from Metro.
 *   React Native takes a variant for a debug build only by its whole name,
 *   `debug` unless told, and bundles every other one into its APK: a
 *   `questDebug` was a Debug build that never asked Metro.
 */

const { withAppBuildGradle, withGradleProperties } = require("expo/config-plugins");

const PROPERTIES = {
  reactNativeArchitectures: "arm64-v8a",
  "android.minSdkVersion": "29",
};

const FLAVORS = ["mobile", "quest"];
const DEBUGGABLE = FLAVORS.flatMap((flavor) => [`${flavor}Debug`, `${flavor}DebugOptimized`]);
const MARK = "// pxl: the flavors' debug builds (plugin/android-build.js)";

module.exports = (config) => {
  config = withGradleProperties(config, (config) => {
    const items = config.modResults.filter((item) => !(item.type === "property" && item.key in PROPERTIES));
    for (const [key, value] of Object.entries(PROPERTIES)) items.push({ type: "property", key, value });
    config.modResults = items;
    return config;
  });
  return withAppBuildGradle(config, (config) => {
    const gradle = config.modResults.contents;
    if (gradle.includes(MARK)) return config;
    const at = gradle.search(/^react \{\n/m);
    if (at < 0) throw new Error("android-build: the app's build.gradle has no react block");
    const end = gradle.indexOf("\n", at) + 1;
    const line = `    ${MARK}\n    debuggableVariants = ${JSON.stringify(DEBUGGABLE)}\n`;
    config.modResults.contents = gradle.slice(0, end) + line + gradle.slice(end);
    return config;
  });
};
