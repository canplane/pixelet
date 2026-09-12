// @pxl/web/modules/world/sched/ledger.ts

/**
 * Read-only instrument for the schedulers: records where time and work go so
 * it can be seen. Never read back into a scheduling decision — the
 * observation→execution edge is one-directional on purpose
 * (discussions/20260909-cooperative-scheduler-and-task-ledger.md).
 *
 * Two record types, one instrument:
 * - TaskRecord — one finished unit of deferred work (a fetch_scheduler.ts
 *   demand today; a CPU task later). The *shape* is meant to stay stable
 *   across the WASM/worker migration: `transport_ms` is 0 while a task body
 *   runs inline, and holds the postMessage + transfer cost once it runs in a
 *   worker.
 * - FrameSample — one rendered frame's gauges (pacing, GPU ask, memory,
 *   queue pressure). Only accrues while frames actually render
 *   (`frameloop="demand"`), which is correct: idle means nothing to sample.
 *
 * Still short of the full step-1 ledger in the discussion doc: the per-frame
 * breakdown is chunk_stage's three phases (recheck/commit/render), not a
 * per-task-category rollup (that needs the category-A scheduler to exist),
 * and scheduler_peak_stats / commit_step_stats still live in chunk_stage as
 * their own session-peak globals. LoAF, the dev overlay (ledger_overlay.tsx),
 * the retained-footprint counts and Chrome-trace export (trace.ts) are here.
 */

import type { Phases } from "./demand.ts";

export type Outcome = "ok" | "aborted" | "error";

export type TaskRecord = {
  key: string;
  category: string;
  priority: number;
  /** monotonic ms (performance.now) at enqueue / first run / settle. */
  enqueue_ms: number;
  start_ms: number | null;
  end_ms: number;
  /** 0 while inline; worker hop cost once a task body runs off-thread. */
  transport_ms: number;
  /** whatever split the demand's own run() reported (terrain: net vs decode).
   * Absent when it reported none — the run is then one opaque step. */
  phases?: Phases;
  outcome: Outcome;
};

/** One rendered frame. Lean on purpose — the live half of the factor set in
 * the discussion doc; latency percentiles, decode bandwidth and upload cost
 * are derived from TaskRecords or added later, not crammed in here. */
export type FrameSample = {
  /** monotonic ms (performance.now) at record time. */
  t: number;
  /** rAF interval in ms — pacing against the 16.7ms budget. */
  dt_ms: number;
  /** The whole tick (sched/tick.ts), every phase; does not include the
   * gl.render call that runs after it. */
  sched_ms: number;
  /** What the tick was allowed this frame — derived, so it moves. */
  budget_ms: number;
  /** sched_ms broken down by phase, in the tick's own order. */
  phases: Record<string, number>;
  /** chunk_stage's own sub-timings for this frame — the CPU breakdown behind
   * sched_ms, plus the wrapped gl.render call. `render_ms` lags by one frame
   * (r3f renders after every useFrame returns, so this holds the previous
   * frame's number), and reads 0 whenever the post-processing composer takes
   * over rendering and the wrapper never fires — which is most of the time,
   * so prefer `task_ms` below. */
  cpu: { recheck_ms: number; commit_ms: number; render_ms: number };
  /**
   * The whole rAF task: from the scheduler pass starting to the JS stack
   * emptying, so it covers every other useFrame subscriber *and* the render
   * the composer performs. Measured with a microtask because the composer
   * bypasses the gl.render wrapper.
   *
   * `dt_ms - task_ms` is then everything outside the main thread's own task —
   * vsync wait, compositor, GPU, GC between frames. That split is the whole
   * point: a 6s frame with a 13ms scheduler pass says nothing until you know
   * which side of this line the missing time is on.
   */
  task_ms: number;
  /** GPU wall time for this frame from EXT_disjoint_timer_query_webgl2; 0 when
   * unsupported or not yet resolved. Lags a few frames (gpu_timer.ts). */
  gpu_ms: number;
  // GPU ask — renderer.info, the fill-rate inputs
  draw_calls: number;
  triangles: number;
  geometries: number;
  textures: number;
  /** drawingBuffer width*height — the raw fill-rate primitive. Multiply by
   * fx_passes for effective per-frame shaded pixels. */
  px: number;
  /** active full-screen passes (post-fx stack + shadow map) — the multiplier
   * on px. Kept separate from px so the sample stays a truthful primitive. */
  fx_passes: number;
  /** chunk_stage's own per-frame LOD counts. Here rather than in a separate
   * session-peak object so the ring carries the *shape* of a spike, not only
   * how high it got. */
  lod: { chunks: number; in_frustum: number; pending: number; settling: number; blend_active: number };
  /** chunks that React mounted / unmounted since the previous sample —
   * the un-budgeted work that lands outside sched_ms. */
  mounts: { added: number; removed: number; rendered: number };
  /** used JS heap in MiB — Chrome only, 0 where unavailable. Its delta
   * across frames is allocation churn. */
  heap_mb: number;
  // retained-payload footprint — the "crash factor" (terrain/config.ts:
  // r=3 ≈ 197MB decoded crashed an iPhone 13 mini). Counts, not bytes: the
  // overlay multiplies tile_pages by a documented per-page estimate, same
  // as px×fx_passes. The gap to heap_mb is geometry/texture/JS overhead.
  tile_pages: number;
  poi_count: number;
  /** chunk_mesher's block pool: MiB currently borrowed, MiB sitting idle in
   * its free lists, and the share of acquires served from one. Reads against
   * heap_mb: `live` tracking the visible chunk count with a high hit rate is
   * reuse working; a low hit rate is the pool being defeated (class thrash,
   * or the idle ceiling), and then heap churn is back to allocation rate. */
  pool: { live_mb: number; idle_mb: number; hit_pct: number };
  /** chunk work started this frame: `scans` is fresh column_runs (one
   * MeshRun[][] per chunk — 4096 arrays plus an object per run), `bakes` is
   * pooled mesh blocks, `runs` the total run count those scans produced.
   * Prices a committing frame's allocation directly instead of inferring it
   * from Δheap: scans × ~1MB against the measured Δheap says whether the
   * remaining churn is column_runs or something else entirely. */
  work: { scans: number; bakes: number; runs: number };
  // network pressure — fetch_scheduler queued + in-flight, per category
  q_terrain: number;
  q_poi: number;
};

