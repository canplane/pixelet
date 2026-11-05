// @pxl/client/@types/index.d.ts

export {};

declare global {
  // Global constants injected at build time by babel.config.ts, on a page and
  // on a device: each a compile-time constant whose branches vanish from the
  // bundle. Deployment values are read through deploy.ts.
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
