// @pxl/client/service/core/perf.ts

/**
 * The core's profiler, read from the words each read's reply names, which go
 * with the reply's frame. A read is one call, and the core keeps nothing of
 * it: it copies what it answers into the core's heap, which the profiler
 * measures, a page at most (`LIMIT.PAGE_SZ`), freed on reply. The statistics,
 * the worst frames and the string table are a page each. The frames and the
 * tasks are tables read by sequence, a page a call (`rows`): a row's sequence
 * is how many rows its table landed before it, the reader asks from the
 * sequence its last call ended at, and the reply says where its rows start,
 * so the reader counts the rows the table dropped before it came to them
 * (`lost`). A task's row never changes. A frame's is written again a frame or
 * two after it lands, as the GPU's timings and the ray counts of its frame
 * land, so the frames are read from the oldest to the end in one pass, never
 * a part of them kept from an earlier one. The layout of what is read is
 * perf.json5's (perf.spec.ts). What is made of those values, and how they are
 * shown, is the UI's work (ui/feature/debug/perf/).
 */

import type { F64Array, U8Array } from "@xpute/kit/abi/array.ts";
import { type f64, type u32, U32_MAX, U64_MAX } from "@xpute/kit/abi/word.ts";
import { td } from "@xpute/kit/codec/encoding.ts";
import { ComponentError } from "@xpute/link/link.ts";
import { PHASES } from "@/abi/phase.spec.ts";
import { BASE_KINDS } from "@/abi/publication.spec.ts";
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
  TABLE_NAMES,
  PERCENTILE,
  STAT,
  STEP_STAT,
  STREAM_STAT,
  TASK,
  WORST,
} from "@/abi/perf.spec.ts";
import {
  INVALIDATION_REASONS,
  EFFECT_NAMES,
  STAGE_PART_NAMES,
  STEP_NAMES,
  STEP_PHASE,
} from "@/abi/step.spec.ts";
import { FRAME_KINDS, GPU_SPAN_NAMES } from "@/abi/span.spec.ts";
import * as perf from "@/abi/perf.ts";
import { ui } from "@/service/core/core.ts";

const WORD = 8;
/** How many bytes a task's `path` column holds (perf.json5 `TASK`). */
const PATH_SZ = 3 * WORD;

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
 * and its file as the string table `strtab` names it at `file`, or unknown at
 * offset 0, where the table had no room for it. */
const site = (strtab: U8Array, file: u32, bytes: u32, line: u32) =>
  line === 0
    ? null
    : `${file === 0 ? "?" : td.decode(strtab.subarray(file, file + bytes))}:${line}`;

/** The `FRAME` columns that are lists, one word for each name in the list. */
const LIST_COLUMNS = [
  "phases",
  "stage_parts",
  "effects",
  "invalidations",
  "gpu_spans",
  "loads",
] as const;
export type ListColumn = (typeof LIST_COLUMNS)[number];
export type FrameColumn = Exclude<keyof typeof FRAME, "WORDS">;
type ScalarColumn = Exclude<FrameColumn, ListColumn>;

export type Frame = { [K in ScalarColumn]: f64 } & {
  [K in ListColumn]: f64[];
};

/** Each `FRAME` column with its first word and its width in words. */
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
export const isList = (column: FrameColumn): column is ListColumn =>
  LISTS.has(column);

