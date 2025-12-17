/**
 * Global constants injected at build time (Vite define)
 * These are replaced during bundling and are available everywhere.
 */
declare const __APP_NAME__: string; // Easter egg 🫧
declare const __IS_DEV__: boolean;

/**
 * Global utility types
 * These types are intentionally placed in the global scope
 * to reduce import noise for commonly used patterns.
 */

/** Allows `null` */
type Nullable<T> = T | null;

/** Allows `null` and `undefined` */
type OptionalNullable<T> = T | null | undefined;

/** Single value or array of values */
type ValueOrArray<T> = T | T[];
