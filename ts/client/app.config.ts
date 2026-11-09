// @pxl/client/app.config.ts

/** The page is one HTML file over one bundle (`output: "single"`), filled from
 * the template public/index.html, which script/web.ts makes from index.html. */

// Relative, as this table is shipped inside the app.
const icon = "./brand/icon.png";

export default {
  expo: {
    name: "pixelet",
    slug: "pixelet",
    version: "0.0.0",
    orientation: "portrait",
    userInterfaceStyle: "light",
    icon,
    platforms: ["ios", "android", "web"],
    web: { bundler: "metro", output: "single" },
    locales: {
      ko: "./locale/ko.json",
      ja: "./locale/ja.json",
      "zh-Hans": "./locale/zh-Hans.json",
      "zh-Hant": "./locale/zh-Hant.json",
    },
    ios: { bundleIdentifier: "me.plei.pixelet" },
    android: {
      package: "me.plei.pixelet",
      adaptiveIcon: { foregroundImage: icon },
    },
    plugins: [
      ["expo-horizon-core", { supportedDevices: "quest3|quest3s" }],
      [
        "expo-horizon-location",
        {
          locationWhenInUsePermission:
            "pixelet shows you where you are in the world, and what you build stands there.",
        },
      ],
      "./plugin/prebuilt-flavor.ts",
      "./plugin/android-build.ts",
      "./plugin/ios-build.ts",
      "./plugin/ios-scene.ts",
    ],
  },
};
