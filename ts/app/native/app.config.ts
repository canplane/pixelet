// @pxl/native/app.config.ts

/**
 * The app as Expo builds it. The icon is the client's brand, found from the
 * workspace the import maps are read under (ts/client/script/import-map.ts).
 */

import * as path from "node:path";
import { WORKSPACE } from "../../client/script/import-map.ts";

const icon = path.join(WORKSPACE, "client/brand/icon.png");

export default {
  expo: {
    name: "pixelet",
    slug: "pixelet",
    version: "0.0.0",
    orientation: "portrait",
    userInterfaceStyle: "light",
    icon,
    platforms: ["ios", "android"],
    ios: { bundleIdentifier: "me.plei.pixelet" },
    android: { package: "me.plei.pixelet", adaptiveIcon: { foregroundImage: icon } },
    plugins: [
      ["expo-horizon-core", { supportedDevices: "quest3|quest3s" }],
      ["expo-horizon-location", { locationWhenInUsePermission: "pixelet shows you where you are on the map, and what you build stands there." }],
      "./plugin/prebuilt-flavor.ts",
      "./plugin/android-build.ts",
      "./plugin/ios-build.ts",
      "./plugin/ios-scene.ts",
    ],
  },
};
