// @pxl/client/plugin/ios-build.ts

/**
 * Sets the iOS version a build targets, in the pods and the app alike, within
 * the files `expo prebuild` generates (ios/ is generated and not kept). The
 * target is the highest floor among the libraries the app links. Liquid
 * Glass needs a later iOS, so the app checks for it at run time. A release
 * ships no symbols, which a build without an archive otherwise leaves in, and
 * its C++ names no directory of this machine.
 */

import {
  withPodfile,
  withPodfileProperties,
  withXcodeProject,
} from "expo/config-plugins";
import type { ConfigPlugin } from "expo/config-plugins";

const DEPLOYMENT_TARGET = "16.4";

const PREFIX_MARK = "# pxl: a release's C++ names no directory of this machine";
const PREFIX_STEP = `
    ${PREFIX_MARK} (plugin/ios-build.ts)
    prefix_maps = [File.expand_path('../../..', __dir__), ENV['HOME']].map { |dir| "-ffile-prefix-map=#{dir}=" }
    installer.pods_project.targets.each do |target|
      target.build_configurations.each do |config|
        next unless config.name == 'Release'
        %w[OTHER_CFLAGS OTHER_CPLUSPLUSFLAGS].each do |key|
          flags = config.build_settings[key] || '$(inherited)'
          flags = flags.split if flags.is_a?(String)
          config.build_settings[key] = flags + prefix_maps
        end
      end
    end
`;

const plugin: ConfigPlugin = (config) => {
  config = withPodfile(config, (config) => {
    const podfile = config.modResults.contents;
    if (podfile.includes(PREFIX_MARK)) return config;
    const call = podfile.indexOf("react_native_post_install(");
    const close = podfile.indexOf("\n    )\n", call);
    if (call < 0 || close < 0) {
      throw new Error(
        "ios-build: the Podfile has no react_native_post_install",
      );
    }
    const end = close + "\n    )\n".length;
    config.modResults.contents =
      podfile.slice(0, end) + PREFIX_STEP.slice(1) + podfile.slice(end);
    return config;
  });
  config = withPodfileProperties(config, (config) => {
    config.modResults["ios.deploymentTarget"] = DEPLOYMENT_TARGET;
    return config;
  });
  return withXcodeProject(config, (config) => {
    const configurations = config.modResults.pbxXCBuildConfigurationSection();
    for (const entry of Object.values(configurations) as {
      name?: string;
      buildSettings?: Record<string, string>;
    }[]) {
      if (entry?.name === "Release" && entry.buildSettings) {
        entry.buildSettings.DEPLOYMENT_POSTPROCESSING = "YES";
        entry.buildSettings.STRIP_INSTALLED_PRODUCT = "YES";
      }
      if (entry?.buildSettings?.IPHONEOS_DEPLOYMENT_TARGET !== undefined) {
        entry.buildSettings.IPHONEOS_DEPLOYMENT_TARGET = DEPLOYMENT_TARGET;
      }
    }
    return config;
  });
};

export default plugin;