/** One long animation frame (LoAF, >50ms). Carries LoAF's own attribution so
 * a stutter pins to script (CPU) vs render/style-layout without guessing —
 * the CPU-vs-GPU split rule 4 in the discussion doc needs. */
export type LongFrameRecord = {
  /** which observer produced it — LoAF is richer but is unsupported outside
   * Chromium-with-the-flag, so `longtask` is the fallback that actually fires
   * in the browsers this gets run in. */
  source: "loaf" | "longtask";
  t: number;
  duration_ms: number;
  /** time past 50ms the frame blocked the event loop. */
  blocking_ms: number;
  /** summed <script> entry durations — the CPU half. */
  script_ms: number;
  /** startTime + duration - renderStart — the render/style/layout half; 0 if
   * the browser didn't report renderStart. */
  render_ms: number;
};

/**
 * Recording is **exhaustive for the session** — every task and every rendered
 * frame, from load until the tab goes away. It was a rolling window (240
 * frames, ~4s) and that was simply wrong: the one thing this instrument
 * exists to capture is a stall, and a stall had already been overwritten by
 * the time anyone pulled a report or a trace. An export taken seconds after a
 * two-second freeze showed an idle scene, which reads as "it didn't happen".
 *
 * The cost that was being optimised away is not real: a FrameSample is about
 * 200 bytes, so a ten-minute session at 60fps is ~7MB. The caps below are
 * pure safety valves against an unattended tab left running for hours, not a
 * retention policy — hitting one sets `truncated`, so an export can say so
 * rather than quietly lying by omission the way the ring did.
 */
const TASK_LIMIT = 200_000;
const FRAME_LIMIT = 500_000; // ~2.3 hours at 60fps
const LONG_FRAME_LIMIT = 50_000;

const task_ring: TaskRecord[] = [];
const frame_ring: FrameSample[] = [];
const long_frame_ring: LongFrameRecord[] = [];

let truncated = false;

/** True once any limit was hit and recording stopped being exhaustive. */
export function ledger_truncated(): boolean {
  return truncated;
}

/**
 * Live consumer for records as they land — log.ts's console stream today.
 * One sink, not a list: the instrument has a single output mode at a time,
 * and a subscriber list would invite the ledger drifting into a general
 * event bus, which is not what it is.
 */
export type LedgerSink = {
  task?: (rec: TaskRecord) => void;
  frame?: (s: FrameSample) => void;
  long_frame?: (l: LongFrameRecord) => void;
};

let sink: LedgerSink | null = null;

export function ledger_set_sink(s: LedgerSink | null): void {
  sink = s;
}

/**
 * Explicit recording window.
 *
 * Recording begins at load, so a session always covers "from the start", but
 * it is a *bounded* window with a stop and a restart rather than an ambient
 * stream. Stopping freezes the data so an export is stable while it is being
 * read; restarting clears and begins a fresh window. Without a boundary there
 * was no way to say what an export actually covered — which is how a capture
 * that did contain a zoom-out could be read as if it hadn't.
 */
