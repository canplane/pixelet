// @pxl/web/../@types/index.d.ts

export {};

declare global {
  // Global constants injected at build time (Vite `define`) — see vite.config.ts.
  // The __SCREAMING__ names are read directly at their call sites on purpose:
  // they mark a compile-time constant whose branches vanish from the bundle.
  // Deployment *values* are read through lib/env/config.ts instead — see
  // that file for why the two differ. Route paths are not injected at all:
  // they are source, in lib/env/routes.ts.
  const __ENV__: "deno" | "node" | "browser";
  const __APP_ENV__: string;
  const __IS_DEV__: boolean;

  /** Whether tiles come from a published bake rather than a live API — the
   * one thing about them the build knows and the source cannot. */
  const __PUBLISHED_TILES__: boolean;

  /** Build timestamp, ISO-8601 UTC. Changes every build by construction. */
  const __BUILD_TIME__: string;

  /** A droplet fades. A pixelet remains. 🫧 */
  const __APP_NAME__: string;

  const __ROLE__: "server" | "client";
}
