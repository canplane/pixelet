// @pxl/web/dev/ledger/view.ts

/**
 * The kernel's ledger, read where it lies. `ledger_read` hands over the
 * address of a few words — the summary block over a window, and the frame,
 * task and worst-frame rings — and `ledger_log` the rows the live stream
 * kept; everything is laid out as ledger.json says and read here through
 * views over the module's memory, into the shapes report.ts formats.
 *
 * Nothing crosses the ring and nothing is built in the kernel for a read, so
 * looking at the instrument allocates nothing from the heap it measures. The
 * views are good until the kernel's next turn, which is why each read here
 * copies what it keeps into plain objects before returning.
 */

import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/config/deploy.ts";
import { CORE_KERNEL } from "@/kernel/core_mem.ts";

import type { Frame, FrameSummary, LedgerReport, LongFrame, Spread, TaskRecord, TaskSummary } from "./report.ts";
import { CATEGORY_NAMES, CENSUS_TAGS, FRAME, LOG, OUTCOME_NAMES, PHASE_NAMES, READ, SPREAD, STEP, STEP_NAMES, SUMMARY, TASK, TASK_PHASE_NAMES, TASK_SUMMARY, WORST } from "./spec.ts";
import { long_frames_since } from "./long_frame.ts";

const WORD = 8;
/** The tail of the task ring the report carries whole, newest last. */
const RECENT_TASKS = 20;

const words = (at: number, n: number) => new Float64Array(CORE_KERNEL.memory.buffer, at, n);
const u32s = (at: number, n: number) => new Uint32Array(CORE_KERNEL.memory.buffer, at, n);

function frame_at(w: Float64Array, o: number): Frame {
  const at = (k: number) => w[o + k];
  const lod: Record<string, number> = {
    chunks: at(FRAME.lod_chunks),
    in_frustum: at(FRAME.lod_in_frustum),
    pending: at(FRAME.lod_pending),
    settling: at(FRAME.lod_settling),
    blend_active: at(FRAME.lod_blend_active),
    offscreen_pending: at(FRAME.lod_offscreen_pending),
    oldest_release_ms: at(FRAME.lod_oldest_release_ms),
    oldest_visible_ms: at(FRAME.lod_oldest_visible_ms),
    pressure: at(FRAME.lod_pressure),
  };
  return {
    t: at(FRAME.t),
    dt_ms: at(FRAME.dt_ms),
    frame_ms: at(FRAME.frame_ms),
    settled: at(FRAME.settled) !== 0,
    sched_ms: at(FRAME.sched_ms),
    budget_ms: at(FRAME.budget_ms),
    task_ms: at(FRAME.task_ms),
    gpu_ms: at(FRAME.gpu_ms),
    tick_ms: at(FRAME.tick_ms),
    phases: PHASE_NAMES.map((_, k) => at(FRAME.phases + k)),
    draw_calls: at(FRAME.draw_calls),
    triangles: at(FRAME.triangles),
    buffers: at(FRAME.buffers),
    buffer_mb: at(FRAME.buffer_mb),
    px: at(FRAME.px),
    tile_pages: at(FRAME.tile_pages),
    poi_count: at(FRAME.poi_count),
    textures_mb: { depth: at(FRAME.tex_depth), msaa: at(FRAME.tex_msaa), shadow: at(FRAME.tex_shadow), layers: at(FRAME.tex_layers) },
    census: CENSUS_TAGS.map((_, k) => [at(FRAME.census + 2 * k), at(FRAME.census + 2 * k + 1)] as [number, number]),
    heap: {
      used_mb: at(FRAME.heap_used_mb),
      live_mb: at(FRAME.heap_live_mb),
      range_mb: at(FRAME.heap_range_mb),
      refusals: at(FRAME.heap_refusals),
      largest_free_mb: at(FRAME.heap_largest_free_mb),
    },
    rings: { cq_peak: at(FRAME.ring_cq_peak), spill_peak: at(FRAME.ring_spill_peak) },
    mounts: { added: at(FRAME.mounts_added), removed: at(FRAME.mounts_removed), layouts: at(FRAME.mounts_layouts) },
    lod,
    q_terrain: at(FRAME.q_terrain),
    q_poi: at(FRAME.q_poi),
  };
}

function frames_at(at: number, count: number): Frame[] {
  const w = words(at, count * FRAME.WORDS);
  return Array.from({ length: count }, (_, i) => frame_at(w, i * FRAME.WORDS));
}

