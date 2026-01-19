// @xpute/core/net/kit/drop.ts

import type { u32 } from "@xpute/spec/lib/kit/word.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";

export type DropMeta = { why: string; errno?: Errno };
export type DropEvent = {
  ts: number;
  tag: string;
  why: string;
  errno?: Errno;
  count: u32;
};

export type DropHook = (tag: string, meta: DropMeta) => void;
export type DropSink = (ev: DropEvent) => void;

const DROP_RING_N = 64;

// log sampling (per (tag,why,errno) key)
const DROP_SAMPLE_WINDOW_MS = 1000;
type DropSample = { ts: number; n: number };
const _drop_sample = new Map<string, DropSample>();

const _drop_ts = new Float64Array(DROP_RING_N);
const _drop_tag = new Array<string>(DROP_RING_N);
const _drop_why = new Array<string>(DROP_RING_N);
const _drop_errno = new Int32Array(DROP_RING_N); // 0 => none (Errno.OK is never emitted as drop errno)
let _drop_i: u32 = 0;

const _drop_cnt = new Map<string, u32>();
const _drop_key_cnt = new Map<string, u32>();
let _drop_sink: DropSink | null = (ev) => {
  console.warn(!ev.errno ? `drop: ${ev.tag}: ${ev.why} (count=${ev.count})` : `drop: ${ev.tag}: ${ev.why} errno=${ev.errno} (count=${ev.count})`);
};

export const drop: DropHook = (tag: string, meta: DropMeta): void => {
  const c: u32 = (_drop_cnt.get(tag) ?? 0) + 1;
  _drop_cnt.set(tag, c);

  const why: string = meta.why;
  const errno: Errno = meta.errno ?? 0;
  const ts = Date.now();

  // fine-grained counts (tag,why,errno)
  const k = `${tag}\0${why}\0${errno}`;
  _drop_key_cnt.set(k, (_drop_key_cnt.get(k) ?? 0) + 1);

  // sample gate: suppress hot spam while keeping observability
  // policy: per 1s window, log first; then log on powers of two (2,4,8,...) within window
  let ok = true;
  if (_drop_sink) {
    const k = `${tag}\0${why}\0${errno}`;
    let st = _drop_sample.get(k);
    if (!st || ts - st.ts >= DROP_SAMPLE_WINDOW_MS) {
      st = { ts, n: 0 };
      _drop_sample.set(k, st);
    }
    st.n++;
    // keep first, then 2^k
    ok = st.n === 1 || (st.n & (st.n - 1)) === 0;
  }
  if (ok) _drop_sink?.(!errno ? { ts, tag, why, count: c } : { ts, tag, why, errno, count: c });

  const i = _drop_i & (DROP_RING_N - 1);
  _drop_i = _drop_i + 1;
  _drop_ts[i] = ts;
  _drop_tag[i] = tag;
  _drop_why[i] = why;
  _drop_errno[i] = errno;
};

export const drop_set_sink = (sink: DropSink | null): void => {
  _drop_sink = sink;
};

export const drop_get_sink = (): DropSink | null => _drop_sink;

export const drop_set_console = (on: boolean): void => {
  _drop_sink = on
    ? (ev) => {
      console.warn(!ev.errno ? `drop: ${ev.tag}: ${ev.why} (count=${ev.count})` : `drop: ${ev.tag}: ${ev.why} errno=${ev.errno} (count=${ev.count})`);
    }
    : null;
};

// -------- exports for debug --------

export type DropCounts = Array<[string, u32]>;
export type DropKeyCounts = Array<[string, u32]>;
export type DropRecent = Array<{ ts: number; tag: string; why: string; errno?: Errno }>;

export const drop_counts = (): DropCounts => Array.from(_drop_cnt.entries());

export const drop_counts_by_key = (): DropKeyCounts => Array.from(_drop_key_cnt.entries());

export const drop_recent = (): DropRecent => {
  const n = Math.min(_drop_i, DROP_RING_N);
  const out: DropRecent = new Array(n);
  for (let k = 0; k < n; k++) {
    const i = (_drop_i - n + k) & (DROP_RING_N - 1);
    const errno = _drop_errno[i] as Errno;
    out[k] = !errno
      ? { ts: _drop_ts[i], tag: _drop_tag[i] ?? "", why: _drop_why[i] ?? "" }
      : { ts: _drop_ts[i], tag: _drop_tag[i] ?? "", why: _drop_why[i] ?? "", errno };
  }
  return out;
};

export const drop_reset = (): void => {
  _drop_cnt.clear();
  _drop_key_cnt.clear();
  _drop_i = 0;
  for (let i = 0; i < DROP_RING_N; i++) {
    _drop_ts[i] = 0;
    _drop_tag[i] = "";
    _drop_why[i] = "";
    _drop_errno[i] = 0;
  }
};

// -------- debug helpers --------

export type DebugSnapshot = {
  counts: DropCounts;
  counts_by_key: DropKeyCounts;
  recent: DropRecent;
};

export const drop_snapshot = (): DebugSnapshot => ({
  counts: drop_counts(),
  counts_by_key: drop_counts_by_key(),
  recent: drop_recent(),
});

export const drop_format = (s: DebugSnapshot): string => {
  const parse_key = (k: string): { tag: string; why: string; errno: Errno } => {
    const a = k.split("\0");
    const tag = a[0] ?? "";
    const why = a[1] ?? "";
    let errno: Errno = 0;
    if (a.length >= 3) {
      const n = Number(a[2]);
      errno = Number.isFinite(n) ? (n as Errno) : 0;
    }
    return { tag, why, errno };
  };

  const lines: string[] = [];
  lines.push("drop_counts:");
  for (const [tag, n] of s.counts) lines.push(`  ${tag}: ${n}`);
  lines.push("drop_counts_by_key:");
  for (const [k, n] of s.counts_by_key) {
    const { tag, why, errno } = parse_key(k);
    lines.push(!errno ? `  ${tag}: ${why}: ${n}` : `  ${tag}: ${why}: errno=${errno}: ${n}`);
  }
  lines.push("drop_recent:");
  for (const e of s.recent) {
    lines.push(!e.errno ? `  ${e.ts} ${e.tag} ${e.why}` : `  ${e.ts} ${e.tag} ${e.why} errno=${e.errno}`);
  }
  return lines.join("\n");
};
