// @pxl/web/config/bundle.ts

/** Whether this is a development bundle — a dev server's, not a built one:
 * `__DEV_BUNDLE__` as the bundler set it (Vite's `define`, the device app's
 * babel.config.js). Deno's tests run no bundler and have none, which reads as not. */
export const DEV_BUNDLE: boolean = typeof __DEV_BUNDLE__ !== "undefined" && __DEV_BUNDLE__ === true;
