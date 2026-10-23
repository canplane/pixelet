// @pxl/web/feature/debug/profiler/view.ts

/**
 * The kernel's profiler read in place through views over module memory, so
 * reading allocates nothing from the heap it measures. The views are valid
 * only until the next turn, so each read copies into plain objects.
 */

import type { F64Array } from "@xpute/kit/abi/array.ts";
import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { td } from "@xpute/kit/codec/encoding.ts";
import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/config/deploy.ts";
import { Command, encode_profiler_read } from "@/kernel/abi/cmd.spec.ts";
import { OUTCOME_NAMES, STREAM_NAMES, TASK_PHASE_NAMES } from "@/kernel/io/demand.spec.ts";
import { kernel_call, kernel_memory, kernel_room } from "@/kernel/shell.ts";

import { PHASES } from "@xpute/host/sched/tick.spec.ts";
import { FRAME, READ, RINGS, SPREAD, STEP, SUMMARY, TASK, TASK_SUMMARY, WAKES, WORST } from "./profiler.spec.ts";
import { STEP_NAMES } from "@/kernel/turn/step.spec.ts";
import { FRAME_KINDS, GPU_SPAN_NAMES } from "@/kernel/render/gpu/span.spec.ts";
import { type Frame, frameAt } from "./lane.ts";
import { type LongFrame, longFramesSince } from "./long-frame.ts";

const WORD = 8;
const RECENT_TASKS = 20;

const words = (at: u32, n: u32) => new Float64Array(kernel_memory(), at, n);
const u32s = (at: u32, n: u32) => new Uint32Array(kernel_memory(), at, n);
const site = (file: u32, bytes: u32, line: u32) => file === 0 ? null : `${td.decode(new Uint8Array(kernel_memory(), file, bytes))}:${line}`;

function framesAt(at: u32, count: u32): Frame[] {
  const w = words(at, count * FRAME.WORDS);
  return Array.from({ length: count }, (_, i) => frameAt(w, i * FRAME.WORDS));
}

