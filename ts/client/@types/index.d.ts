// @pxl/client/@types/index.d.ts

export {};

declare global {
  // Global constants injected at build time: Vite's `define` on a page
  // (vite.config.ts), the device app's babel.config.ts on a device.
  // The __SCREAMING__ names are read directly at their call sites on purpose:
  // they mark a compile-time constant whose branches vanish from the bundle.
  // Deployment *values* are read through deploy.ts instead — see
  // that file for why the two differ. Route paths are not injected at all:
  // they are source, in namespace/route.ts.
  const __APP_ENV__: string;
  const __IS_DEV__: boolean;

  /** Whether this is a development bundle — a dev server's, not a built
   * one — which is not the same question as __IS_DEV__'s (deploy.ts
   * says why). Absent where none is defined: the tests. */
  const __DEV_BUNDLE__: boolean | undefined;

  /** Where the tiles come from: `delivery`, the deployment's, or the servers
   * of `dev:oven` or `dev:shelf`, named as the task is. */
  const __DEV_SOURCE__: string;
}
