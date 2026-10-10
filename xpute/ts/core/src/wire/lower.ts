// @xpute/core/wire/lower.ts

// A record to a tuple and back in a schema's key order, since XTP lowers arrays, not objects.

import { Errno } from "@xpute/core/status/errno.spec.ts";
import { MarshalError } from "@xpute/core/status/error.ts";

export function to_tuple<T extends Record<PropertyKey, unknown>>(
  obj: T,
  keys: readonly (keyof T)[],
): T[keyof T][] {
  const len = keys.length;
  const out: T[keyof T][] = new Array(len);

  for (let i = 0; i < len; i++) {
    const key = keys[i];
    if (!(key in obj)) throw new MarshalError(Errno.EINVAL);
    out[i] = obj[key];
  }
  return out;
}

export function from_tuple<T extends Record<PropertyKey, unknown>>(
  vals: readonly unknown[],
  keys: readonly (keyof T)[],
): T {
  if (vals.length !== keys.length) {
    throw new MarshalError(Errno.EINVAL);
  }

  const out = {} as T;
  for (let i = 0; i < keys.length; i++) {
    out[keys[i]] = vals[i] as T[keyof T];
  }
  return out;
}
