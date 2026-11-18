// @pxl/client/plugin/generated.ts

/**
 * Holds a generated project to the config that generated it. `ios/` and
 * `android/` are written by `expo prebuild` from the app's config, its plugins
 * and the files they name, and a build compiles whatever stands there: a font
 * added to the config reaches no app until the project is generated again.
 * So prebuild leaves in each project the stamp of what it read, and each
 * build takes the stamp again before anything else and stops where the two
 * differ (script/generated.ts, which says what the stamp is of).
 *
 * The step is the first of the app's target on iOS and one the app's
 * `preBuild` waits on on Android, each running the one script.
 */

import { execFileSync } from "node:child_process";
import { join } from "node:path";

import {
  withAppBuildGradle,
  withDangerousMod,
  withXcodeProject,
} from "expo/config-plugins";
import type { ConfigPlugin } from "expo/config-plugins";

const IOS_PHASE = "pxl: the project is of the app's config";
const IOS_STEP = `if [[ -f "$PODS_ROOT/../.xcode.env" ]]; then source "$PODS_ROOT/../.xcode.env"; fi
if [[ -f "$PODS_ROOT/../.xcode.env.local" ]]; then source "$PODS_ROOT/../.xcode.env.local"; fi
"$NODE_BINARY" "$PROJECT_DIR/../script/generated.ts" ios
`;

const ANDROID_MARK =
  "// pxl: the project is of the app's config (plugin/generated.ts)";
const ANDROID_STEP = `
${ANDROID_MARK}
def pxlGenerated = tasks.register("pxlGenerated", Exec) {
    workingDir rootDir.parentFile
    commandLine "node", "script/generated.ts", "android"
}
preBuild.dependsOn(pxlGenerated)
`;

const plugin: ConfigPlugin = (config) => {
  for (const platform of ["ios", "android"] as const) {
    config = withDangerousMod(config, [
      platform,
      (config) => {
        const root = config.modRequest.projectRoot;
        execFileSync(
          process.execPath,
          [join(root, "script/generated.ts"), "--write", platform],
          { stdio: "inherit" },
        );
        return config;
      },
    ]);
  }
  config = withXcodeProject(config, (config) => {
    const project = config.modResults;
    const target = project.getFirstTarget();
    const phases = target.firstTarget.buildPhases as { comment: string }[];
    if (phases.some((p) => p.comment === IOS_PHASE)) return config;
    project.addBuildPhase(
      [],
      "PBXShellScriptBuildPhase",
      IOS_PHASE,
      target.uuid,
      { shellPath: "/bin/bash", shellScript: IOS_STEP },
    );
    phases.unshift(phases.pop()!);
    return config;
  });
  return withAppBuildGradle(config, (config) => {
    const gradle = config.modResults.contents;
    if (!gradle.includes(ANDROID_MARK)) {
      config.modResults.contents = gradle + ANDROID_STEP;
    }
    return config;
  });
};

export default plugin;
