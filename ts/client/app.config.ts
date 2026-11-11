// @pxl/client/app.config.ts

/** The page is one HTML file over one bundle (`output: "single"`), filled from
 * the template public/index.html, which script/web.ts makes from index.html. */

/** The app's table, every path in it named from `root`, where this folder
 * stands from the app being built. A path is relative, as the table is shipped
 * inside the app. */
export function app(root: string) {
  const at = (p: string) => `${root}/${p}`;
  const icon = at("brand/icon.png");
  return {
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
        ko: at("locale/ko.json"),
        ja: at("locale/ja.json"),
        "zh-Hans": at("locale/zh-Hans.json"),
        "zh-Hant": at("locale/zh-Hant.json"),
      },
      ios: { bundleIdentifier: "me.plei.pixelet" },
      android: {
        package: "me.plei.pixelet",
        adaptiveIcon: { foregroundImage: icon },
      },
      plugins: [
        [
          "expo-location",
          {
            locationWhenInUsePermission:
              "pixelet shows you where you are in the world, and what you build stands there.",
          },
        ],
        at("plugin/prebuilt-flavor.ts"),
        at("plugin/android-build.ts"),
        at("plugin/ios-build.ts"),
        at("plugin/ios-scene.ts"),
      ],
    },
  };
}

export default app(".");
