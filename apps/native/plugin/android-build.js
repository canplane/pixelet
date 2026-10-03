// @pxl/native/plugin/android-build.js

/**
 * What an Android build of this app is for, written into the Gradle
 * properties, the app's build script and the daemon's criteria `expo
 * prebuild` generates (android/ is its, and not kept).
 *
 * - One ABI, arm64-v8a: the kernel is built for aarch64-linux-android alone
 *   (script/build-native.ts), and an ABI packed without it is an app that
 *   dies loading its library on the phones that take that ABI.
 * - The SDK floor: the Vulkan this build takes for wgpu.
 * - The debug builds of both flavors, a phone's and a Quest's (the `mobile`
 *   and `quest` expo-horizon-core adds), load their JavaScript from Metro.
 *   React Native takes a variant for a debug build only by its whole name,
 *   `debug` unless told, and bundles every other one into its APK: a
 *   `questDebug` was a Debug build that never asked Metro.
 * - Gradle's daemon on Java 17, whatever launches it: the NDK's native build
 *   of expo-modules-core fails under a newer Java ("A restricted method in
 *   java.lang.System has been called"). A shell's `JAVA_HOME` did not hold
 *   it — Android Studio's sync writes the daemon's criteria for its own
 *   bundled Java 25, and the criteria outrank `JAVA_HOME` — so the criteria
 *   are written here, where Studio finds them already said.
 */

const fs = require("node:fs");
const path = require("node:path");
const { withAppBuildGradle, withDangerousMod, withGradleProperties } = require("expo/config-plugins");

const PROPERTIES = {
  reactNativeArchitectures: "arm64-v8a",
  "android.minSdkVersion": "29",
};

const FLAVORS = ["mobile", "quest"];
const DEBUGGABLE = FLAVORS.flatMap((flavor) => [`${flavor}Debug`, `${flavor}DebugOptimized`]);
const MARK = "// pxl: the flavors' debug builds (plugin/android-build.js)";
const DAEMON_JAVA = "17";

module.exports = (config) => {
  config = withGradleProperties(config, (config) => {
    const items = config.modResults.filter((item) => !(item.type === "property" && item.key in PROPERTIES));
    for (const [key, value] of Object.entries(PROPERTIES)) items.push({ type: "property", key, value });
    config.modResults = items;
    return config;
  });
  config = withDangerousMod(config, ["android", (config) => {
    const file = path.join(config.modRequest.platformProjectRoot, "gradle", "gradle-daemon-jvm.properties");
    fs.writeFileSync(file, `# plugin/android-build.js\ntoolchainVersion=${DAEMON_JAVA}\n`);
    return config;
  }]);
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
