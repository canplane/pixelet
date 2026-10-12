// @pxl/web/@types/index.d.ts

export {};

declare global {
  // Global constants injected at build time: Vite's `define` on a page
  // (vite.config.ts), the device app's babel.config.js on a device.
  // The __SCREAMING__ names are read directly at their call sites on purpose:
  // they mark a compile-time constant whose branches vanish from the bundle.
  // Deployment *values* are read through config/deploy.ts instead — see
  // that file for why the two differ. Route paths are not injected at all:
  // they are source, in config/route.ts.
  const __APP_ENV__: string;
  const __IS_DEV__: boolean;

  /** Whether this is a development bundle — a dev server's, not a built
   * one — which is not the same question as __IS_DEV__'s (config/deploy.ts
   * says why). Absent where no bundler ran: Deno's tests. */
  const __DEV_BUNDLE__: boolean | undefined;

  /** Where the tiles come from: `delivery`, the deployment's, or the servers
   * of `dev:oven` or `dev:shelf`, named as the task is. */
  const __DEV_SOURCE__: string;
}

/** What a build is, from vite.config.ts#buildStamp on a page and the device
 * app's metro.config.js#buildStamp on a device: read through
 * config/deploy.ts, which names each. */
declare module "virtual:pxl-build" {
  /** ISO-8601 UTC. */
  export const time: string;
  /** `r<commits>-<hash>[-dirty]`, from git. */
  export const id: string;
  /** `b<n>`, `b<n>+` after it, or `b?`. */
  export const number: string;
}
