// @xpute/spec/lib/kit/comp.ts

import type { numeric } from "./word.ts";

export type Comp<T> = (a: T, b: T) => number;

export const comp_numeric = (a: numeric, b: numeric): number => (a < b ? -1 : a > b ? 1 : 0);
export const comp_str = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0);

export function make_comp<T, K extends numeric = number>(
  key_fn: (e: T) => K,
  tie_fn?: (a: T, b: T) => number, // tie_fn 없으면 0 (ties: implementation-defined ordering)
): Comp<T> {
  const tie = tie_fn ?? (() => 0);

  return (a, b) => {
    const ka = key_fn(a);
    const kb = key_fn(b);
    const c = comp_numeric(ka, kb);
    return c ? c : tie(a, b);
  };
}

export const CLAMP = <T extends numeric>(x: T, min: T, max: T) => {
  if (x < min) return min;
  if (x > max) return max;
  return x;
};
