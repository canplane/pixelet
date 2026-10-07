// @xpute/core/kit/comp.ts

import type { f64, i32, numeric } from "../abi/word.ts";

export type Comp<T> = (a: T, b: T) => i32;

export const comp_numeric = (a: numeric, b: numeric): i32 => (a < b ? -1 : a > b ? 1 : 0);
export const comp_str = (a: string, b: string): i32 => (a < b ? -1 : a > b ? 1 : 0);

export function make_comp<T, K extends numeric = f64>(
  key_fn: (e: T) => K,
  tie_fn?: (a: T, b: T) => i32,
): Comp<T> {
  const tie = tie_fn ?? (() => 0);

  return (a, b) => {
    const ka = key_fn(a);
    const kb = key_fn(b);
    const c = comp_numeric(ka, kb);
    return c ? c : tie(a, b);
  };
}
