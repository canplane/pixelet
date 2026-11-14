// @pxl/client/build.ts

/**
 * The build's values read as data: its stamp and its tile source. `__DEV__`
 * and the build's constants stay bare globals where a branch is taken on
 * them: a release build writes them in as values (ts/client/babel.config.ts,
 * and Expo's Babel preset for `__DEV__`), so the branch visibly drops from
 * the bundle.
 */

import {
  id as buildId,
  number as buildNumber,
  time as buildTime,
} from "virtual:pxl-build";

/** When the build was stamped, in ISO-8601 UTC. On a dev server, it is when
 * the dev server started. */
export const BUILD_TIME: string = buildTime;

/** git's short hash of the source, with `-dirty` when the tree had changes. */
export const BUILD_ID: string = buildId;

/** `#<n>` for a build CI numbered. A build CI did not number is `#<n>-<id>`,
 * after the last number the deployment served. */
export const BUILD_NUMBER: string = buildNumber;

export const TILE_SOURCE: string = __TILE_SOURCE__;