function tasks_at(at: number, count: number): TaskRecord[] {
  const w = words(at, count * TASK.WORDS);
  const subjects = new BigUint64Array(CORE_KERNEL.memory.buffer, at, count * TASK.WORDS);
  return Array.from({ length: count }, (_, i) => {
    const o = i * TASK.WORDS;
    const phase_count = w[o + TASK.phase_count];
    const phases: [string, number][] = [];
    for (let k = 0; k < phase_count; k++) {
      phases.push([TASK_PHASE_NAMES[w[o + TASK.phase_ids + k]] ?? "?", w[o + TASK.phase_values + k]]);
    }
    const start = w[o + TASK.start_ms];
    return {
      category: CATEGORY_NAMES[w[o + TASK.category]] ?? "?",
      subject: subjects[o + TASK.subject].toString(),
      priority: w[o + TASK.priority],
      enqueue_ms: w[o + TASK.enqueue_ms],
      start_ms: Number.isNaN(start) ? null : start,
      end_ms: w[o + TASK.end_ms],
      transport_ms: w[o + TASK.transport_ms],
      outcome: OUTCOME_NAMES[w[o + TASK.outcome]] ?? "?",
      fail: w[o + TASK.fail],
      status: w[o + TASK.status],
      phases,
    };
  });
}

const spread_at = (s: Float64Array, at: number): Spread => ({ p50: s[at + SPREAD.p50], p95: s[at + SPREAD.p95], max: s[at + SPREAD.max] });

function frame_summary(s: Float64Array): FrameSummary | null {
  const F = SUMMARY.frames;
  const a = F.AT;
  if (s[a + F.present] === 0) return null;
  const nan_null = (v: number) => (Number.isNaN(v) ? null : v);
  return {
    n: s[a + F.n],
    seconds: s[a + F.seconds],
    moving: s[a + F.moving],
    missed: s[a + F.missed],
    overran: s[a + F.overran],
    dt: spread_at(s, a + F.dt),
    task: spread_at(s, a + F.task),
    offthread: spread_at(s, a + F.offthread),
    gpu: spread_at(s, a + F.gpu),
    sched: spread_at(s, a + F.sched),
    tick: spread_at(s, a + F.tick),
    phases: PHASE_NAMES.map((name, k) => ({ name, spread: spread_at(s, a + F.phases + k * SPREAD.WORDS) })),
    work: [s[a + F.work_scans], s[a + F.work_bakes]],
    per_s: [s[a + F.per_s_scans], s[a + F.per_s_bakes]],
    backlog_s: s[a + F.backlog_s],
    backlog_per_s: [s[a + F.backlog_per_s_scans], s[a + F.backlog_per_s_bakes]],
    tick_per_bake: nan_null(s[a + F.tick_per_bake]),
    mounts: [s[a + F.mounts_added], s[a + F.mounts_removed]],
    used_delta_mb: s[a + F.used_delta_mb],
    census: CENSUS_TAGS.map((tag, k) => ({ tag, draws: s[a + F.census + 2 * k], tris: s[a + F.census + 2 * k + 1] }))
      .filter((r) => r.draws > 0)
      .sort((x, y) => y.tris - x.tris),
    last: frame_at(s, a + F.last),
  };
}

function task_summaries(s: Float64Array): TaskSummary[] {
  const out: TaskSummary[] = [];
  CATEGORY_NAMES.forEach((category, c) => {
    const a = SUMMARY.tasks.AT + SUMMARY.tasks.slots + c * TASK_SUMMARY.WORDS;
    const n = s[a + TASK_SUMMARY.n];
    if (n === 0) return;
    const reasons = Array.from({ length: s[a + TASK_SUMMARY.reason_count] }, (_, i) => {
      const w = a + TASK_SUMMARY.reasons + 3 * i;
      return { fail: s[w], status: s[w + 1], n: s[w + 2] };
    });
    const means = TASK_PHASE_NAMES.map((name, p) => [name, s[a + TASK_SUMMARY.phase_mean + p]] as [string, number]).filter(([, v]) => !Number.isNaN(v));
    out.push({
      category,
      n,
      ok: s[a + TASK_SUMMARY.ok],
      aborted: s[a + TASK_SUMMARY.aborted],
      error: s[a + TASK_SUMMARY.error],
      lat: spread_at(s, a + TASK_SUMMARY.lat),
      reasons,
      phase_mean: s[a + TASK_SUMMARY.phase_n] > 0 ? means : null,
    });
  });
  return out;
}

/** A section of the block as named numbers, in the order ledger.json lists
 * its fields. */
function section(s: Float64Array, sec: Record<string, number>): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [k, v] of Object.entries(sec)) if (k !== "AT" && k !== "WORDS") out[k] = s[sec.AT + v];
  return out;
}

type Read = { summary: Float64Array; frames: [number, number]; tasks: [number, number]; worst: [number, number] };

function read(window_ms: number): Read {
  const r = u32s(CORE_KERNEL.ledger_read(window_ms), READ.WORDS);
  return {
    summary: words(r[READ.summary_at], r[READ.summary_words]),
    frames: [r[READ.frames_at], r[READ.frames_count]],
    tasks: [r[READ.tasks_at], r[READ.tasks_count]],
    worst: [r[READ.worst_at], r[READ.worst_count]],
  };
}

export type Gauge = {
  rec: { recording: boolean; elapsed_ms: number; tasks: number; frames: number; truncated: boolean };
  frames: FrameSummary | null;
  tasks: TaskSummary[];
  faces: [number, number];
  atlas: { size: number; entries: number; shelves: number; used_px: number };
  poi: { ms: number; frames: number; max_frames: number; hits: number; candidates: number; kept: number; indexed: number; dropped: number };
  longs: LongFrame[];
};

