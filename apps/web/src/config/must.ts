// @pxl/web/config/must.ts

export interface MustOptions {
  allowEmpty?: boolean;
}

export function must(v: string | undefined, opts: MustOptions = {}): string {
  if (v == null) throw new Error("missing");
  if (!opts.allowEmpty && v.length === 0) throw new Error("empty");
  return v;
}

export interface MustIntOptions {
  fallback?: number;
  min?: number;
  max?: number;
}

export function mustInt(raw: string | undefined, opts: MustIntOptions = {}): number {
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

export interface MustBoolOptions {
  fallback?: boolean;
}

export function mustBool(raw: string | undefined, opts: MustBoolOptions = {}): boolean {
  if (raw == null || raw === "") {
    if (opts.fallback !== undefined) return opts.fallback;
    throw new Error("missing");
  }
  const v = raw.toLowerCase();
  if (v === "1" || v === "true" || v === "yes" || v === "y" || v === "on") return true;
  if (v === "0" || v === "false" || v === "no" || v === "n" || v === "off") return false;
  throw new Error("invalid bool");
}

export function mustMatch(v: string | undefined, re: RegExp, msg?: string): string {
  const s = must(v);
  if (!re.test(s)) throw new Error(msg ?? "no match");
  return s;
}

export function mustOneOf<T extends readonly string[]>(
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