let recording = true;
let started_at = performance.now();
let stopped_at: number | null = null;

export type RecordingState = {
  recording: boolean;
  /** ms of wall time the window spans; live while recording. */
  elapsed_ms: number;
  tasks: number;
  frames: number;
  long_frames: number;
  truncated: boolean;
};

export function ledger_recording_state(): RecordingState {
  return {
    recording,
    elapsed_ms: (stopped_at ?? performance.now()) - started_at,
    tasks: task_ring.length,
    frames: frame_ring.length,
    long_frames: long_frame_ring.length,
    truncated,
  };
}

/** Clears everything and opens a fresh window. */
export function ledger_recording_start(): void {
  task_ring.length = 0;
  frame_ring.length = 0;
  long_frame_ring.length = 0;
  worst_frames.length = 0;
  truncated = false;
  started_at = performance.now();
  stopped_at = null;
  recording = true;
}

/** Freezes the window; exports taken afterwards see exactly this data. */
export function ledger_recording_stop(): void {
  if (!recording) return;
  recording = false;
  stopped_at = performance.now();
}

export function ledger_record(rec: TaskRecord): void {
  if (!recording) return;
  if (task_ring.length >= TASK_LIMIT) truncated = true;
  else task_ring.push(rec);
  sink?.task?.(rec);
}

/**
 * Session-wide worst-seen values, kept alongside the ring rather than by each
 * work loop separately.
 *
 * The ring is ~4s deep, which is the right window for reading the *shape* of
 * a stutter but not for finding one you noticed thirty seconds ago — the
 * reason chunk_stage grew its own session-peak object in the first place
 * ("a manual read during a live stutter usually lands a frame or two after
 * the storm already passed"). Both views are wanted; keeping the peak here
 * means one place does the bookkeeping and the work loops just report frames.
 */
export type FramePeak = {
  dt_ms: number;
  sched_ms: number;
  recheck_ms: number;
  commit_ms: number;
  render_ms: number;
  draw_calls: number;
  triangles: number;
  heap_mb: number;
  q_terrain: number;
  q_poi: number;
  chunks: number;
  in_frustum: number;
  pending: number;
  settling: number;
  blend_active: number;
};

const frame_peak: FramePeak = {
  dt_ms: 0,
  sched_ms: 0,
  recheck_ms: 0,
  commit_ms: 0,
  render_ms: 0,
  draw_calls: 0,
  triangles: 0,
  heap_mb: 0,
  q_terrain: 0,
  q_poi: 0,
  chunks: 0,
  in_frustum: 0,
  pending: 0,
  settling: 0,
  blend_active: 0,
};

/** Copy of the session's worst-seen frame values. */
export function ledger_frame_peak(): FramePeak {
  return { ...frame_peak };
}

/**
 * The session's worst frames by dt, kept whole rather than reduced to a peak.
 *
 * The ring above is ~4s deep, which turned out to be too short to investigate
 * with: a zoom-out stall would be gone from it by the time a report was
 * pulled, leaving only a peak number and no way to ask *what else* that frame
 * was doing. Each entry keeps the frame and the one before it, because
 * geometry/texture/heap only mean anything as deltas.
 */
export type WorstFrame = { sample: FrameSample; prev: FrameSample | null };

const WORST_CAP = 12;
const worst_frames: WorstFrame[] = [];

/** Session worst frames, slowest first. */
export function ledger_worst_frames(): readonly WorstFrame[] {
  return worst_frames.slice();
}

export function ledger_record_frame(s: FrameSample): void {
  if (!recording) return;
  const prev = frame_ring.length > 0 ? frame_ring[frame_ring.length - 1] : null;
  if (worst_frames.length < WORST_CAP || s.dt_ms > worst_frames[worst_frames.length - 1].sample.dt_ms) {
    worst_frames.push({ sample: s, prev });
    worst_frames.sort((a, b) => b.sample.dt_ms - a.sample.dt_ms);
    if (worst_frames.length > WORST_CAP) worst_frames.length = WORST_CAP;
  }

  if (frame_ring.length >= FRAME_LIMIT) truncated = true;
  else frame_ring.push(s);

  const p = frame_peak;
  if (s.dt_ms > p.dt_ms) p.dt_ms = s.dt_ms;
  if (s.sched_ms > p.sched_ms) p.sched_ms = s.sched_ms;
  if (s.cpu.recheck_ms > p.recheck_ms) p.recheck_ms = s.cpu.recheck_ms;
  if (s.cpu.commit_ms > p.commit_ms) p.commit_ms = s.cpu.commit_ms;
  if (s.cpu.render_ms > p.render_ms) p.render_ms = s.cpu.render_ms;
  if (s.draw_calls > p.draw_calls) p.draw_calls = s.draw_calls;
  if (s.triangles > p.triangles) p.triangles = s.triangles;
  if (s.heap_mb > p.heap_mb) p.heap_mb = s.heap_mb;
  if (s.q_terrain > p.q_terrain) p.q_terrain = s.q_terrain;
  if (s.q_poi > p.q_poi) p.q_poi = s.q_poi;
  if (s.lod.chunks > p.chunks) p.chunks = s.lod.chunks;
  if (s.lod.in_frustum > p.in_frustum) p.in_frustum = s.lod.in_frustum;
  if (s.lod.pending > p.pending) p.pending = s.lod.pending;
  if (s.lod.settling > p.settling) p.settling = s.lod.settling;
  if (s.lod.blend_active > p.blend_active) p.blend_active = s.lod.blend_active;

  sink?.frame?.(s);
}

