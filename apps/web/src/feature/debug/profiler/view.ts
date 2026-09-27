// @pxl/web/feature/debug/profiler/view.ts

/**
 * The kernel's profiler, read where it lies. PROFILER_READ answers with the
 * address of a few words — the summary block over a window, and the frame,
 * task and worst-frame rings — and PROFILER_TAKE with the rows the live stream
 * kept; everything is laid out as profiler.json says and read here through
 * views over the module's memory, into the shapes report.ts formats.
 *
 * Only the address crosses the ring and nothing is built in the kernel for a
 * read, so looking at the instrument allocates nothing from the heap it
 * measures. The views are good until the kernel's next turn, which is why
 * each read here copies what it keeps into plain objects before returning.
 */

import type { F64Array } from "@xpute/core/abi/array.ts";
import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/config/deploy.ts";
import { Command, encode_profiler_read } from "@/kernel/abi/cmd.spec.ts";
import { CORE, CORE_DOORBELL } from "@/kernel/mem.ts";

import {
  CATEGORY_NAMES,
  CENSUS_TAGS,
  FRAME,
  GPU_SPAN_NAMES,
  LOG,
  OUTCOME_NAMES,
  PHASE_NAMES,
  READ,
  SPREAD,
  STEP,
  STEP_NAMES,
  SUMMARY,
  TASK,
  TASK_PHASE_NAMES,
  TASK_SUMMARY,
  WORST,
} from "./profiler.spec.ts";
import { type Frame, frameAt } from "./lane.ts";
import { type LongFrame, longFramesSince } from "./long-frame.ts";

const WORD = 8;
/** The tail of the task ring the report carries whole, newest last. */
const RECENT_TASKS = 20;

const words = (at: u32, n: u32) => new Float64Array(CORE.buffer, at, n);
const u32s = (at: u32, n: u32) => new Uint32Array(CORE.buffer, at, n);
/** Where the kernel made a failure: a file name in its memory and a line. */
const site = (file: u32, bytes: u32, line: u32) => file === 0 ? null : `${new TextDecoder().decode(new Uint8Array(CORE.buffer, file, bytes))}:${line}`;

function framesAt(at: u32, count: u32): Frame[] {
  const w = words(at, count * FRAME.WORDS);
  return Array.from({ length: count }, (_, i) => frameAt(w, i * FRAME.WORDS));
}

function tasksAt(at: u32, count: u32): TaskRecord[] {
  const w = words(at, count * TASK.WORDS);
  const subjects = new BigUint64Array(CORE.buffer, at, count * TASK.WORDS);
  return Array.from({ length: count }, (_, i) => {
    const o = i * TASK.WORDS;
    const phaseCount = w[o + TASK.phase_count];
    const phases: [string, f64][] = [];
    for (let k = 0; k < phaseCount; k++) {
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
      outcome: OUTCOME_NAMES[w[o + TASK.outcome]] ?? "?",
      fail: w[o + TASK.fail],
      detail: w[o + TASK.detail],
      at: site(w[o + TASK.at_file], w[o + TASK.at_file_bytes], w[o + TASK.at_line]),
      phases,
    };
  });
}

// ---- what the profiler holds, as it is read ----

export interface Spread {
  p50: f64;
  p95: f64;
  max: f64;
}

export interface FrameSummary {
  n: u32;
  seconds: f64;
  moving: f64;
  missed: f64;
  overran: f64;
  dt: Spread;
  task: Spread;
  offthread: Spread;
  gpu: Spread;
  sched: Spread;
  chunks: Spread;
  phases: { name: string; spread: Spread }[];
  /** A span's time over the frames that ran it. */
  gpu_spans: { name: string; spread: Spread }[];
  work: [f64, f64];
  perS: [f64, f64];
  backlog_s: f64;
  backlogPerS: [f64, f64];
  chunks_per_bake: f64 | null;
  mounts: [f64, f64];
  used_delta_mb: f64;
  census: { tag: string; draws: f64; tris: f64 }[];
  /** The terrain's triangles a frame, by level. */
  level_tris: f64[];
  last: Frame;
}

export interface TaskRecord {
  category: string;
  /** A u64, as its decimal digits. */
  subject: string;
  priority: f64;
  enqueue_ms: f64;
  start_ms: f64 | null;
  end_ms: f64;
  outcome: string;
  fail: u32;
  detail: u32;
  /** `file:line`, where the kernel made the failure; null for the host's. */
  at: string | null;
  phases: [string, f64][];
}

export interface TaskSummary {
  category: string;
  n: u32;
  ok: u32;
  aborted: u32;
  error: u32;
  lat: Spread;
  reasons: { fail: u32; detail: u32; n: u32 }[];
  phase_mean: [string, f64][] | null;
}

