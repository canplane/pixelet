// @pxl/client/deploy.ts

/**
 * Deployment values that ship as data. `__IS_DEV__` and the other build
 * constants stay bare globals where they are used: the app's build writes
 * them in as values (ts/client/babel.config.ts), so a branch on one visibly
 * drops from the bundle.
 */

import {
  id as buildId,
  number as buildNumber,
  time as buildTime,
} from "virtual:pxl-build";

/** Shown in the settings, because a phone has no URL bar to tell which build
 * is on screen. */
export const APP_ENV: string = __APP_ENV__;

/** When the build was stamped, in ISO-8601 UTC. On a dev server, it is when
 * the dev server started. */
export const BUILD_TIME: string = buildTime;

/** git's short hash of the source, with `-dirty` when the tree had changes. */
export const BUILD_ID: string = buildId;

/** `#<n>` for a build CI numbered. A build CI did not number is `#<n>-<id>`,
 * after the last number the deployment served. */
export const BUILD_NUMBER: string = buildNumber;

export const DEV_SOURCE: string = __DEV_SOURCE__;
