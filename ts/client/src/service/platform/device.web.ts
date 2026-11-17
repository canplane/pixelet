// @pxl/client/service/platform/device.web.ts

/** Whether the web app runs on a handheld, told by a coarse pointer, the one
 * signal every mobile browser gives. */
export const HANDHELD: boolean = matchMedia("(pointer: coarse)").matches;
