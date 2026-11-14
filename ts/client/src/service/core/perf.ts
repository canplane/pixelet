// @pxl/client/service/core/perf.ts

/**
 * The core's profiler, read from the words each read's reply names, which go
 * with the reply's frame. A read allocates nothing in the core's heap, which
 * the profiler measures. The layout of what is read is perf.json5's
 * (perf.spec.ts). What is made of those values, and how they are shown, is the
 * UI's work (ui/feature/debug/perf/).
 */

import type { F64Array, U8Array } from "@xpute/kit/abi/array.ts";
import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { td } from "@xpute/kit/codec/encoding.ts";
import { ComponentError } from "@xpute/link";
import { PHASES } from "@/abi/phase.spec.ts";
import {
  OUTCOME_NAMES,
  STREAM_NAMES,
  TASK_PHASE_NAMES,
} from "@/abi/demand.spec.ts";
import {
  FRAME,
  FRAME_GROUP,
  LIMIT,
  NOW,
  RING_NAMES,
  PERCENTILE,
  STAT,
  STEP_STAT,
  STREAM_STAT,
  TASK,
  WORST,
} from "@/abi/perf.spec.ts";
import { EFFECT_NAMES, STEP_NAMES } from "@/abi/step.spec.ts";
import { FRAME_KINDS, GPU_SPAN_NAMES } from "@/abi/span.spec.ts";
import * as perf from "@/abi/perf.ts";
import { ui } from "@/service/core/core.ts";

const WORD = 8;
/** How many bytes a task's `path` column holds (perf.json5 `TASK`). */
const PATH_BYTES = 3 * WORD;

export function start(): void {
  void perf.start(ui).catch((e: unknown) => {
    if (!(e instanceof ComponentError)) throw e;
    console.warn(`[perf] not started: ${e.payload}`);
  });
}

/** Freezes the recording window, so that what is exported holds still. */
export function stop(): void {
  void perf.stop(ui);
}

/** The words of a reply's bytes, which every host hands over 8-aligned. */
const words = (b: U8Array) =>
  new Float64Array(b.buffer, b.byteOffset, b.byteLength / WORD);
/** A failure's site, or none at line 0, a failure of the host's: its line,
 * and its file as the string table `strtab` (PERF_STRTAB) names it at `file`,
 * or unknown at offset 0, where the table had no room for it. */
const site = (strtab: U8Array, file: u32, bytes: u32, line: u32) =>
  line === 0
    ? null
    : `${file === 0 ? "?" : td.decode(strtab.subarray(file, file + bytes))}:${line}`;

/** The FRAME columns that are lists, one word for each name in the list. */
export const LIST_COLUMNS = ["phases", "effects", "gpu_spans"] as const;
export type ListColumn = (typeof LIST_COLUMNS)[number];
export type FrameColumn = Exclude<keyof typeof FRAME, "WORDS">;
type ScalarColumn = Exclude<FrameColumn, ListColumn>;

export type Frame = { [K in ScalarColumn]: f64 } & {
  phases: f64[];
  effects: f64[];
  gpu_spans: f64[];
};

/** Each FRAME column with its first word and its width in words. */
export const COLUMNS: readonly { name: FrameColumn; at: u32; width: u32 }[] =
  (() => {
    const at = (Object.entries(FRAME) as [FrameColumn | "WORDS", u32][]).filter(
      ([k]) => k !== "WORDS",
    ) as [FrameColumn, u32][];
    at.sort((a, b) => a[1] - b[1]);
    return at.map(([name, a], i) => ({
      name,
      at: a,
      width: (i + 1 < at.length ? at[i + 1][1] : FRAME.WORDS) - a,
    }));
  })();

const LISTS: ReadonlySet<string> = new Set(LIST_COLUMNS);
export const is_list = (column: FrameColumn): column is ListColumn =>
  LISTS.has(column);

export function frame_at(words: ArrayLike<f64>, o: u32): Frame {
  const f: Record<string, f64 | f64[]> = {};
  for (const { name, at, width } of COLUMNS) {
    const w = (k: u32) => words[o + at + k];
    if (is_list(name)) {
      f[name] = Array.from({ length: width }, (_, k) => w(k));
    } else f[name] = w(0);
  }
  return f as Frame;
}

export interface Percentile {
  p50: f64;
  p95: f64;
  max: f64;
}

