// @pxl/web/lib/env/config.ts

/**
 * Deployment configuration — the one place these values are read and the one
 * place their meaning, default, and reason for existing are written down.
 *
 * **Not everything injected by Vite belongs here.** The split is deliberate:
 *
 * - `__IS_DEV__` and friends stay bare `__DOUBLE_UNDERSCORE__` globals, read
 *   directly at their call sites. The screaming name is the point: it says
 *   "compile-time constant, this branch disappears from the bundle", and a
 *   plain imported identifier does not say that. Verified — a production
 *   build contains no trace of the dev-only routes or debug markers those
 *   gates guard. Do not "centralize" them into this module; the visual
 *   signal is worth more than the tidiness.
 * - Values below are different in kind. They ship as strings, gate nothing
 *   away, and have defaults and deployment caveats that need somewhere to
 *   live. Scattering them across call sites is what this module prevents.
 *
 * Also distinct from `import.meta.env.DEV`, which is Vite's own build mode
 * and is what strips development-only instrumentation (see chunk_stage.tsx's
 * timing blocks). `__IS_DEV__` is the *application* environment, from
 * `VITE_APP_ENV`; a dev server started with `VITE_APP_ENV=production` has
 * `import.meta.env.DEV === true` while `__IS_DEV__ === false`.
 */

/** Application environment name, from VITE_APP_ENV (falling back to Vite's
 * mode). Shown in settings so it is possible to tell which build is on screen
 * — on a phone there is no URL bar or devtools to check against. */
export const APP_ENV: string = __APP_ENV__;

/** When this bundle was built, ISO-8601 in UTC. Formatted at the point it is
 * shown rather than here, so it reads in the viewer's own timezone. */
export const BUILD_TIME: string = __BUILD_TIME__;

/** A base is the live API when it starts with this, a published bucket otherwise. */
const API_PREFIX = "/api/";

/**
 * The prefix a tile URL is built from — `<base>/<page_idx>`, bucket or live
 * API. Both forms are in apps/web/.env.example. Trailing slashes are
 * stripped so callers join with a single one. An absolute origin works as
 * well as a same-origin path.
 */
export const TILE_BASE_URL: string = __TILE_BASE_URL__.replace(/\/+$/, "");

/** Whether a 404 means "open ocean, never baked" or a genuine failure. */
export const HAS_STATIC_TILES: boolean = !TILE_BASE_URL.startsWith(API_PREFIX);

/** A sibling prefix in the same bucket, not inside terrain's. */
export const POI_TILE_BASE_URL: string = __POI_TILE_BASE_URL__.replace(/\/+$/, "");

/** Whether a 404 means "no POI on this page" or a genuine failure. */
export const HAS_STATIC_POI: boolean = !POI_TILE_BASE_URL.startsWith(API_PREFIX);

export const GEOCODE_TILE_BASE_URL: string = __GEOCODE_TILE_BASE_URL__.replace(/\/+$/, "");

/** Whether a 404 means "outside Overture's Divisions coverage" or a genuine
 * failure. */
export const HAS_STATIC_GEOCODE: boolean = !GEOCODE_TILE_BASE_URL.startsWith(API_PREFIX);
