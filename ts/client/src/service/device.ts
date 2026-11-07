// @pxl/client/service/device.ts

/** A coarse pointer: the one signal every mobile browser gives. Gates the
 * fixed allocations an iPhone 13 mini reloaded under at desktop sizes. A
 * worker has no `matchMedia`: the page tells its own (runtime/page.ts). */
export const handheld: boolean = matchMedia("(pointer: coarse)").matches;