export interface FrameSummary {
  n: u32;
  seconds: f64;
  moving: f64;
  /** The sum of the moving frames' intervals. */
  moving_ms: f64;
  overran: f64;
  /** How many frames a GPU read timed. The GPU's figures are taken over
   * these frames alone. */
  gpu_timed: f64;
  interval: Percentile;
  tick: Percentile;
  effect: Percentile;
  stage: Percentile;
  perf: Percentile;
  /** The GPU's busy time: the time its spans ran, with overlapping spans
   * counted once. */
  gpu_busy: Percentile;
  /** For each frame kind, the p50 and p95 of the GPU's busy time over the
   * frames of that kind that a read timed, and how many there were. */
  gpu_busy_kinds: { name: string; p50: f64; p95: f64; n: f64 }[];
  /** The p95 of each phase, each effect and each GPU span, which is all that
   * is read of them. A span's p95 is over the `n` frames that a read timed
   * and that ran the span. */
  phases: { name: string; p95: f64 }[];
  effects_p95: { name: string; p95: f64 }[];
  gpu_spans: { name: string; p95: f64; n: f64 }[];
  /** How many frames were of each kind the renderer draws. */
  kinds: { name: string; n: f64 }[];
  /** How many of the world's pixels the last frame a read timed drew. */
  gpu_px: f64;
  /** The ray counts of the window's frames, summed for each of FRAME's rays
   * columns. */
  ray_cnt: RayCount;
  /** How far the heap's high-water mark rose over the window: the last
   * frame's mark less the first frame's. */
  heap_used_delta_bytes: f64;
  first: Frame;
  last: Frame;
}

export interface TaskRecord {
  stream: string;
  path: string;
  priority: f64;
  enqueue_ms: f64;
  start_ms: f64 | null;
  end_ms: f64;
  outcome: string;
  fail: u32;
  detail: u32;
  /** Where in the core the task failed, as `file:line`, or null for a
   * failure of the host's. */
  at: string | null;
  phases: [string, f64][];
}

export interface TaskSummary {
  stream: string;
  n: u32;
  ok: u32;
  aborted: u32;
  err: u32;
  lat: { p50: f64; p95: f64 };
  reasons: { fail: u32; detail: u32; n: u32 }[];
  phase_mean: [string, f64][] | null;
}

export interface Step {
  phase: string;
  name: string;
  ms: f64;
  cnt: f64;
  max_ms: f64;
}

/** One of the worst frames the core kept, and what changed since the frame
 * before it. `delta` is null when there was no frame before it. */
export interface Worst {
  sample: Frame;
  delta: {
    upload_cnt: f64;
    upload_bytes: f64;
    heap_used_bytes: f64;
  } | null;
}

export type Section<S> = { [K in Exclude<keyof S, "AT" | "WORDS">]: f64 };
export type ViewSection = Section<typeof NOW.view>;
export type DrawSection = Section<typeof NOW.draw>;
/** The highest or lowest values the frames held since the recording started
 * (STAT.recording). */
export type RecordingSection = Section<typeof STAT.recording>;
export type RayCount = { [K in (typeof FRAME_GROUP.rays)[number]]: f64 };

/** What PERF_STATS sums over its window, section by section. `recording` and
 * `steps` are over the whole recording instead, from when it started. */
export interface Summary {
  recording_state: {
    recording: boolean;
    elapsed_ms: f64;
    truncated: boolean;
    frame_cnt: u32;
    task_cnt: u32;
    worst_cnt: u32;
  };
  frames: FrameSummary | null;
  tasks: TaskSummary[];
  steps: Step[];
  recording: RecordingSection;
  view: ViewSection;
}

const percentile_at = (s: F64Array, at: u32): Percentile => ({
  p50: s[at + PERCENTILE.p50],
  p95: s[at + PERCENTILE.p95],
  max: s[at + PERCENTILE.max],
});

function frame_summary(s: F64Array): FrameSummary | null {
  const F = STAT.frames;
  const a = F.AT;
  if (s[a + F.present] === 0) return null;
  const first = frame_at(s, a + F.first);
  const last = frame_at(s, a + F.last);
  return {
    n: s[a + F.cnt],
    seconds: s[a + F.seconds],
    moving: s[a + F.moving],
    moving_ms: s[a + F.moving_ms],
    overran: s[a + F.overran],
    gpu_timed: s[a + F.gpu_timed],
    interval: percentile_at(s, a + F.interval),
    tick: percentile_at(s, a + F.tick),
    effect: percentile_at(s, a + F.effect),
    stage: percentile_at(s, a + F.stage),
    perf: percentile_at(s, a + F.perf),
    gpu_busy: percentile_at(s, a + F.gpu_busy),
    gpu_busy_kinds: FRAME_KINDS.map((name, k) => ({
      name,
      p50: s[a + F.gpu_busy_kinds + 3 * k],
      p95: s[a + F.gpu_busy_kinds + 3 * k + 1],
      n: s[a + F.gpu_busy_kinds + 3 * k + 2],
    })),
    phases: PHASES.map((name, k) => ({ name, p95: s[a + F.phases_p95 + k] })),
    effects_p95: EFFECT_NAMES.map((name, k) => ({
      name,
      p95: s[a + F.effects_p95 + k],
    })),
    gpu_spans: GPU_SPAN_NAMES.map((name, k) => ({
      name,
      p95: s[a + F.gpu_spans_p95 + k],
      n: s[a + F.gpu_spans_cnt + k],
    })),
    kinds: FRAME_KINDS.map((name, k) => ({ name, n: s[a + F.kinds + k] })),
    gpu_px: s[a + F.gpu_px],
    ray_cnt: Object.fromEntries(
      FRAME_GROUP.rays.map((name, k) => [name, s[a + F.rays + k]]),
    ) as RayCount,
    heap_used_delta_bytes: last.heap_used_bytes - first.heap_used_bytes,
    first,
    last,
  };
}

