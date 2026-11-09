// @pxl/client/@types/index.d.ts

export {};

declare global {
  // Constants that babel.config.ts writes into the code at build time, on a
  // page and on a device. A branch on one is folded away in the bundle. The
  // deployment's values are read through build.ts.
  const __APP_ENV__: string;
  const __IS_DEV__: boolean;

  /** Whether this is a development bundle, served by a dev server rather
   * than built. __IS_DEV__ turns on the development features; this says only
   * how the bundle is served. It is undefined where nothing defines it, as in
   * the tests. */
  const __DEV_BUNDLE__: boolean | undefined;

  /** Where the tiles come from: `delivery` for the deployment's, or `oven` or
   * `shelf` for the local servers of the dev tasks named after them. */
  const __DEV_SOURCE__: string;

  /** Whether this is a build made for measuring, which runs the drive at boot
   * (`PXL_PERF_DRIVE=1`). */
  const __PERF_DRIVE__: boolean;
}
