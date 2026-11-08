// @pxl/client/service/bundle.ts

/** Whether this is a dev bundle. The tests define no `__DEV_BUNDLE__`, and
 * that reads as not a dev bundle. */
export const DEV_BUNDLE: boolean =
  typeof __DEV_BUNDLE__ !== "undefined" && __DEV_BUNDLE__ === true;
