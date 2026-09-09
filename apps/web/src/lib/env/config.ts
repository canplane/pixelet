// @pxl/web/lib/env/config.ts

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

import { tile_base } from "@/lib/env/routes.ts";

/** Application environment name, from VITE_APP_ENV (falling back to Vite's
 * mode). Shown in settings so it is possible to tell which build is on screen
 * — on a phone there is no URL bar or devtools to check against. */
export const APP_ENV: string = __APP_ENV__;

/** When this bundle was built, ISO-8601 in UTC. Formatted at the point it is
 * shown rather than here, so it reads in the viewer's own timezone. */
export const BUILD_TIME: string = __BUILD_TIME__;

/**
 * Whether tiles come from a published bake rather than a live API.
 *
 * One question, asked once. It decides three things that used to derive it
 * separately and could disagree: whether a version token is in the path,
 * where the dev proxy forwards, and whether a 404 means "never baked" —
 * open ocean, an empty POI page, ground outside Overture's Divisions
 * coverage — or a genuine failure.
 */
export const PUBLISHED_TILES: boolean = __PUBLISHED_TILES__;

export const TILE_BASE_URL: string = tile_base("terrain", PUBLISHED_TILES);
export const POI_TILE_BASE_URL: string = tile_base("poi", PUBLISHED_TILES);
export const GEOCODE_TILE_BASE_URL: string = tile_base("geocode", PUBLISHED_TILES);
