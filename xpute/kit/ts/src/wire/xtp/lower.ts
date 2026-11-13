// @xpute/kit/wire/xtp/lower.ts

// This turns a record into a tuple in a schema's key order, and back. XTP
// writes arrays but not objects, so a schema that owns its keys writes an
// object this way. A missing key, or a tuple of the wrong length, is EINVAL.

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";

// ---- Tuple ----

// record -> tuple
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

// tuple -> record
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
