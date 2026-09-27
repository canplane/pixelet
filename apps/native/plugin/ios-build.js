// @pxl/native/plugin/ios-build.js

/**
 * What an iOS build of this app is for, written into what `expo prebuild`
 * generates (ios/ is its, and not kept): iOS 26 and later, the pods and the
 * app alike. The device draws through wgpu on Metal as the page draws
 * through WebGPU, and the chrome is the system's of the same release — its
 * controls and its symbols — so nothing earlier is a device it is made for.
 */

const { withPodfileProperties, withXcodeProject } = require("expo/config-plugins");

const DEPLOYMENT_TARGET = "26.0";

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
