// @pxl/web/@types/index.d.ts

export {};

declare global {
  // Global constants injected at build time (Vite `define`) — see vite.config.ts.
  // The __SCREAMING__ names are read directly at their call sites on purpose:
  // they mark a compile-time constant whose branches vanish from the bundle.
  // Deployment *values* are read through config/deploy.ts instead — see
  // that file for why the two differ. Route paths are not injected at all:
  // they are source, in config/route.ts.
  const __ENV__: "deno" | "node" | "browser" | "native";
  const __APP_ENV__: string;
  const __IS_DEV__: boolean;

  /** Whether this is a development bundle — a dev server's, not a built
   * one — which is not the same question as __IS_DEV__'s (config/deploy.ts
   * says why). Absent where no bundler ran: Deno's tests. */
  const __DEV_BUNDLE__: boolean | undefined;

  /** Whether tiles come from a published bake rather than a live API — the
   * one thing about them the build knows and the source cannot. */
  const __PUBLISHED_TILES__: boolean;

  /** A droplet fades. A pixelet remains. 🫧 */
  const __APP_NAME__: string;

  const __ROLE__: "server" | "client";
}

/** What a build is, from vite.config.ts#build_stamp: read through
 * config/deploy.ts, which names each. */
declare module "virtual:pxl-build" {
  /** ISO-8601 UTC. */
  export const time: string;
  /** `r<commits>-<hash>[-dirty]`, from git. */
  export const id: string;
  /** `b<n>`, `b<n>+` after it, or `b?`. */
  export const number: string;
}