export interface ProfilerReport {
  t: f64;
  build: { number: string; id: string; time: string };
  frames: FrameSummary | null;
  /** The summary's side sections, a field a number, as profiler.json names them. */
  sections: Record<"counters" | "atlas" | "retain" | "fetch", Record<string, f64>>;
  steps: { phase: string; name: string; ms: f64; count: f64; max_ms: f64 }[];
  peak: Record<string, f64> | null;
  taskN: u32;
  tasks: TaskSummary[];
  recent: TaskRecord[];
  worst: { sample: Frame; prev: Frame | null }[];
  longs: LongFrame[];
}

const spreadAt = (s: F64Array, at: u32): Spread => ({ p50: s[at + SPREAD.p50], p95: s[at + SPREAD.p95], max: s[at + SPREAD.max] });

function frameSummary(s: F64Array): FrameSummary | null {
  const F = SUMMARY.frames;
  const a = F.AT;
  if (s[a + F.present] === 0) return null;
  const nanNull = (v: f64) => (Number.isNaN(v) ? null : v);
  return {
    n: s[a + F.n],
    seconds: s[a + F.seconds],
    moving: s[a + F.moving],
    missed: s[a + F.missed],
    overran: s[a + F.overran],
    dt: spreadAt(s, a + F.dt),
    task: spreadAt(s, a + F.task),
    offthread: spreadAt(s, a + F.offthread),
    gpu: spreadAt(s, a + F.gpu),
    sched: spreadAt(s, a + F.sched),
    chunks: spreadAt(s, a + F.chunks),
    phases: PHASE_NAMES.map((name, k) => ({ name, spread: spreadAt(s, a + F.phases + k * SPREAD.WORDS) })),
    gpu_spans: GPU_SPAN_NAMES.map((name, k) => ({ name, spread: spreadAt(s, a + F.gpu_spans + k * SPREAD.WORDS) })),
    work: [s[a + F.work_scans], s[a + F.work_bakes]],
    perS: [s[a + F.per_s_scans], s[a + F.per_s_bakes]],
    backlog_s: s[a + F.backlog_s],
    backlogPerS: [s[a + F.backlog_per_s_scans], s[a + F.backlog_per_s_bakes]],
    chunks_per_bake: nanNull(s[a + F.chunks_per_bake]),
    mounts: [s[a + F.mounts_added], s[a + F.mounts_removed]],
    used_delta_mb: s[a + F.used_delta_mb],
    census: CENSUS_TAGS.map((tag, k) => ({ tag, draws: s[a + F.census + 2 * k], tris: s[a + F.census + 2 * k + 1] }))
      .filter((r) => r.draws > 0)
      .sort((x, y) => y.tris - x.tris),
    level_tris: Array.from({ length: F.last - F.level_tris }, (_, k) => s[a + F.level_tris + k]),
    last: frameAt(s, a + F.last),
  };
}

function taskSummaries(s: F64Array): TaskSummary[] {
  const out: TaskSummary[] = [];
  CATEGORY_NAMES.forEach((category, c) => {
    const a = SUMMARY.tasks.AT + SUMMARY.tasks.slots + c * TASK_SUMMARY.WORDS;
    const n = s[a + TASK_SUMMARY.n];
    if (n === 0) return;
    const reasons = Array.from({ length: s[a + TASK_SUMMARY.reason_count] }, (_, i) => {
      const w = a + TASK_SUMMARY.reasons + 3 * i;
      return { fail: s[w], detail: s[w + 1], n: s[w + 2] };
    });
    const means = TASK_PHASE_NAMES.map((name, p) => [name, s[a + TASK_SUMMARY.phase_mean + p]] as [string, f64]).filter(([, v]) => !Number.isNaN(v));
    out.push({
      category,
      n,
      ok: s[a + TASK_SUMMARY.ok],
      aborted: s[a + TASK_SUMMARY.aborted],
      error: s[a + TASK_SUMMARY.error],
      lat: spreadAt(s, a + TASK_SUMMARY.lat),
      reasons,
      phase_mean: s[a + TASK_SUMMARY.phase_n] > 0 ? means : null,
    });
  });
  return out;
}

/** A section of the block as named numbers, in the order profiler.json lists
 * its fields. */
function section(s: F64Array, sec: Record<string, u32>): Record<string, f64> {
  const out: Record<string, f64> = {};
  for (const [k, v] of Object.entries(sec)) if (k !== "AT" && k !== "WORDS") out[k] = s[sec.AT + v];
  return out;
}

interface Read {
  summary: F64Array;
  frames: [u32, u32];
  tasks: [u32, u32];
  worst: [u32, u32];
}

