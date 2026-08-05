// @xpute/core/kit/type.ts

export type Async<T> = T | Promise<T>;

/** Allows `null`. */
export type Nullable<T> = T | null;