/** Copy of the task buffer, newest last — for a future dev overlay / trace dump. */
export function ledger_snapshot(): readonly TaskRecord[] {
  return task_ring.slice();
}

/** The frame sample just recorded, so a late-arriving measurement (task_ms,
 * which is only known once the rAF task's stack empties) can be written into
 * the frame it belongs to instead of being attributed to the next one. */
export function ledger_last_frame(): FrameSample | null {
  return frame_ring.length > 0 ? frame_ring[frame_ring.length - 1] : null;
}

/** Copy of the frame buffer, newest last. */
export function ledger_frames_snapshot(): readonly FrameSample[] {
  return frame_ring.slice();
}

/** Copy of the long-frame buffer, newest last. */
export function ledger_long_frames_snapshot(): readonly LongFrameRecord[] {
  return long_frame_ring.slice();
}

type LoafEntry = PerformanceEntry & {
  blockingDuration: number;
  renderStart: number;
  scripts: { duration: number }[];
};

let observers_installed = false;

/**
 * Wires the LoAF `PerformanceObserver` into the long-frame ring. Idempotent;
 * a no-op where the browser lacks `long-animation-frames` (Safari / iOS
 * WKWebView today — which is exactly why the shipped monitor cannot lean on
 * this and it stays a dev instrument). Call once from the dev overlay.
 */
export function ledger_install_observers(): void {
  if (observers_installed) return;
  if (typeof PerformanceObserver === "undefined") return;
  if (!PerformanceObserver.supportedEntryTypes?.includes("long-animation-frames")) return;
  observers_installed = true;

  const obs = new PerformanceObserver((list) => {
    for (const raw of list.getEntries()) {
      const e = raw as LoafEntry;
      const script_ms = e.scripts?.reduce((sum, s) => sum + s.duration, 0) ?? 0;
      const render_ms = e.renderStart > 0 ? Math.max(0, e.startTime + e.duration - e.renderStart) : 0;
      const rec: LongFrameRecord = {
        source: "loaf",
        t: e.startTime,
        duration_ms: e.duration,
        blocking_ms: e.blockingDuration ?? 0,
        script_ms,
        render_ms,
      };
      if (!recording) continue;
      if (long_frame_ring.length >= LONG_FRAME_LIMIT) truncated = true;
      else long_frame_ring.push(rec);
      sink?.long_frame?.(rec);
    }
  });
  obs.observe({ type: "long-animation-frames", buffered: true } as PerformanceObserverInit);
}

let longtask_installed = false;

/**
 * `longtask` is the fallback for everywhere LoAF is missing — it reports far
 * less (a duration and an attribution container, no script/render split) but
 * it actually fires, and a >50ms task is exactly the event worth catching.
 * Installed alongside LoAF rather than instead of it; both feed one ring and
 * each record says which observer it came from.
 */
export function ledger_install_longtask_observer(): void {
  if (longtask_installed) return;
  if (typeof PerformanceObserver === "undefined") return;
  if (!PerformanceObserver.supportedEntryTypes?.includes("longtask")) return;
  longtask_installed = true;

  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) {
      if (!recording) continue;
      if (long_frame_ring.length >= LONG_FRAME_LIMIT) {
        truncated = true;
        continue;
      }
      const rec: LongFrameRecord = {
        source: "longtask",
        t: e.startTime,
        duration_ms: e.duration,
        blocking_ms: Math.max(0, e.duration - 50),
        script_ms: 0,
        render_ms: 0,
      };
      long_frame_ring.push(rec);
      sink?.long_frame?.(rec);
    }
  }).observe({ type: "longtask", buffered: true } as PerformanceObserverInit);
}