export function frameAt(words: ArrayLike<f64>, o: u32): Frame {
  const f: Record<string, f64 | f64[]> = {};
  for (const { name, at, width } of COLUMNS) {
    const w = (k: u32) => words[o + at + k];
    if (isList(name)) {
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
  movingMs: f64;
  overran: f64;
  /** How many frames a GPU read timed. The GPU's figures are taken over
   * these frames alone. */
  gpuTimed: f64;
  interval: Percentile;
  tick: Percentile;
  effect: Percentile;
  pump: Percentile;
  stage: Percentile;
  /** The ground's fill after a frame, and the bricks it filled over the
   * window. */
  fill: Percentile;
  fillCount: f64;
  perf: Percentile;
  /** The GPU's busy time: the time its spans ran, with overlapping spans
   * counted once. */
  gpuBusy: Percentile;
  /** For each frame kind, the p50 and p95 of the GPU's busy time over the
   * frames of that kind that a read timed, and how many there were. */
  gpuBusyKinds: { name: string; p50: f64; p95: f64; n: f64 }[];
  /** The p95 of each phase, each part of the stage, each effect and each GPU
   * span, which is all that is read of them. A span's p95 is over the `n`
   * frames that a read timed and that ran the span. */
  phases: { name: string; p95: f64 }[];
  stagePartsP95: { name: string; p95: f64 }[];
  effectsP95: { name: string; p95: f64 }[];
  /** How many times a frame was asked for over the window, by reason. */
  invalidations: { name: string; n: f64 }[];
  gpuSpans: { name: string; p95: f64; n: f64 }[];
  /** How many frames were of each kind the renderer draws. */
  kinds: { name: string; n: f64 }[];
  /** How many of the world's pixels the last frame a read timed drew. */
  gpuPx: f64;
  /** The ray counts of the window's frames, summed for each of `FRAME`'s
   * `rays` columns. */
  rayCount: RayCount;
  /** How far the heap's high-water mark rose over the window: the last
   * frame's mark less the first frame's. */
  heapUsedDeltaBytes: f64;
  last: Frame;
}

export interface TaskRecord {
  stream: string;
  path: string;
  priority: f64;
  enqueueMs: f64;
  startMs: f64 | null;
  endMs: f64;
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
  failed: u32;
  latency: { p50: f64; p95: f64 };
  reasons: { fail: u32; detail: u32; n: u32 }[];
  phaseMean: [string, f64][] | null;
}

export interface Step {
  phase: string;
  name: string;
  ms: f64;
  count: f64;
  maxMs: f64;
}

/** One of the worst frames the core kept, and what changed since the frame
 * before it. `delta` is null when there was no frame before it. */
export interface Worst {
  sample: Frame;
  delta: {
    uploadCount: f64;
    uploadBytes: f64;
    heapUsedBytes: f64;
  } | null;
}

type Section<S> = { [K in Exclude<keyof S, "AT" | "WORDS">]: f64 };
export type ViewSection = Section<typeof NOW.view>;
type DrawSection = Section<typeof NOW.draw>;
/** The highest or lowest values the frames held since the recording started
 * (`STAT.recording`), and the tiles each base layer's tile cache refused a
 * slot. */
export type RecordingSection = Omit<
  Section<typeof STAT.recording>,
  "refusals"
> & { refusals: { name: string; n: f64 }[] };
export type RayCount = { [K in (typeof FRAME_GROUP.rays)[number]]: f64 };

/** Whether the profiler records, for how long, how much it holds, and
 * whether it has filled. */
export interface RecordingState {
  recording: boolean;
  elapsedMs: f64;
  truncated: boolean;
  frameCount: u32;
  taskCount: u32;
}

/** What `perf.stats` sums over its window, section by section. `recording` and
 * `steps` are over the whole recording instead, from when it started. */
interface Summary {
  recordingState: RecordingState;
  frames: FrameSummary | null;
  tasks: TaskSummary[];
  steps: Step[];
  recording: RecordingSection;
  view: ViewSection;
}

const percentileAt = (s: F64Array, at: u32): Percentile => ({
  p50: s[at + PERCENTILE.p50],
  p95: s[at + PERCENTILE.p95],
  max: s[at + PERCENTILE.max],
});

function frameSummary(stats: F64Array): FrameSummary | null {
  const layout = STAT.frames;
  const at = layout.AT;
  if (stats[at + layout.present] === 0) return null;
  const last = frameAt(stats, at + layout.last);
  return {
    n: stats[at + layout.cnt],
    seconds: stats[at + layout.seconds],
    moving: stats[at + layout.moving],
    movingMs: stats[at + layout.moving_ms],
    overran: stats[at + layout.overran],
    gpuTimed: stats[at + layout.gpu_timed],
    interval: percentileAt(stats, at + layout.interval),
    tick: percentileAt(stats, at + layout.tick),
    effect: percentileAt(stats, at + layout.effect),
    pump: percentileAt(stats, at + layout.pump),
    stage: percentileAt(stats, at + layout.stage),
    fill: percentileAt(stats, at + layout.fill),
    fillCount: stats[at + layout.fill_cnt],
    perf: percentileAt(stats, at + layout.perf),
    gpuBusy: percentileAt(stats, at + layout.gpu_busy),
    gpuBusyKinds: FRAME_KINDS.map((name, k) => ({
      name,
      p50: stats[at + layout.gpu_busy_kinds + 3 * k],
      p95: stats[at + layout.gpu_busy_kinds + 3 * k + 1],
      n: stats[at + layout.gpu_busy_kinds + 3 * k + 2],
    })),
    phases: PHASES.map((name, k) => ({
      name,
      p95: stats[at + layout.phases_p95 + k],
    })),
    stagePartsP95: STAGE_PART_NAMES.map((name, k) => ({
      name,
      p95: stats[at + layout.stage_parts_p95 + k],
    })),
    effectsP95: EFFECT_NAMES.map((name, k) => ({
      name,
      p95: stats[at + layout.effects_p95 + k],
    })),
    invalidations: INVALIDATION_REASONS.map((name, k) => ({
      name,
      n: stats[at + layout.invalidations + k],
    })),
    gpuSpans: GPU_SPAN_NAMES.map((name, k) => ({
      name,
      p95: stats[at + layout.gpu_spans_p95 + k],
      n: stats[at + layout.gpu_spans_cnt + k],
    })),
    kinds: FRAME_KINDS.map((name, k) => ({
      name,
      n: stats[at + layout.kinds + k],
    })),
    gpuPx: stats[at + layout.gpu_px],
    rayCount: Object.fromEntries(
      FRAME_GROUP.rays.map((name, k) => [name, stats[at + layout.rays + k]]),
    ) as RayCount,
    heapUsedDeltaBytes: stats[at + layout.heap_used_delta_bytes],
    last,
  };
}

function taskSummaries(stats: F64Array): TaskSummary[] {
  const out: TaskSummary[] = [];
  STREAM_NAMES.forEach((stream, c) => {
    const at = STAT.tasks.AT + STAT.tasks.slots + c * STREAM_STAT.WORDS;
    const n = stats[at + STREAM_STAT.cnt];
    if (n === 0) return;
    const reasons = Array.from(
      { length: stats[at + STREAM_STAT.reason_cnt] },
      (_, i) => {
        const w = at + STREAM_STAT.reasons + LIMIT.REASON_WORDS * i;
        return { fail: stats[w], detail: stats[w + 1], n: stats[w + 2] };
      },
    );
    const means = TASK_PHASE_NAMES.map(
      (name, p) =>
        [name, stats[at + STREAM_STAT.phase_mean + p]] as [string, f64],
    ).filter(([, v]) => !Number.isNaN(v));
    out.push({
      stream,
      n,
      ok: stats[at + STREAM_STAT.ok],
      aborted: stats[at + STREAM_STAT.aborted],
      failed: stats[at + STREAM_STAT.err],
      latency: {
        p50: stats[at + STREAM_STAT.lat_p50],
        p95: stats[at + STREAM_STAT.lat_p95],
      },
      reasons,
      phaseMean: stats[at + STREAM_STAT.phase_n] > 0 ? means : null,
    });
  });
  return out;
}

function stepsAt(stats: F64Array): Step[] {
  const at = STAT.steps.AT;
  return Array.from({ length: stats[at + STAT.steps.cnt] }, (_, i) => {
    const w = at + STAT.steps.slots + i * STEP_STAT.WORDS;
    const name = stats[w + STEP_STAT.name];
    return {
      phase: PHASES[STEP_PHASE[name]] ?? "?",
      name: STEP_NAMES[name] ?? "?",
      ms: stats[w + STEP_STAT.ms],
      count: stats[w + STEP_STAT.cnt],
      maxMs: stats[w + STEP_STAT.max_ms],
    };
  });
}

function section<S extends { AT: u32; WORDS: u32 }>(
  stats: F64Array,
  layout: S,
): Section<S> {
  const out: Record<string, f64> = {};
  for (const [k, v] of Object.entries(layout) as [string, u32][]) {
    if (k !== "AT" && k !== "WORDS") out[k] = stats[layout.AT + v];
  }
  return out as Section<S>;
}

/** STAT's words over `windowMs` and NOW's, as `perf.stats`'s reply names
 * them. */
async function statsOf(
  windowMs: f64 | null,
): Promise<{ stats: F64Array; now: F64Array }> {
  const w = words(await perf.stats(ui, windowMs));
  return { stats: w.subarray(0, STAT.WORDS), now: w.subarray(STAT.WORDS) };
}

/** The summary over the last `windowMs`, or over the whole recording when
 * `windowMs` is null. */
async function perfSummary(windowMs: f64 | null): Promise<Summary> {
  const { stats, now } = await statsOf(windowMs);
  const layout = NOW.recording_state;
  return {
    recordingState: {
      recording: now[layout.AT + layout.recording] !== 0,
      elapsedMs: now[layout.AT + layout.elapsed_ms],
      truncated: now[layout.AT + layout.truncated] !== 0,
      frameCount: now[layout.AT + layout.frame_cnt],
      taskCount: now[layout.AT + layout.task_cnt],
    },
    frames: frameSummary(stats),
    tasks: taskSummaries(stats),
    steps: stepsAt(stats),
    recording: {
      ...section(stats, STAT.recording),
      refusals: BASE_KINDS.map((name, k) => ({
        name,
        n: stats[STAT.recording.AT + STAT.recording.refusals + k],
      })),
    },
    view: section(now, NOW.view),
  };
}

/** Reads `NOW`'s view alone. The core is asked for an empty window, so that
 * no frame or task is folded. */
async function perfView(): Promise<ViewSection> {
  return section((await statsOf(0)).now, NOW.view);
}

/** Reads `NOW`'s draw alone, as `perfView` reads its view. */
async function perfDraw(): Promise<DrawSection> {
  return section((await statsOf(0)).now, NOW.draw);
}

/** The words of a row of each table, in `TABLE_NAMES`' order. */
const ROW_WORDS = [FRAME.WORDS, TASK.WORDS] as const;

/**
 * The rows of the table `name`, oldest first, read by sequence a page a call
 * (`perf.json5` `LIMIT.PAGE_SZ`) up to the table's end as the last call finds
 * it: its newest `count` rows, or every row it holds where `count` is null.
 * `lost` is how many rows the table dropped between two of the calls, which
 * the read never took.
 */
async function rows(
  name: (typeof TABLE_NAMES)[number],
  count: u32 | null,
): Promise<{ words: F64Array; lost: u32 }> {
  const k = TABLE_NAMES.indexOf(name);
  const rowSz = WORD * ROW_WORDS[k];
  const cap = count ?? U32_MAX;
  // A sequence past the table's end asks for its newest rows (perf.wit
  // `table`).
  let at = count === null ? 0n : U64_MAX;
  const pages: U8Array[] = [];
  let rowCount = 0;
  let lost = 0;
  for (;;) {
    const page = await perf.table(ui, k, at, cap - rowCount);
    // The first call's rows start where the table does; a later call's start
    // past where the last ended only by the rows dropped between them.
    if (pages.length > 0) lost += Number(page.first - at);
    pages.push(page.bytes);
    rowCount += page.bytes.length / rowSz;
    at = page.first + BigInt(page.bytes.length / rowSz);
    if (at >= page.end || rowCount >= cap) break;
  }
  const out = new Uint8Array(rowCount * rowSz);
  pages.reduce((o, p) => (out.set(p, o), o + p.length), 0);
  return { words: words(out), lost };
}

/** Every frame the table holds, oldest first, and the frames lost to the
 * read. */
async function perfFrames(): Promise<{ frames: Frame[]; lost: u32 }> {
  const { words: w, lost } = await rows("frames", null);
  const count = w.length / FRAME.WORDS;
  return {
    frames: Array.from({ length: count }, (_, i) =>
      frameAt(w, i * FRAME.WORDS),
    ),
    lost,
  };
}

/** The tasks the table holds, oldest first: its newest `count`, or all of
 * them when `count` is null, and the tasks lost to the read. */
async function perfTasks(
  count: u32 | null = null,
): Promise<{ tasks: TaskRecord[]; lost: u32 }> {
  // The string table is read after the tasks, so that it holds every name
  // their records point into.
  const { words: records, lost } = await rows("tasks", count);
  const strtab = await perf.strtab(ui);
  const tasks = Array.from({ length: records.length / TASK.WORDS }, (_, i) => {
    const at = i * TASK.WORDS;
    const phaseCount = records[at + TASK.phase_cnt];
    const phases: [string, f64][] = [];
    for (let k = 0; k < phaseCount; k++) {
      phases.push([
        TASK_PHASE_NAMES[records[at + TASK.phase_ids + k]] ?? "?",
        records[at + TASK.phase_vals + k],
      ]);
    }
    const start = records[at + TASK.start_ms];
    return {
      stream: STREAM_NAMES[records[at + TASK.stream]] ?? "?",
      path: td
        .decode(
          new Uint8Array(
            records.buffer,
            records.byteOffset + (at + TASK.path) * WORD,
            PATH_SZ,
          ),
        )
        .replace(/\0+$/, ""),
      priority: records[at + TASK.priority],
      enqueueMs: records[at + TASK.enqueue_ms],
      startMs: Number.isNaN(start) ? null : start,
      endMs: records[at + TASK.end_ms],
      outcome: OUTCOME_NAMES[records[at + TASK.outcome]] ?? "?",
      fail: records[at + TASK.fail],
      detail: records[at + TASK.detail],
      at: site(
        strtab,
        records[at + TASK.at_file],
        records[at + TASK.at_file_sz],
        records[at + TASK.at_line],
      ),
      phases,
    };
  });
  return { tasks, lost };
}

/** The worst frames the core kept, longest first. */
async function perfWorst(): Promise<Worst[]> {
  const records = words(await perf.worst(ui));
  const count = records.length / WORST.WORDS;
  return Array.from({ length: count }, (_, i) => {
    const at = i * WORST.WORDS;
    return {
      sample: frameAt(records, at + WORST.sample),
      delta:
        records[at + WORST.has_prev] === 0
          ? null
          : {
              uploadCount: records[at + WORST.upload_cnt_delta],
              uploadBytes: records[at + WORST.upload_delta_bytes],
              heapUsedBytes: records[at + WORST.heap_used_delta_bytes],
            },
    };
  });
}

const READERS = {
  summary: perfSummary,
  view: perfView,
  draw: perfDraw,
  frames: perfFrames,
  tasks: perfTasks,
  worst: perfWorst,
};

export type PerfRead = keyof typeof READERS;

/** Runs a read over the UI's end, on the web and natively alike. */
export function read<K extends PerfRead>(
  what: K,
  ...args: Parameters<(typeof READERS)[K]>
): ReturnType<(typeof READERS)[K]> {
  return (READERS[what] as (...a: unknown[]) => unknown)(...args) as ReturnType<
    (typeof READERS)[K]
  >;
}
