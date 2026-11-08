// @pxl/client/service/device.ts

/** Whether the page runs on a handheld, told by a coarse pointer, the one
 * signal every mobile browser gives. It gates the fixed allocations, which at
 * desktop sizes make an iPhone 13 mini reload the page. A worker has no
 * `matchMedia`, so the page tells its worker (runtime/page.ts). */
export const handheld: boolean = matchMedia("(pointer: coarse)").matches;
