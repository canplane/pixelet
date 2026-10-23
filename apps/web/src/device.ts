// @pxl/web/device.ts

/** A coarse pointer: the one signal every mobile browser gives. Gates the
 * fixed allocations an iPhone 13 mini reloaded under at desktop sizes. */
export const HANDHELD: boolean = typeof matchMedia === "function" && matchMedia("(pointer: coarse)").matches;