function task_summaries(s: F64Array): TaskSummary[] {
  const out: TaskSummary[] = [];
  STREAM_NAMES.forEach((stream, c) => {
    const a = STAT.tasks.AT + STAT.tasks.slots + c * STREAM_STAT.WORDS;
    const n = s[a + STREAM_STAT.cnt];
    if (n === 0) return;
    const reasons = Array.from(
      { length: s[a + STREAM_STAT.reason_cnt] },
      (_, i) => {
        const w = a + STREAM_STAT.reasons + LIMIT.REASON_WORDS * i;
        return { fail: s[w], detail: s[w + 1], n: s[w + 2] };
      },
    );
    const means = TASK_PHASE_NAMES.map(
      (name, p) => [name, s[a + STREAM_STAT.phase_mean + p]] as [string, f64],
    ).filter(([, v]) => !Number.isNaN(v));
    out.push({
      stream,
      n,
      ok: s[a + STREAM_STAT.ok],
      aborted: s[a + STREAM_STAT.aborted],
      err: s[a + STREAM_STAT.err],
      lat: {
        p50: s[a + STREAM_STAT.lat_p50],
        p95: s[a + STREAM_STAT.lat_p95],
      },
      reasons,
      phase_mean: s[a + STREAM_STAT.phase_n] > 0 ? means : null,
    });
  });
  return out;
}

function steps_at(s: F64Array): Step[] {
  const at = STAT.steps.AT;
  return Array.from({ length: s[at + STAT.steps.cnt] }, (_, i) => {
    const w = at + STAT.steps.slots + i * STEP_STAT.WORDS;
    return {
      phase: PHASES[s[w + STEP_STAT.phase]] ?? "?",
      name: STEP_NAMES[s[w + STEP_STAT.name]] ?? "?",
      ms: s[w + STEP_STAT.ms],
      cnt: s[w + STEP_STAT.cnt],
      max_ms: s[w + STEP_STAT.max_ms],
    };
  });
}

function section<S extends { AT: u32; WORDS: u32 }>(
  s: F64Array,
  sec: S,
): Section<S> {
  const out: Record<string, f64> = {};
  for (const [k, v] of Object.entries(sec) as [string, u32][]) {
    if (k !== "AT" && k !== "WORDS") out[k] = s[sec.AT + v];
  }
  return out as Section<S>;
}

/** STAT's words over `window_ms` and NOW's, as PERF_STATS's reply names
 * them. */
async function stats_of(
  window_ms: f64 | null,
): Promise<{ stats: F64Array; now: F64Array }> {
  const w = words(await perf.stats(ui, window_ms));
  return { stats: w.subarray(0, STAT.WORDS), now: w.subarray(STAT.WORDS) };
}

/** The summary over the last `window_ms`, or over the whole recording when
 * `window_ms` is null. */
async function perf_summary(window_ms: f64 | null): Promise<Summary> {
  const { stats: s, now: n } = await stats_of(window_ms);
  const R = NOW.recording_state;
  return {
    recording_state: {
      recording: n[R.AT + R.recording] !== 0,
      elapsed_ms: n[R.AT + R.elapsed_ms],
      truncated: n[R.AT + R.truncated] !== 0,
      frame_cnt: n[R.AT + R.frame_cnt],
      task_cnt: n[R.AT + R.task_cnt],
      worst_cnt: n[R.AT + R.worst_cnt],
    },
    frames: frame_summary(s),
    tasks: task_summaries(s),
    steps: steps_at(s),
    recording: section(s, STAT.recording),
    view: section(n, NOW.view),
  };
}

/** Reads NOW's view alone. The core is asked for an empty window, so that no
 * frame or task is folded. */
async function perf_view(): Promise<ViewSection> {
  return section((await stats_of(0)).now, NOW.view);
}

