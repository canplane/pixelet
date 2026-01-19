// @pxl/api/lib/env/mod.ts

import * as mustv from "./must.ts";

export function get(key: string): string | undefined {
  return Deno.env.get(key);
}

function wrap_err(key: string, err: unknown): never {
  const msg = err instanceof Error ? err.message : String(err);
  throw new Error(`env:${key}:${msg}`);
}

export function must(key: string, opts: mustv.MustOptions = {}): string {
  try {
    return mustv.must(get(key), opts);
  } catch (err) {
    wrap_err(key, err);
  }
}

export function must_int(key: string, opts: mustv.MustIntOptions = {}): number {
  try {
    return mustv.must_int(get(key), opts);
  } catch (err) {
    wrap_err(key, err);
  }
}

export function must_bool(key: string, opts: mustv.MustBoolOptions = {}): boolean {
  try {
    return mustv.must_bool(get(key), opts);
  } catch (err) {
    wrap_err(key, err);
  }
}

export function must_match(key: string, re: RegExp, msg?: string): string {
  try {
    return mustv.must_match(get(key), re, msg);
  } catch (err) {
    wrap_err(key, err);
  }
}

export function must_one_of<T extends readonly string[]>(
  key: string,
  allowed: T,
  fallback?: T[number],
): T[number] {
  try {
    return mustv.must_one_of(get(key), allowed, fallback);
  } catch (err) {
    wrap_err(key, err);
  }
}