function rec_of(s: Float64Array): Gauge["rec"] {
  const r = section(s, SUMMARY.rec);
  return { recording: r.recording !== 0, elapsed_ms: r.elapsed_ms, tasks: r.tasks, frames: r.frames, truncated: r.truncated !== 0 };
}

const atlas_of = (s: Float64Array) => section(s, SUMMARY.atlas) as Gauge["atlas"];

/** What the panel draws: the summary over the last `window_ms`. */
export function ledger_gauge_read(window_ms: number): Gauge {
  const { summary: s } = read(window_ms);
  const c = section(s, SUMMARY.counters);
  const poi = section(s, SUMMARY.poi);
  return {
    rec: rec_of(s),
    frames: frame_summary(s),
    tasks: task_summaries(s),
    faces: [c.faces_kept, c.faces_possible],
    atlas: atlas_of(s),
    poi: {
      ms: poi.place_ms,
      frames: poi.place_frames,
      max_frames: poi.place_max_frames,
      hits: poi.place_hits,
      candidates: poi.place_candidates,
      kept: poi.place_kept,
      indexed: poi.index_indexed,
      dropped: poi.index_dropped,
    },
    longs: long_frames_since(performance.now() - window_ms),
  };
}

/** The whole recording, as the report prints it. */
export function ledger_report_read(): LedgerReport {
  const { summary: s, tasks, worst } = read(Infinity);
  const poi = section(s, SUMMARY.poi);
  const peak = section(s, SUMMARY.peak);
  const steps_at = SUMMARY.steps.AT;
  const steps = Array.from({ length: s[steps_at + SUMMARY.steps.count] }, (_, i) => {
    const w = steps_at + SUMMARY.steps.slots + i * STEP.WORDS;
    return {
      phase: PHASE_NAMES[s[w + STEP.phase]] ?? "?",
      name: STEP_NAMES[s[w + STEP.name]] ?? "?",
      ms: s[w + STEP.ms],
      count: s[w + STEP.count],
      max_ms: s[w + STEP.max_ms],
    };
  });
  const recent_from = Math.max(0, tasks[1] - RECENT_TASKS);
  const worst_words = words(worst[0], worst[1] * WORST.WORDS);
  const task_summary = task_summaries(s);
  return {
    t: performance.now(),
    build: { number: BUILD_NUMBER, id: BUILD_ID, time: BUILD_TIME },
    frames: frame_summary(s),
    counters: section(s, SUMMARY.counters),
    atlas: atlas_of(s),
    retain: section(s, SUMMARY.retain),
    fetch: section(s, SUMMARY.fetch) as LedgerReport["fetch"],
    poi: {
      badge_count: poi.badge_count,
      dropped: poi.dropped,
      place: Object.fromEntries(Object.entries(poi).filter(([k]) => k.startsWith("place_")).map(([k, v]) => [k.slice(6), v])),
      index: Object.fromEntries(Object.entries(poi).filter(([k]) => k.startsWith("index_")).map(([k, v]) => [k.slice(6), v])),
    },
    steps,
    peak: peak.present === 0 ? null : peak,
    task_n: task_summary.reduce((n, t) => n + t.n, 0),
    tasks: task_summary,
    recent: tasks_at(tasks[0] + recent_from * TASK.WORDS * WORD, tasks[1] - recent_from),
    worst: Array.from({ length: worst[1] }, (_, i) => {
      const o = i * WORST.WORDS;
      return { sample: frame_at(worst_words, o + WORST.sample), prev: worst_words[o + WORST.has_prev] === 0 ? null : frame_at(worst_words, o + WORST.prev) };
    }),
    longs: long_frames_since(-Infinity),
  };
}

/** The whole recording's rings, for the trace. */
export function ledger_rings_read(): { frames: Frame[]; tasks: TaskRecord[]; worst: LedgerReport["worst"] } {
  const { tasks, frames, worst } = read(0);
  const worst_words = words(worst[0], worst[1] * WORST.WORDS);
  return {
    frames: frames_at(frames[0], frames[1]),
    tasks: tasks_at(tasks[0], tasks[1]),
    worst: Array.from({ length: worst[1] }, (_, i) => {
      const o = i * WORST.WORDS;
      return { sample: frame_at(worst_words, o + WORST.sample), prev: worst_words[o + WORST.has_prev] === 0 ? null : frame_at(worst_words, o + WORST.prev) };
    }),
  };
}

/** What the live stream kept since the last take, cleared by taking. */
export function ledger_log_take(): { tasks: TaskRecord[]; frames: Frame[] } {
  const l = u32s(CORE_KERNEL.ledger_log(), LOG.WORDS);
  const out = { tasks: tasks_at(l[LOG.tasks_at], l[LOG.tasks_count]), frames: frames_at(l[LOG.frames_at], l[LOG.frames_count]) };
  if (out.tasks.length + out.frames.length > 0) CORE_KERNEL.ledger_log_clear();
  return out;
}
