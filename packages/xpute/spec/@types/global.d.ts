// @xpute/spec/../@types/global.d.ts

/**
 * Global constants injected at build time (Vite define)
 * These are replaced during bundling and are available everywhere.
 */

/**
 * Global utility types
 * These types are intentionally placed in the global scope
 * to reduce import noise for commonly used patterns.
 */

export {};

declare global {
  const __ENV__: "deno" | "node" | "browser";
  const __APP_ENV__: string;
  const __IS_DEV__: boolean;

  /** A droplet fades. A pixelet remains. 🫧 */
  const __APP_NAME__: string;

  const __ROLE__: "server" | "client";

  /** Allows `null` */
  type Nullable<T> = T | null;

  /** Allows `null` and `undefined` */
  type OptionalNullable<T> = T | null | undefined;

  /** Single value or array of values */
  type ValueOrArray<T> = T | T[];
}
