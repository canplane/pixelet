// @pxl/client/@types/index.d.ts

export {};

declare global {
  // Constants that babel.config.ts writes into the code at build time, on the
  // web and natively. A branch on one is folded away in the bundle. The
  // deployment's values are read through build.ts.

  /** Where the tiles come from: `delivery` for the deployment's, or `oven` or
   * `shelf` for the local servers of the dev tasks named after them. */
  const __TILE_SOURCE__: string;

  /** Whether this is a build made for measuring, which runs the drive at boot
   * (`PXL_PERF_DRIVE=1`). */
  const __PERF_DRIVE__: boolean;

  namespace NodeJS {
    interface Require {
      /** The address of a worker's bundle, which Metro builds from `name`. */
      unstable_resolveWorker(name: string): string;
    }
  }
}