/** Reads NOW's draw alone, as `perf_view` reads its view. */
async function perf_draw(): Promise<DrawSection> {
  return section((await stats_of(0)).now, NOW.draw);
}

/** The words of a record of each ring, in RING_NAMES' order. */
const RING_WORDS = [FRAME.WORDS, TASK.WORDS, WORST.WORDS] as const;

/** The records of the ring `name`: the frames and the tasks oldest first,
 * the worst frames longest first, read a page at a time until a page comes
 * short (perf.json5 LIMIT.PAGE_BYTES). */
async function ring(name: (typeof RING_NAMES)[number]): Promise<U8Array> {
  const k = RING_NAMES.indexOf(name);
  const record = 8 * RING_WORDS[k]!;
  const full = Math.floor(LIMIT.PAGE_BYTES / record) * record;
  const pages: U8Array[] = [];
  let bytes = 0;
  let from = 0;
  for (;;) {
    const page = await perf.ring(ui, k, from, 0xffff_ffff);
    pages.push(page);
    bytes += page.length;
    from += page.length / record;
    if (page.length < full) break;
  }
  const out = new Uint8Array(bytes);
  pages.reduce((at, p) => (out.set(p, at), at + p.length), 0);
  return out;
}

/** Every frame the ring holds, oldest first. */
async function perf_frames(): Promise<Frame[]> {
  const w = words(await ring("frames"));
  const cnt = w.length / FRAME.WORDS;
  return Array.from({ length: cnt }, (_, i) => frame_at(w, i * FRAME.WORDS));
}

/** The tasks the ring holds, oldest first: the last `last` of them, or all of
 * them when `last` is null. */
async function perf_tasks(last: u32 | null = null): Promise<TaskRecord[]> {
  const [tasks, strtab] = await Promise.all([
    ring("tasks"),
    perf.strtab(ui),
  ]);
  const all = words(tasks);
  const total = all.length / TASK.WORDS;
  const from = last === null ? 0 : Math.max(0, total - last);
  const cnt = total - from;
  const w = all.subarray(from * TASK.WORDS);
  return Array.from({ length: cnt }, (_, i) => {
    const o = i * TASK.WORDS;
    const phase_cnt = w[o + TASK.phase_cnt];
    const phases: [string, f64][] = [];
    for (let k = 0; k < phase_cnt; k++) {
      phases.push([
        TASK_PHASE_NAMES[w[o + TASK.phase_ids + k]] ?? "?",
        w[o + TASK.phase_vals + k],
      ]);
    }
    const start = w[o + TASK.start_ms];
    return {
      stream: STREAM_NAMES[w[o + TASK.stream]] ?? "?",
      path: td
        .decode(
          new Uint8Array(
            w.buffer,
            w.byteOffset + (o + TASK.path) * WORD,
            PATH_BYTES,
          ),
        )
        .replace(/\0+$/, ""),
      priority: w[o + TASK.priority],
      enqueue_ms: w[o + TASK.enqueue_ms],
      start_ms: Number.isNaN(start) ? null : start,
      end_ms: w[o + TASK.end_ms],
      outcome: OUTCOME_NAMES[w[o + TASK.outcome]] ?? "?",
      fail: w[o + TASK.fail],
      detail: w[o + TASK.detail],
      at: site(
        strtab,
        w[o + TASK.at_file],
        w[o + TASK.at_file_bytes],
        w[o + TASK.at_line],
      ),
      phases,
    };
  });
}

async function perf_worst(): Promise<Worst[]> {
  const w = words(await ring("worst"));
  const cnt = w.length / WORST.WORDS;
  return Array.from({ length: cnt }, (_, i) => {
    const o = i * WORST.WORDS;
    return {
      sample: frame_at(w, o + WORST.sample),
      delta:
        w[o + WORST.has_prev] === 0
          ? null
          : {
              upload_cnt: w[o + WORST.upload_cnt_delta],
              upload_bytes: w[o + WORST.upload_delta_bytes],
              heap_used_bytes: w[o + WORST.heap_used_delta_bytes],
            },
    };
  });
}

const READERS = {
  summary: perf_summary,
  view: perf_view,
  draw: perf_draw,
  frames: perf_frames,
  tasks: perf_tasks,
  worst: perf_worst,
};

export type PerfRead = keyof typeof READERS;

/** Runs a read over the UI's link, on a page and on a device alike. */
export function read<K extends PerfRead>(
  what: K,
  ...args: Parameters<(typeof READERS)[K]>
): ReturnType<(typeof READERS)[K]> {
  return (READERS[what] as (...a: unknown[]) => unknown)(...args) as ReturnType<
    (typeof READERS)[K]
  >;
}
