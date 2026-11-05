// @pxl/client/@types/virtual.d.ts

/** What a build is, from metro.config.ts#buildStamp (ts/build/stamp.ts): read
 * through deploy.ts, which names each. */
declare module "virtual:pxl-build" {
  export const time: string;
  export const id: string;
  export const number: string;
}

/** A stylesheet the page imports for its document (src/page.ts), which
 * Metro bundles as CSS and nothing reads as a module. */
declare module "*.css";
