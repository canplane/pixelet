// @pxl/web/shared/lib/browser/handheld.ts

/**
 * Whether this device is a phone or tablet, for the few ceilings that are
 * set by memory rather than by measurement of time.
 *
 * A coarse pointer is a touch screen, which is the one signal every mobile
 * browser gives and no desktop does; user agents lie and `deviceMemory` is
 * Chrome-only. What it gates: the core memory reservation, the shadow
 * map's size and the badge sheet's largest size — each a fixed
 * allocation a phone's tab cannot afford at the desktop's size. An iPhone
 * 13 mini read 1.6 to 2.1 GB for the page with them at the desktop's
 * numbers and reloaded under memory pressure (2026-09-13).
 */
export const HANDHELD: boolean = typeof matchMedia === "function" && matchMedia("(pointer: coarse)").matches;
