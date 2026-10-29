// @pxl/client/@types/virtual.d.ts

/** What a build is, from vite.config.ts#buildStamp on a page and the device
 * app's metro.config.ts#buildStamp on a device: read through
 * deploy.ts, which names each. */
declare module "virtual:pxl-build" {
  /** ISO-8601 UTC. */
  export const time: string;
  /** `r<commits>-<hash>[-dirty]`, from git. */
  export const id: string;
  /** `b<n>`, `b<n>+` after it, or `b?`. */
  export const number: string;
}
