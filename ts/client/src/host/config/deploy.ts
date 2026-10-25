// @pxl/client/host/config/deploy.ts

/**
 * Deployment values that ship as data. `__IS_DEV__` and friends stay bare
 * globals at call sites, so the branch visibly drops from the bundle. It is
 * `VITE_APP_ENV`, not `import.meta.env.DEV`.
 */

import { id as buildId, number as buildNumber, time as buildTime } from "virtual:pxl-build";

/** Shown in settings: a phone has no URL bar to tell which build is on screen. */
export const APP_ENV: string = __APP_ENV__;

/** ISO-8601 UTC; on a dev server, the page load. */
export const BUILD_TIME: string = buildTime;

/** `<hash>[-dirty]`. */
export const BUILD_ID: string = buildId;

/** `#<n>`, or `#<n>-<id>` for a build after it that CI did not number. */
export const BUILD_NUMBER: string = buildNumber;

export const DEV_SOURCE: string = __DEV_SOURCE__;
