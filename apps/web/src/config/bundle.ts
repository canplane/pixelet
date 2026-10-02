// @pxl/web/config/bundle.ts

/** Deno's tests run no bundler, which reads as not a dev bundle. */
export const DEV_BUNDLE: boolean = typeof __DEV_BUNDLE__ !== "undefined" && __DEV_BUNDLE__ === true;