function tasksAt(at: u32, count: u32): TaskRecord[] {
  const w = words(at, count * TASK.WORDS);
  const subjects = new BigUint64Array(kernel_memory(), at, count * TASK.WORDS);
  return Array.from({ length: count }, (_, i) => {
    const o = i * TASK.WORDS;
    const phaseCount = w[o + TASK.phase_count];
    const phases: [string, f64][] = [];
    for (let k = 0; k < phaseCount; k++) {
      phases.push([TASK_PHASE_NAMES[w[o + TASK.phase_ids + k]] ?? "?", w[o + TASK.phase_values + k]]);
    }
    const start = w[o + TASK.start_ms];
    return {
      stream: STREAM_NAMES[w[o + TASK.stream]] ?? "?",
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
  /** Intervals whose turns ran past them. */
  long: f64;
  turns: f64;
  /** Frames a GPU read landed with, which the GPU's figures are over. */
  gpu_read: f64;
  interval: Spread;
  busy: Spread;
  tick: Spread;
  gpu: Spread;
  /** Each phase's and each span's p95: what is read of them. */
  phases: { name: string; p95: f64 }[];
  gpu_spans: { name: string; p95: f64 }[];
  /** Frames by the renderer's kind, and the turns' last ask by wake. */
  kinds: { name: string; n: f64 }[];
  wakes: { name: string; n: f64 }[];
  heap_used_delta_bytes: f64;
  last: Frame;
}

export interface TaskRecord {
  stream: string;
  subject: string;
  priority: f64;
  enqueue_ms: f64;
  start_ms: f64 | null;
  end_ms: f64;
  outcome: string;
  fail: u32;
  detail: u32;
  /** null for the host's failures. */
  at: string | null;
  phases: [string, f64][];
}

export interface TaskSummary {
  stream: string;
  n: u32;
  ok: u32;
  aborted: u32;
  error: u32;
  lat: { p50: f64; p95: f64 };
  reasons: { fail: u32; detail: u32; n: u32 }[];
  phase_mean: [string, f64][] | null;
}

export interface ProfilerReport {
  t: f64;
  build: { number: string; id: string; time: string };
  frames: FrameSummary | null;
  steps: { phase: string; name: string; ms: f64; count: f64; max_ms: f64 }[];
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
  return {
    n: s[a + F.n],
    seconds: s[a + F.seconds],
    moving: s[a + F.moving],
    missed: s[a + F.missed],
    overran: s[a + F.overran],
    long: s[a + F.long],
    turns: s[a + F.turns],
    gpu_read: s[a + F.gpu_read],
    interval: spreadAt(s, a + F.interval),
    busy: spreadAt(s, a + F.busy),
    tick: spreadAt(s, a + F.tick),
    gpu: spreadAt(s, a + F.gpu),
    phases: PHASES.map((name, k) => ({ name, p95: s[a + F.phases_p95 + k] })),
    gpu_spans: GPU_SPAN_NAMES.map((name, k) => ({ name, p95: s[a + F.gpu_spans_p95 + k] })),
    kinds: FRAME_KINDS.map((name, k) => ({ name, n: s[a + F.kinds + k] })),
    wakes: WAKES.map((name, k) => ({ name, n: s[a + F.wakes + k] })),
    heap_used_delta_bytes: s[a + F.heap_used_delta_bytes],
    last: frameAt(s, a + F.last),
  };
}

function taskSummaries(s: F64Array): TaskSummary[] {
  const out: TaskSummary[] = [];
  STREAM_NAMES.forEach((stream, c) => {
    const a = SUMMARY.tasks.AT + SUMMARY.tasks.slots + c * TASK_SUMMARY.WORDS;
    const n = s[a + TASK_SUMMARY.n];
    if (n === 0) return;
    const reasons = Array.from({ length: s[a + TASK_SUMMARY.reason_count] }, (_, i) => {
      const w = a + TASK_SUMMARY.reasons + 3 * i;
      return { fail: s[w], detail: s[w + 1], n: s[w + 2] };
    });
    const means = TASK_PHASE_NAMES.map((name, p) => [name, s[a + TASK_SUMMARY.phase_mean + p]] as [string, f64]).filter(([, v]) => !Number.isNaN(v));
    out.push({
      stream,
      n,
      ok: s[a + TASK_SUMMARY.ok],
      aborted: s[a + TASK_SUMMARY.aborted],
      error: s[a + TASK_SUMMARY.error],
      lat: { p50: s[a + TASK_SUMMARY.lat_p50], p95: s[a + TASK_SUMMARY.lat_p95] },
      reasons,
      phase_mean: s[a + TASK_SUMMARY.phase_n] > 0 ? means : null,
    });
  });
  return out;
}

function section(s: F64Array, sec: Record<string, u32>): Record<string, f64> {
  const out: Record<string, f64> = {};
  for (const [k, v] of Object.entries(sec)) if (k !== "AT" && k !== "WORDS") out[k] = s[sec.AT + v];
  return out;
}

/** The summary over the last `windowMs`, or the whole recording at null. */
function read(windowMs: f64 | null): F64Array {
  const r = u32s(kernel_call(Command.PROFILER_READ, encode_profiler_read(kernel_room, windowMs)), READ.WORDS);
  return words(r[READ.summary_at], r[READ.summary_words]);
}

interface Rings {
  frames: [u32, u32];
  tasks: [u32, u32];
  worst: [u32, u32];
}

/** The rings as they lie, each its address and count, with nothing reduced. */
function rings(): Rings {
  const r = u32s(kernel_call(Command.PROFILER_RINGS), RINGS.WORDS);
  return {
    frames: [r[RINGS.frames_at], r[RINGS.frames_count]],
    tasks: [r[RINGS.tasks_at], r[RINGS.tasks_count]],
    worst: [r[RINGS.worst_at], r[RINGS.worst_count]],
  };
}

/** Frames a second over a window: its intervals over its span, none under two frames. */
export const fps = (f: { n: f64; seconds: f64 }): f64 | null => (f.n >= 2 && f.seconds > 0 ? (f.n - 1) / f.seconds : null);

export interface Gauge {
  /** The frames and tasks the rings hold, as RINGS counts them. */
  rec: { recording: boolean; elapsed_ms: f64; tasks: u32; frames: u32; truncated: boolean };
  frames: FrameSummary | null;
  tasks: TaskSummary[];
  view: Record<string, f64>;
}

/** The worst frames the kernel kept, each with the one before it where there was one. */
function worstAt([at, count]: [u32, u32]): ProfilerReport["worst"] {
  const w = words(at, count * WORST.WORDS);
  return Array.from({ length: count }, (_, i) => {
    const o = i * WORST.WORDS;
    return { sample: frameAt(w, o + WORST.sample), prev: w[o + WORST.has_prev] === 0 ? null : frameAt(w, o + WORST.prev) };
  });
}

export function profilerGaugeRead(windowMs: f64): Gauge {
  const s = read(windowMs);
  const { frames, tasks } = rings();
  const r = section(s, SUMMARY.rec);
  return {
    rec: { recording: r.recording !== 0, elapsed_ms: r.elapsed_ms, tasks: tasks[1], frames: frames[1], truncated: r.truncated !== 0 },
    frames: frameSummary(s),
    tasks: taskSummaries(s),
    view: section(s, SUMMARY.view),
  };
}

export function profilerReportRead(): ProfilerReport {
  const s = read(null);
  const { tasks, worst } = rings();
  const stepsAt = SUMMARY.steps.AT;
  const steps = Array.from({ length: s[stepsAt + SUMMARY.steps.count] }, (_, i) => {
    const w = stepsAt + SUMMARY.steps.slots + i * STEP.WORDS;
    return {
      phase: PHASES[s[w + STEP.phase]] ?? "?",
      name: STEP_NAMES[s[w + STEP.name]] ?? "?",
      ms: s[w + STEP.ms],
      count: s[w + STEP.count],
      max_ms: s[w + STEP.max_ms],
    };
  });
  const recentFrom = Math.max(0, tasks[1] - RECENT_TASKS);
  return {
    t: performance.now(),
    build: { number: BUILD_NUMBER, id: BUILD_ID, time: BUILD_TIME },
    frames: frameSummary(s),
    steps,
    tasks: taskSummaries(s),
    recent: tasksAt(tasks[0] + recentFrom * TASK.WORDS * WORD, tasks[1] - recentFrom),
    worst: worstAt(worst),
    longs: longFramesSince(-Infinity),
  };
}

export function profilerRingsRead(): { frames: Frame[]; tasks: TaskRecord[]; worst: ProfilerReport["worst"] } {
  const { tasks, frames, worst } = rings();
  return { frames: framesAt(frames[0], frames[1]), tasks: tasksAt(tasks[0], tasks[1]), worst: worstAt(worst) };
}
