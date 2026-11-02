// @pxl/client/@types/virtual.d.ts

/** What a build is, from vite.config.ts#buildStamp on a page and the device
 * app's metro.config.ts#buildStamp on a device: read through
 * deploy.ts, which names each. */
declare module "virtual:pxl-build" {
  export const time: string;
  export const id: string;
  export const number: string;
}
