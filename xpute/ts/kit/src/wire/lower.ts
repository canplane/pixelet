// @xpute/kit/wire/lower.ts

// A record to a tuple and back in a schema's key order, since XTP lowers arrays, not objects.

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";

export function to_tuple<T extends Record<PropertyKey, unknown>>(
  obj: T,
  keys: readonly (keyof T)[],
): T[keyof T][] {
  return Array.from({ length: keys.length }, (_, i) => {
    const key = keys[i];
    if (!(key in obj)) throw new MarshalError(Errno.EINVAL);
    return obj[key];
  });
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
