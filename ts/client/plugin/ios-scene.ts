// @pxl/client/plugin/ios-scene.ts

/**
 * Moves the app to the scene life cycle, which iOS 27 asserts at launch: an
 * app that leaves its window to the app delegate is stopped before it draws.
 * Expo's scene delegate makes the window from the scene and starts React
 * Native in it, so the app delegate only hands it the factory.
 */

import { withAppDelegate, withInfoPlist } from "expo/config-plugins";
import type { ConfigPlugin } from "expo/config-plugins";

const SCENE_DELEGATE = "EXExpoAppSceneDelegate";

const DELEGATE_CLASS = "class AppDelegate: ExpoAppDelegate {";
const WINDOW_START = `#if os(iOS) || os(tvOS)
    window = UIWindow(frame: UIScreen.main.bounds)
    factory.startReactNative(
      withModuleName: "main",
      in: window,
      launchOptions: launchOptions)
#endif
`;

const plugin: ConfigPlugin = (cfg) => {
  cfg = withInfoPlist(cfg, (cfg) => {
    cfg.modResults.UIApplicationSceneManifest = {
      UIApplicationSupportsMultipleScenes: false,
      UISceneConfigurations: {
        UIWindowSceneSessionRoleApplication: [
          {
            UISceneConfigurationName: "Default Configuration",
            UISceneDelegateClassName: SCENE_DELEGATE,
          },
        ],
      },
    };
    return cfg;
  });
  return withAppDelegate(cfg, (cfg) => {
    const source = cfg.modResults.contents;
    if (source.includes("ExpoReactNativeFactoryProvider")) return cfg;
    // A template this plugin does not know is refused rather than skipped,
    // because a changed template would launch into an empty screen.
    if (!source.includes(DELEGATE_CLASS) || !source.includes(WINDOW_START)) {
      throw new Error(
        "ios-scene: AppDelegate.swift is not the template this plugin rewrites",
      );
    }
    cfg.modResults.contents = source
      .replace(
        DELEGATE_CLASS,
        "class AppDelegate: ExpoAppDelegate, ExpoReactNativeFactoryProvider {",
      )
      .replace(WINDOW_START, "");
    return cfg;
  });
};

export default plugin;
