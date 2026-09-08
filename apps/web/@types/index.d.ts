// @pxl/web/../@types/index.d.ts

export {};

declare global {
  // Global constants injected at build time (Vite `define`) — see vite.config.ts.
  // The __SCREAMING__ names are read directly at their call sites on purpose:
  // they mark a compile-time constant whose branches vanish from the bundle.
  // Deployment *values* (tile/geocode origins) are read through
  // lib/env/config.ts instead — see that file for why the two differ.
  const __ENV__: "deno" | "node" | "browser";
  const __APP_ENV__: string;
  const __IS_DEV__: boolean;

  /** Origin (+ version prefix) the pre-baked terrain tiles are served from,
   * e.g. "https://tiles.example.com/v1". Empty string => no static tile
   * store configured, fall back to the live /api route. */
  const __TILE_BASE_URL__: string;

  /** Same as __TILE_BASE_URL__, for the pre-baked POI set — a sibling
   * prefix in the same bucket (e.g. ".../tiles/poi/v1"), not part of
   * __TILE_BASE_URL__'s own path. Empty string => fall back to the live
   * /api route. */
  const __POI_TILE_BASE_URL__: string;

  /** Reverse-geocoding origin, e.g. "https://nominatim.openstreetmap.org". */
  const __GEOCODE_BASE_URL__: string;

  /** Build timestamp, ISO-8601 UTC. Changes every build by construction. */
  const __BUILD_TIME__: string;

  /** A droplet fades. A pixelet remains. 🫧 */
  const __APP_NAME__: string;

  const __ROLE__: "server" | "client";
}
