// @pxl/client/@types/virtual.d.ts

/** What the build is, written by metro.config.ts `build_stamp` from
 * ts/build/stamp.ts. Code reads it through build.ts, which says what each
 * field means. */
declare module "virtual:pxl-build" {
  export const time: string;
  export const id: string;
  export const number: string;
}

/** A stylesheet the page imports for its document (src/page.ts). Metro
 * bundles it as CSS, and nothing reads it as a module. */
declare module "*.css";
