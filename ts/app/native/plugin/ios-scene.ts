// @pxl/native/plugin/ios-scene.ts

/**
 * The scene life cycle, which iOS 27 asserts at launch: an app that leaves its
 * window to the app delegate is stopped before it draws. Expo's scene delegate
 * makes the window from the scene and starts React Native in it, so the app
 * delegate only hands it the factory. Expo 57's template does not do this yet.
 */

import { withAppDelegate, withInfoPlist } from "expo/config-plugins";

const SCENE_DELEGATE = "EXExpoAppSceneDelegate";

const DECLARED = "class AppDelegate: ExpoAppDelegate {";
const STARTED = `#if os(iOS) || os(tvOS)
    window = UIWindow(frame: UIScreen.main.bounds)
    factory.startReactNative(
      withModuleName: "main",
      in: window,
      launchOptions: launchOptions)
#endif
`;

export default (config) => {
  config = withInfoPlist(config, (config) => {
    config.modResults.UIApplicationSceneManifest = {
      UIApplicationSupportsMultipleScenes: false,
      UISceneConfigurations: {
        UIWindowSceneSessionRoleApplication: [{
          UISceneConfigurationName: "Default Configuration",
          UISceneDelegateClassName: SCENE_DELEGATE,
        }],
      },
    };
    return config;
  });
  return withAppDelegate(config, (config) => {
    const source = config.modResults.contents;
    if (source.includes("ExpoReactNativeFactoryProvider")) return config;
    // Refused rather than skipped: a template that changed would launch into nothing.
    if (!source.includes(DECLARED) || !source.includes(STARTED)) {
      throw new Error(
        "ios-scene: AppDelegate.swift is not the template this plugin rewrites",
      );
    }
    config.modResults.contents = source.replace(
      DECLARED,
      "class AppDelegate: ExpoAppDelegate, ExpoReactNativeFactoryProvider {",
    ).replace(STARTED, "");
    return config;
  });
};
