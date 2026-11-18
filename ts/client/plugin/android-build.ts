// @pxl/client/plugin/android-build.ts

/**
 * Writes what an Android build of this app needs into the files `expo
 * prebuild` generates: the Gradle properties and the app's build script.
 * android/ is generated and not kept, so the choices are made here.
 *
 * - One ABI, arm64-v8a. The core is built for aarch64-linux-android alone
 *   (xpute's module), and an app that packs another ABI
 *   without the core dies loading its library on the phones that take that
 *   ABI.
 * - The SDK floor is set by the Vulkan that this build needs for wgpu.
 * - A build's JavaScript is bundled again when `PXL_PERF_DRIVE` changes, so a
 *   build made for measuring and a normal one share no bundle.
 */

import { withAppBuildGradle, withGradleProperties } from "expo/config-plugins";
import type { ConfigPlugin } from "expo/config-plugins";

const PROPERTIES = {
  reactNativeArchitectures: "arm64-v8a",
  "android.minSdkVersion": "29",
  "android.enableMinifyInReleaseBuilds": "true",
  "android.enableShrinkResourcesInReleaseBuilds": "true",
};

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

// pxl: a release's Rust names no directory of this machine: the flags cargo
// reads from the environment of the task that builds the core (xpute's),
// which are the prefixes the paths are then numbered under
// (script/number-paths.py)
project(":xpute-expo").tasks.matching { it.name == "buildGuestRelease" }.configureEach {
    def app = rootDir.parentFile
    def workspace = new File(app, new groovy.json.JsonSlurper().parse(new File(app, "package.json")).xpute.guest.workspace).canonicalPath
    def cmd = ["\${app}/script/number-paths.py", "--remaps", workspace].execute()
    if (cmd.waitFor() != 0) throw new GradleException("number-paths.py --remaps failed")
    environment "RUSTFLAGS", [environment.RUSTFLAGS, cmd.text.trim()].findAll().join(" ")
}
`;

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
  return withAppBuildGradle(config, (config) => {
    const gradle = config.modResults.contents;
    if (!gradle.includes(BUNDLE_INPUT_MARK)) {
      config.modResults.contents = gradle + BUNDLE_INPUT;
    }
    return config;
  });
};

export default plugin;
