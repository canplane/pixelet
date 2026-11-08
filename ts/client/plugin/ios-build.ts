// @pxl/client/plugin/ios-build.ts

/**
 * Sets the iOS version a build targets, in the pods and the app alike, within
 * the files `expo prebuild` generates (ios/ is generated and not kept). The
 * target is the highest floor among the libraries the app links. Liquid
 * Glass needs a later iOS, so the app checks for it at run time.
 */

import { withPodfileProperties, withXcodeProject } from "expo/config-plugins";
import type { ConfigPlugin } from "expo/config-plugins";

const DEPLOYMENT_TARGET = "16.4";

const plugin: ConfigPlugin = (config) => {
  config = withPodfileProperties(config, (config) => {
    config.modResults["ios.deploymentTarget"] = DEPLOYMENT_TARGET;
    return config;
  });
  return withXcodeProject(config, (config) => {
    const configurations = config.modResults.pbxXCBuildConfigurationSection();
    for (const entry of Object.values(configurations) as {
      buildSettings?: Record<string, string>;
    }[]) {
      if (entry?.buildSettings?.IPHONEOS_DEPLOYMENT_TARGET !== undefined) {
        entry.buildSettings.IPHONEOS_DEPLOYMENT_TARGET = DEPLOYMENT_TARGET;
      }
    }
    return config;
  });
};

export default plugin;
