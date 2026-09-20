// @pxl/web/config/mod.ts

import * as mustv from "./must.ts";

export function get(key: string): string | undefined {
  return import.meta.env[key];
}

function wrapErr(key: string, err: unknown): never {
  const msg = err instanceof Error ? err.message : String(err);
  throw new Error(`env:${key}:${msg}`);
}

export function must(key: string, opts: mustv.MustOptions = {}): string {
  try {
    return mustv.must(get(key), opts);
  } catch (err) {
    wrapErr(key, err);
  }
}

export function mustInt(key: string, opts: mustv.MustIntOptions = {}): number {
  try {
    return mustv.mustInt(get(key), opts);
  } catch (err) {
    wrapErr(key, err);
  }
}

export function mustBool(key: string, opts: mustv.MustBoolOptions = {}): boolean {
  try {
    return mustv.mustBool(get(key), opts);
  } catch (err) {
    wrapErr(key, err);
  }
}

export function mustMatch(key: string, re: RegExp, msg?: string): string {
  try {
    return mustv.mustMatch(get(key), re, msg);
  } catch (err) {
    wrapErr(key, err);
  }
}

export function mustOneOf<T extends readonly string[]>(
  key: string,
  allowed: T,
  fallback?: T[number],
): T[number] {
  try {
    return mustv.mustOneOf(get(key), allowed, fallback);
  } catch (err) {
    wrapErr(key, err);
  }
}
