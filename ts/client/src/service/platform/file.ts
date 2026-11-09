// @pxl/client/service/platform/file.ts

/** A page has no way to fetch a file past JavaScript, and needs none: its
 * runtime is off the UI's thread already. */

import type { NativeFetch } from "./types.ts";

export const native_fetch: NativeFetch | undefined = undefined;
