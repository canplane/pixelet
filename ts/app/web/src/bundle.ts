// @pxl/web/bundle.ts

/** The tests define no `__DEV_BUNDLE__`, which reads as not a dev bundle. */
export const DEV_BUNDLE: boolean =
  typeof __DEV_BUNDLE__ !== "undefined" && __DEV_BUNDLE__ === true;
