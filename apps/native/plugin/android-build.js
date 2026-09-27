// @pxl/native/plugin/android-build.js

/**
 * What an Android build of this app is for, written into the Gradle
 * properties `expo prebuild` generates (android/ is its, and not kept).
 *
 * - One ABI, arm64-v8a: the kernel is built for aarch64-linux-android alone
 *   (script/build-native.ts), and an ABI packed without it is an app that
 *   dies loading its library on the phones that take that ABI.
 * - Android 10 (API 29) and later: the device draws through wgpu on Vulkan,
 *   which is dependable from there down to no earlier.
 */

const { withGradleProperties } = require("expo/config-plugins");

const PROPERTIES = {
  reactNativeArchitectures: "arm64-v8a",
  "android.minSdkVersion": "29",
};

module.exports = (config) =>
  withGradleProperties(config, (config) => {
    const items = config.modResults.filter((item) => !(item.type === "property" && item.key in PROPERTIES));
    for (const [key, value] of Object.entries(PROPERTIES)) items.push({ type: "property", key, value });
    config.modResults = items;
    return config;
  });
