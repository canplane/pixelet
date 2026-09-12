// @pxl/web/module/world/sched/summary.ts

/**
 * The ledger window, reduced once.
 *
 * The overlay (ledger_overlay.tsx) and the report (log.ts) each used to
 * reduce the rings themselves, and they drifted: the report named failure
 * reasons the panel did not, and both counted missed frames against a 60 Hz
 * constant the display did not have. Both now format what this returns, so a
 * number on the panel and the same number in a pasted report agree.
 */

import type { FrameSample, TaskRecord } from "./ledger.ts";
import { HARD_SLACK_MS } from "./tick.ts";

/** Percentile over an already-sorted array; 0 for an empty one. */
export function pct(sorted: readonly number[], q: number): number {
  if (sorted.length === 0) return 0;
  return sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * q))];
}

/** One decoded terrain page, measured live (terrain/config.ts: ~4.03MB) —
 * the multiplier on tile_pages, same idea as px × fx_passes. */
export const TILE_DECODED_MB = 4;

/** A pending chunk waiting longer than this is being passed over rather than
 * queued. Ten seconds is above what a full zoom-out's queue takes to drain
 * (~7 s against one scan slot), so an ordinary backlog never trips it, and
 * below the sixteen-second wait that ageing (priority.ts) exists to prevent:
 * past it, ageing is not doing its job. */
export const STARVED_MS = 10_000;

/** A gap this many display intervals long skipped a vsync. */
const MISSED_INTERVALS = 1.5;

/**
 * A frame that dropped a vsync while the view was moving. A still frame is
 * excluded: the tick takes a larger budget then on purpose (tick.ts), and a
 * repeated image is not a stutter anyone sees.
 */
export function missed(f: FrameSample): boolean {
  return !f.settled && f.dt_ms > f.frame_ms * MISSED_INTERVALS;
}

/** The tick ran past what even the hard ceiling allows — a single step too
 * big for the forward-progress waiver to excuse. */
export function overran(f: FrameSample): boolean {
  return f.sched_ms > f.budget_ms + HARD_SLACK_MS;
}

export type Spread = { p50: number; p95: number; max: number };

function spread(values: number[]): Spread {
  values.sort((a, b) => a - b);
  return { p50: pct(values, 0.5), p95: pct(values, 0.95), max: values.length > 0 ? values[values.length - 1] : 0 };
}

export type FrameSummary = NonNullable<ReturnType<typeof summarize_frames>>;

export function summarize_frames(frames: readonly FrameSample[]) {
  if (frames.length === 0) return null;
  const first = frames[0];
  const last = frames[frames.length - 1];
  const of = (pick: (f: FrameSample) => number, from: readonly FrameSample[] = frames) => spread(from.map(pick));
  const sum = (pick: (f: FrameSample) => number) => frames.reduce((a, f) => a + pick(f), 0);
  const seconds = Math.max(1e-3, (last.t - first.t) / 1000);
  const moving = frames.filter((f) => !f.settled);
  const work = { scans: sum((f) => f.work.scans), bakes: sum((f) => f.work.bakes), runs: sum((f) => f.work.runs) };
  const backlog = frames.filter((f) => f.lod.pending > 0);
  const backlog_s = Math.max(1e-3, backlog.reduce((a, f) => a + f.dt_ms, 0) / 1000);
  const commit_ms = sum((f) => f.cpu.commit_ms);
  const pool_hits = last.pool.hits - first.pool.hits;
  const pool_acquires = pool_hits + last.pool.misses - first.pool.misses;

  return {
    n: frames.length,
    seconds,
    /** Frames the view was moving in — the only ones pacing is judged on. */
    moving: moving.length,
    missed: moving.filter(missed).length,
    overran: frames.filter(overran).length,
    dt: of((f) => f.dt_ms, moving),
    task: of((f) => f.task_ms),
    offthread: of((f) => Math.max(0, f.dt_ms - f.task_ms)),
    gpu: of((f) => f.gpu_ms),
    sched: of((f) => f.sched_ms),
    recheck: of((f) => f.cpu.recheck_ms),
    commit: of((f) => f.cpu.commit_ms),
    phases: Object.fromEntries(Object.keys(last.phases).map((k) => [k, of((f) => f.phases[k] ?? 0)])) as Record<string, Spread>,
    work,
    /** Throughput over the whole window. The last frame's counts only say
     * whether something ran, not how fast the backlog is draining. */
    per_s: { scans: work.scans / seconds, bakes: work.bakes / seconds, runs: work.runs / seconds },
    /** Throughput while there was a backlog to drain. `per_s` divides by the
     * whole window, idle stretches included, so two sessions of different
     * length could not be compared on it; this can. */
    backlog_s,
    backlog_per_s: {
      scans: backlog.reduce((a, f) => a + f.work.scans, 0) / backlog_s,
      bakes: backlog.reduce((a, f) => a + f.work.bakes, 0) / backlog_s,
    },
    /** What one bake cost the commit pass — comparable across sessions where
     * the absolute commit time is not, because it mixes in how much work the
     * window happened to contain. */
    commit_per_bake: work.bakes > 0 ? commit_ms / work.bakes : null,
    mounts: { added: sum((f) => f.mounts.added), removed: sum((f) => f.mounts.removed) },
    /** Share of the window's pool acquires served from a region already
     * reserved; null when the window acquired nothing. */
    pool_hit_pct: pool_acquires > 0 ? Math.round((pool_hits / pool_acquires) * 100) : null,
    /** Per-frame mean draws and triangles by tag, heaviest first. */
    census: census_mean(frames),
    heap_delta: last.heap_mb - first.heap_mb,
    last,
  };
}

