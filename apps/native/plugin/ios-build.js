// @pxl/native/plugin/ios-build.js

/**
 * The iOS a build targets, written into what `expo prebuild` generates
 * (ios/ is its, and not kept), the pods and the app alike: the highest floor
 * of what it links — Expo's modules, @expo/ui, expo-glass-effect and
 * expo-symbols, at 16.4. Liquid Glass is asked for at run time.
 */

const { withPodfileProperties, withXcodeProject } = require("expo/config-plugins");

const DEPLOYMENT_TARGET = "16.4";

module.exports = (config) => {
  config = withPodfileProperties(config, (config) => {
    config.modResults["ios.deploymentTarget"] = DEPLOYMENT_TARGET;
    return config;
  });
  return withXcodeProject(config, (config) => {
    const configurations = config.modResults.pbxXCBuildConfigurationSection();
    for (const entry of Object.values(configurations)) {
      if (entry?.buildSettings?.IPHONEOS_DEPLOYMENT_TARGET !== undefined) {
        entry.buildSettings.IPHONEOS_DEPLOYMENT_TARGET = DEPLOYMENT_TARGET;
      }
    }
    return config;
  });
};