function read(windowMs: f64): Read {
  const r = u32s(CORE_DOORBELL.call(Command.PROFILER_READ, encode_profiler_read({ window_ms: windowMs }), (at) => at >>> 0), READ.WORDS);
  return {
    summary: words(r[READ.summary_at], r[READ.summary_words]),
    frames: [r[READ.frames_at], r[READ.frames_count]],
    tasks: [r[READ.tasks_at], r[READ.tasks_count]],
    worst: [r[READ.worst_at], r[READ.worst_count]],
  };
}

export interface Gauge {
  rec: { recording: boolean; elapsed_ms: f64; tasks: u32; frames: u32; truncated: boolean };
  frames: FrameSummary | null;
  tasks: TaskSummary[];
  counters: Record<string, f64>;
  atlas: { size: u32; entries: u32; shelves: u32; used_px: u32 };
  longs: LongFrame[];
}

function recOf(s: F64Array): Gauge["rec"] {
  const r = section(s, SUMMARY.rec);
  return { recording: r.recording !== 0, elapsed_ms: r.elapsed_ms, tasks: r.tasks, frames: r.frames, truncated: r.truncated !== 0 };
}

const atlasOf = (s: F64Array) => section(s, SUMMARY.atlas) as Gauge["atlas"];

/** What the panel draws: the summary over the last `windowMs`. */
export function profilerGaugeRead(windowMs: f64): Gauge {
  const { summary: s } = read(windowMs);
  return {
    rec: recOf(s),
    frames: frameSummary(s),
    tasks: taskSummaries(s),
    counters: section(s, SUMMARY.counters),
    atlas: atlasOf(s),
    longs: longFramesSince(performance.now() - windowMs),
  };
}

/** The whole recording, as the report prints it. */
export function profilerReportRead(): ProfilerReport {
  const { summary: s, tasks, worst } = read(Infinity);
  const peak = section(s, SUMMARY.peak);
  const stepsAt = SUMMARY.steps.AT;
  const steps = Array.from({ length: s[stepsAt + SUMMARY.steps.count] }, (_, i) => {
    const w = stepsAt + SUMMARY.steps.slots + i * STEP.WORDS;
    return {
      phase: PHASE_NAMES[s[w + STEP.phase]] ?? "?",
      name: STEP_NAMES[s[w + STEP.name]] ?? "?",
      ms: s[w + STEP.ms],
      count: s[w + STEP.count],
      max_ms: s[w + STEP.max_ms],
    };
  });
  const recentFrom = Math.max(0, tasks[1] - RECENT_TASKS);
  const worstWords = words(worst[0], worst[1] * WORST.WORDS);
  const taskSummary = taskSummaries(s);
  return {
    t: performance.now(),
    build: { number: BUILD_NUMBER, id: BUILD_ID, time: BUILD_TIME },
    frames: frameSummary(s),
    sections: {
      counters: section(s, SUMMARY.counters),
      atlas: section(s, SUMMARY.atlas),
      retain: section(s, SUMMARY.retain),
      fetch: section(s, SUMMARY.fetch),
    },
    steps,
    peak: peak.present === 0 ? null : Object.fromEntries(Object.entries(peak).filter(([k]) => k !== "present")),
    taskN: taskSummary.reduce((n, t) => n + t.n, 0),
    tasks: taskSummary,
    recent: tasksAt(tasks[0] + recentFrom * TASK.WORDS * WORD, tasks[1] - recentFrom),
    worst: Array.from({ length: worst[1] }, (_, i) => {
      const o = i * WORST.WORDS;
      return { sample: frameAt(worstWords, o + WORST.sample), prev: worstWords[o + WORST.has_prev] === 0 ? null : frameAt(worstWords, o + WORST.prev) };
    }),
    longs: longFramesSince(-Infinity),
  };
}

/** The whole recording's rings, for the trace. */
export function profilerRingsRead(): { frames: Frame[]; tasks: TaskRecord[]; worst: ProfilerReport["worst"] } {
  const { tasks, frames, worst } = read(0);
  const worstWords = words(worst[0], worst[1] * WORST.WORDS);
  return {
    frames: framesAt(frames[0], frames[1]),
    tasks: tasksAt(tasks[0], tasks[1]),
    worst: Array.from({ length: worst[1] }, (_, i) => {
      const o = i * WORST.WORDS;
      return { sample: frameAt(worstWords, o + WORST.sample), prev: worstWords[o + WORST.has_prev] === 0 ? null : frameAt(worstWords, o + WORST.prev) };
    }),
  };
}

/** What the live stream kept since the last take, cleared by taking. */
export function profilerLogTake(): { tasks: TaskRecord[]; frames: Frame[] } {
  const l = u32s(CORE_DOORBELL.call(Command.PROFILER_TAKE, null, (at) => at >>> 0), LOG.WORDS);
  return { tasks: tasksAt(l[LOG.tasks_at], l[LOG.tasks_count]), frames: framesAt(l[LOG.frames_at], l[LOG.frames_count]) };
}