/** [tag, draws, triangles], heaviest first. */
export type CensusRow = [string, number, number];

function census_mean(frames: readonly FrameSample[]): CensusRow[] {
  const sums = new Map<string, [number, number]>();
  for (const f of frames) {
    for (const [tag, [draws, tris]] of Object.entries(f.census)) {
      const s = sums.get(tag);
      if (s) {
        s[0] += draws;
        s[1] += tris;
      } else sums.set(tag, [draws, tris]);
    }
  }
  return [...sums.entries()].map(([tag, [d, t]]): CensusRow => [tag, d / frames.length, t / frames.length]).sort((a, b) => b[2] - a[2]);
}

/** One census as rows, heaviest first — the last frame's, say. */
export function census_rows(census: FrameSample["census"]): CensusRow[] {
  return Object.entries(census).map(([tag, [d, t]]): CensusRow => [tag, d, t]).sort((a, b) => b[2] - a[2]);
}

export function tris_text(t: number): string {
  return t >= 1e6 ? `${(t / 1e6).toFixed(2)}M` : `${Math.round(t / 1000)}k`;
}

export type TaskSummary = {
  cat: string;
  n: number;
  ok: number;
  aborted: number;
  error: number;
  /** Latency of the tasks that succeeded, enqueue to settle. */
  lat: Spread;
  /** Failures by reason, commonest first — fetch_scheduler.ts's error_reason. */
  reasons: [string, number][];
  /** Mean of each phase the demand's run() reported, when any did — the
   * number that says whether the category's static bound label is true. */
  phase_mean: Record<string, number> | null;
};

export function summarize_tasks(tasks: readonly TaskRecord[]): TaskSummary[] {
  type Group = { lat: number[]; ok: number; aborted: number; error: number; reasons: Map<string, number>; phase_sum: Map<string, number>; phase_n: number };
  const by = new Map<string, Group>();
  for (const rec of tasks) {
    let g = by.get(rec.category);
    if (!g) by.set(rec.category, g = { lat: [], ok: 0, aborted: 0, error: 0, reasons: new Map(), phase_sum: new Map(), phase_n: 0 });
    if (rec.outcome === "ok") {
      g.ok++;
      g.lat.push(rec.end_ms - rec.enqueue_ms);
    } else if (rec.outcome === "aborted") g.aborted++;
    else {
      g.error++;
      const reason = rec.error ?? "unrecorded";
      g.reasons.set(reason, (g.reasons.get(reason) ?? 0) + 1);
    }
    if (rec.phases) {
      g.phase_n++;
      for (const [k, v] of Object.entries(rec.phases)) g.phase_sum.set(k, (g.phase_sum.get(k) ?? 0) + v);
    }
  }
  return [...by.entries()].map(([cat, g]) => ({
    cat,
    n: g.ok + g.aborted + g.error,
    ok: g.ok,
    aborted: g.aborted,
    error: g.error,
    lat: spread(g.lat),
    reasons: [...g.reasons.entries()].sort((a, b) => b[1] - a[1]),
    phase_mean: g.phase_n > 0 ? Object.fromEntries([...g.phase_sum.entries()].map(([k, v]) => [k, v / g.phase_n])) : null,
  }));
}
