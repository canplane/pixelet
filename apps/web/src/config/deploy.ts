// @pxl/web/config/deploy.ts

/**
 * Deployment configuration: values that ship as data.
 *
 * `__IS_DEV__` and friends deliberately stay bare `__SCREAMING__` globals at
 * their call sites — the name says "compile-time constant, this branch
 * disappears from the bundle", which an imported identifier does not. Do not
 * centralize those here. What belongs here gates nothing away.
 *
 * `__IS_DEV__` is also not `import.meta.env.DEV`: it is the *application*
 * environment from `VITE_APP_ENV`, so a dev server started with
 * `VITE_APP_ENV=production` has `import.meta.env.DEV === true` while
 * `__IS_DEV__ === false`.
 */

import { id as build_id, number as build_number, time as build_time } from "virtual:pxl-build";

import { tileBase } from "@/config/route.ts";

/** Application environment name, from VITE_APP_ENV (falling back to Vite's
 * mode). Shown in settings so it is possible to tell which build is on screen
 * — on a phone there is no URL bar or devtools to check against. */
export const APP_ENV: string = __APP_ENV__;

/** When this bundle was built, ISO-8601 in UTC. On a dev server, when the
 * page was loaded (vite.config.ts#build_stamp). */
export const BUILD_TIME: string = build_time;

/** Which source this bundle was built from, `<hash>[-dirty]`
 * (vite.config.ts#build_id). Stamped on every ledger report and trace so a
 * measurement names what it measured, and carried in BUILD_NUMBER where the
 * build is not one CI numbered. */
export const BUILD_ID: string = build_id;

/** Which build this is, counted per deploy outside git: `#<n>`, and
 * `#<n>-<id>` for one built after it from source CI did not number
 * (vite.config.ts#build_number). */
export const BUILD_NUMBER: string = build_number;

/**
 * Whether tiles come from a published bake rather than a live API. Decides
 * whether a version token is in the path, where the dev proxy forwards, and
 * whether a 404 means "never baked" — open ocean, an empty POI page, ground
 * outside Overture's Divisions coverage — or a genuine failure.
 */
export const PUBLISHED_TILES: boolean = __PUBLISHED_TILES__;

export const TILE_BASE_URL: string = tileBase("terrain", PUBLISHED_TILES);
export const POI_TILE_BASE_URL: string = tileBase("poi", PUBLISHED_TILES);
export const GEOCODE_TILE_BASE_URL: string = tileBase("geocode", PUBLISHED_TILES);
