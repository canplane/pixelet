// @pxl/native/plugin/prebuilt-flavor.js

/**
 * Makes a `pod install` leave React Native's prebuilt parts marked as of no
 * configuration, so the next build extracts the one it is for.
 *
 * The core, its dependencies and Hermes are each one folder holding either
 * the Debug or the Release build, with a file saying which
 * (.last_build_configuration in Pods/React-Core-prebuilt,
 * Pods/ReactNativeDependencies, and Pods itself for Hermes); a phase of
 * every build extracts the other when they differ. `pod install` makes the folders
 * again without that file and keeps what was extracted, and a missing file
 * is read as Debug. A Release build followed by an install then left the
 * Release core under a Debug app, whose link fails on what only the Debug
 * core defines (`facebook::react::Sealable`). Saying neither makes the next
 * build of either extract its own, one extraction after an install.
 *
 * A config plugin, because `ios/` is `expo prebuild`'s and not kept: this
 * is what puts the step into the Podfile it writes.
 */

const { withPodfile } = require("expo/config-plugins");

const MARK = "# pxl: React Native's prebuilt parts, of no configuration";

const STEP = `
    ${MARK} (plugin/prebuilt-flavor.js)
    pods = installer.sandbox.root.to_s
    { 'React-Core-prebuilt' => 'React-Core-prebuilt', 'ReactNativeDependencies' => 'ReactNativeDependencies', 'hermes-engine' => '.' }.each do |pod, marked|
      File.write(File.join(pods, marked, '.last_build_configuration'), 'None') if File.directory?(File.join(pods, pod))
    end
`;

module.exports = (config) =>
  withPodfile(config, (config) => {
    const podfile = config.modResults.contents;
    if (!podfile.includes(MARK)) {
      const at = podfile.indexOf("post_install do |installer|");
      if (at < 0) throw new Error("prebuilt-flavor: the Podfile has no post_install");
      const end = podfile.indexOf("\n", at) + 1;
      config.modResults.contents = podfile.slice(0, end) + STEP.slice(1) + podfile.slice(end);
    }
    return config;
  });
