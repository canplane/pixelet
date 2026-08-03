// @pixelet/server/lib/env/must.ts

export type MustOptions = { allow_empty?: boolean };

export function must(v: string | undefined, opts: MustOptions = {}): string {
  if (v == null) throw new Error("missing");
  if (!opts.allow_empty && v.length === 0) throw new Error("empty");
  return v;
}

export type MustIntOptions = { fallback?: number; min?: number; max?: number };

export function must_int(raw: string | undefined, opts: MustIntOptions = {}): number {
  if (raw == null || raw === "") {
    if (opts.fallback !== undefined) return opts.fallback;
    throw new Error("missing");
  }
  const n = parseInt(raw, 10);
  if (!Number.isFinite(n)) throw new Error("invalid int");
  if (opts.min !== undefined && n < opts.min) throw new Error("range min");
  if (opts.max !== undefined && n > opts.max) throw new Error("range max");
  return n;
}

export type MustBoolOptions = { fallback?: boolean };

export function must_bool(raw: string | undefined, opts: MustBoolOptions = {}): boolean {
  if (raw == null || raw === "") {
    if (opts.fallback !== undefined) return opts.fallback;
    throw new Error("missing");
  }
  const v = raw.toLowerCase();
  if (v === "1" || v === "true" || v === "yes" || v === "y" || v === "on") return true;
  if (v === "0" || v === "false" || v === "no" || v === "n" || v === "off") return false;
  throw new Error("invalid bool");
}

export function must_match(v: string | undefined, re: RegExp, msg?: string): string {
  const s = must(v);
  if (!re.test(s)) throw new Error(msg ?? "no match");
  return s;
}

export function must_one_of<T extends readonly string[]>(
  raw: string | undefined,
  allowed: T,
  fallback?: T[number],
): T[number] {
  if (raw == null || raw === "") {
    if (fallback !== undefined) return fallback;
    throw new Error("missing");
  }
  for (const a of allowed) if (raw === a) return raw as T[number];
  throw new Error("not allowed");
}
