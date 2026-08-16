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

/**
 * Origin the pre-baked terrain tiles are served from, version prefix
 * included, with any trailing slashes removed so callers can always join
 * with a single `/`.
 *
 * Empty means no static tile store is configured: the client then requests
 * the live API route through the dev proxy and every page — ocean included —
 * is baked on demand. Accepts an absolute origin
 * (`https://tiles.example.com/v1`) or a same-origin path (`/tiles/v1`); the
 * latter is what avoids CORS entirely when the bucket is bound to a path on
 * the app's own hostname.
 */
export const TILE_BASE_URL: string = __TILE_BASE_URL__.replace(/\/+$/, "");

/** True when tiles come from a static store rather than the live API. Keyed
 * off separately from any dev flag because the distinction that matters is
 * "is there an API behind this" — most notably whether a 404 means "open
 * ocean, never baked" or a genuine failure. */
export const HAS_STATIC_TILES: boolean = TILE_BASE_URL.length > 0;

/**
 * Base URL for reverse geocoding, trailing slashes removed.
 *
 * Defaults to OSM's shared public Nominatim. That instance's usage policy
 * expects an identifying User-Agent, which a browser cannot set — only a
 * Referer goes out — so public demo traffic can be throttled or blocked.
 * Pointing this at a self-hosted instance or a proxy is the way out, which
 * is why it is configuration rather than a constant.
 */
export const GEOCODE_BASE_URL: string = __GEOCODE_BASE_URL__.replace(/\/+$/, "");
