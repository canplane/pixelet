// @pxl/client/port/perf.ts

/**
 * The kernel's profiler, read in place through views over its memory, so
 * reading allocates nothing from the heap it measures, and decoded by the
 * layout perf.json gives (perf.spec.ts). The views hold only until
 * the next turn, so each read copies into plain values; what is made of
 * them and how it is shown is the shell's (shell/debug/perf/).
 */

import type { F64Array } from "@xpute/kit/abi/array.ts";
import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { td } from "@xpute/kit/codec/encoding.ts";
import { FaultError } from "@xpute/kit/status/error.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
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
  RING,
  RING_NAMES,
  RINGS,
  PERCENTILES,
  STAT,
  STATS,
  STEP_STAT,
  STREAM_STAT,
  TASK,
  WAKES,
  WORST,
} from "@/abi/perf.spec.ts";
import { EFFECT_NAMES, STEP_NAMES } from "@/abi/step.spec.ts";
import { FRAME_KINDS, GPU_SPAN_NAMES } from "@/abi/span.spec.ts";
import { control, runtime, kernel } from "@/runtime/kernel.ts";

const WORD = 8;
/** A task's `path` column, its bytes (perf.json `TASK`). */
const PATH_BYTES = 3 * WORD;

/** A call's answer, the address of its reply in the kernel's memory; a
 * refusal is -errno, never an address. */
function reply_at(at: i32): u32 {
  if (at < 0) throw new FaultError(-at as i32);
  return at >>> 0;
}

/** A fresh recording window, called at once. */
export function start(): void {
  kernel.perf_start();
}

/** The window frozen, so what is exported holds still. */
export function stop(): void {
  kernel.perf_stop();
}

const words = (at: u32, n: u32) => new Float64Array(runtime.core.mem, at, n);
const u32s = (at: u32, n: u32) => new Uint32Array(runtime.core.mem, at, n);
const site = (file: u32, bytes: u32, line: u32) =>
  file === 0
    ? null
    : `${td.decode(new Uint8Array(runtime.core.mem, file, bytes))}:${line}`;

/** The columns that are lists, a word a name. The names are their owners', and
 * shown by the shell. */
export const LIST_COLUMNS = ["phases", "effects", "gpu_spans"] as const;
export type ListColumn = (typeof LIST_COLUMNS)[number];
export type FrameColumn = Exclude<keyof typeof FRAME, "WORDS">;
type ScalarColumn = Exclude<FrameColumn, ListColumn>;

export type Frame = { [K in ScalarColumn]: f64 } & {
  phases: f64[];
  effects: f64[];
  gpu_spans: f64[];
};

/** Each column at its word, and the words it spans: to the next column's, the last to FRAME's end. */
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

/** The FRAME record at word `o` of `words`. */
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

export interface Percentiles {
  p50: f64;
  p95: f64;
  max: f64;
}

export interface FrameSummary {
  n: u32;
  seconds: f64;
  moving: f64;
  /** The moving frames' intervals summed. */
  moving_ms: f64;
  missed: f64;
  overran: f64;
  /** Intervals whose turns ran past them. */
  long: f64;
  turns: f64;
  /** Frames a GPU read timed, which the GPU's figures are over. */
  gpu_read: f64;
  interval: Percentiles;
  busy: Percentiles;
  tick: Percentiles;
  /** The other parts of `busy`, and what they leave of it, unmeasured. */
  effects: Percentiles;
  stage: Percentiles;
  tasks: Percentiles;
  perf: Percentiles;
  rest: Percentiles;
  /** The GPU's time: the time its spans ran, those that overlap counted once. */
  gpu_busy: Percentiles;
  /** The busy time's p50 and p95 of each kind's frames a read timed, and how
   * many they are. */
  gpu_busy_kinds: { name: string; p50: f64; p95: f64; n: f64 }[];
  /** Each phase's, each effect's and each span's p95: what is read of them; a
   * span's over the n frames a read timed that ran it. */
  phases: { name: string; p95: f64 }[];
  effects_p95: { name: string; p95: f64 }[];
  gpu_spans: { name: string; p95: f64; n: f64 }[];
  /** Frames by the renderer's kind, and the turns' last ask by wake. */
  kinds: { name: string; n: f64 }[];
  wakes: { name: string; n: f64 }[];
  /** The world's pixels of the last frame a read timed. */
  gpu_px: f64;
  /** The ray counts of the window's frames summed, by FRAME's rays columns. */
  rays: RayCounts;
  /** The heap's high-water mark, the window's last frame's less its first's. */
  heap_used_delta_bytes: f64;
  /** The window's first frame and its last. */
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
  /** null for the runtime's failures. */
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

/** A worst frame the kernel kept, and what changed from the one before it,
 * null where there was none. */
export interface Worst {
  sample: Frame;
  delta: {
    uploads: f64;
    uploads_bytes: f64;
    heap_used_bytes: f64;
  } | null;
}

/** A section's words by the names its spec gives them (perf.json), AT and
 * WORDS aside. */
export type Section<S> = { [K in Exclude<keyof S, "AT" | "WORDS">]: f64 };
/** Where the view is as the read finds it (NOW.view). */
export type ViewSection = Section<typeof NOW.view>;
/** What the frames held at the most or the least since the recording
 * started (STAT.recording). */
export type RecordingSection = Section<typeof STAT.recording>;
/** The window's ray counts by FRAME's rays columns (FRAME_GROUP.rays). */
export type RayCounts = { [K in (typeof FRAME_GROUP.rays)[number]]: f64 };

/** What PERF_STATS sums over its window, section by section: `recording` is since
 * the recording started, as the steps are, by STAT.recording's names. */
export interface Summary {
  recording_state: { recording: boolean; elapsed_ms: f64; truncated: boolean };
  frames: FrameSummary | null;
  tasks: TaskSummary[];
  steps: Step[];
  recording: RecordingSection;
  view: ViewSection;
}

/** The records each ring holds, as RINGS counts them. */
export interface RingCounts {
  frames: u32;
  tasks: u32;
  worst: u32;
}

const percentiles_at = (s: F64Array, at: u32): Percentiles => ({
  p50: s[at + PERCENTILES.p50],
  p95: s[at + PERCENTILES.p95],
  max: s[at + PERCENTILES.max],
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
    missed: s[a + F.missed],
    overran: s[a + F.overran],
    long: s[a + F.long],
    turns: s[a + F.turns],
    gpu_read: s[a + F.gpu_read],
    interval: percentiles_at(s, a + F.interval),
    busy: percentiles_at(s, a + F.busy),
    tick: percentiles_at(s, a + F.tick),
    effects: percentiles_at(s, a + F.effects),
    stage: percentiles_at(s, a + F.stage),
    tasks: percentiles_at(s, a + F.tasks),
    perf: percentiles_at(s, a + F.perf),
    rest: percentiles_at(s, a + F.rest),
    gpu_busy: percentiles_at(s, a + F.gpu_busy),
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
    wakes: WAKES.map((name, k) => ({ name, n: s[a + F.wakes + k] })),
    gpu_px: s[a + F.gpu_px],
    rays: Object.fromEntries(
      FRAME_GROUP.rays.map((name, k) => [name, s[a + F.rays + k]]),
    ) as RayCounts,
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

/** STAT's words over `window_ms` and NOW's, as PERF_STATS lays them out. */
function stats_of(window_ms: f64 | null): { stats: F64Array; now: F64Array } {
  const r = u32s(reply_at(kernel.perf_stats(window_ms)), STATS.WORDS);
  return {
    stats: words(r[STATS.stats_at], r[STATS.stats_words]),
    now: words(r[STATS.now_at], r[STATS.now_words]),
  };
}

/** The summary over the last `window_ms`, or the whole recording at null. */
function perf_summary(window_ms: f64 | null): Summary {
  const { stats: s, now: n } = stats_of(window_ms);
  const R = NOW.recording_state;
  return {
    recording_state: {
      recording: n[R.AT + R.recording] !== 0,
      elapsed_ms: n[R.AT + R.elapsed_ms],
      truncated: n[R.AT + R.truncated] !== 0,
    },
    frames: frame_summary(s),
    tasks: task_summaries(s),
    steps: steps_at(s),
    recording: section(s, STAT.recording),
    view: section(n, NOW.view),
  };
}

/** The view alone, NOW's, read over an empty window, which folds no frame or
 * task. */
function perf_view(): ViewSection {
  return section(stats_of(0).now, NOW.view);
}

/** The rings as they lie, each its address and count, with nothing reduced. */
function rings(): { frames: [u32, u32]; tasks: [u32, u32]; worst: [u32, u32] } {
  const r = u32s(reply_at(kernel.perf_rings()), RINGS.WORDS);
  const ring = (name: (typeof RING_NAMES)[number]): [u32, u32] => {
    const at = RINGS.slots + RING_NAMES.indexOf(name) * RING.WORDS;
    return [r[at + RING.at], r[at + RING.cnt]];
  };
  return { frames: ring("frames"), tasks: ring("tasks"), worst: ring("worst") };
}

function perf_counts(): RingCounts {
  const { frames, tasks, worst } = rings();
  return { frames: frames[1], tasks: tasks[1], worst: worst[1] };
}

/** Every frame the ring holds, oldest first. */
function perf_frames(): Frame[] {
  const [at, cnt] = rings().frames;
  const w = words(at, cnt * FRAME.WORDS);
  return Array.from({ length: cnt }, (_, i) => frame_at(w, i * FRAME.WORDS));
}

/** The tasks the ring holds, oldest first: its last `last`, or every one at null. */
function perf_tasks(last: u32 | null = null): TaskRecord[] {
  const [ring, held] = rings().tasks;
  const from = last === null ? 0 : Math.max(0, held - last);
  const at = ring + from * TASK.WORDS * WORD;
  const cnt = held - from;
  const w = words(at, cnt * TASK.WORDS);
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
            runtime.core.mem,
            at + (o + TASK.path) * WORD,
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
        w[o + TASK.at_file],
        w[o + TASK.at_file_bytes],
        w[o + TASK.at_line],
      ),
      phases,
    };
  });
}

/** The worst frames the kernel kept since recording began. */
function perf_worst(): Worst[] {
  const [at, cnt] = rings().worst;
  const w = words(at, cnt * WORST.WORDS);
  return Array.from({ length: cnt }, (_, i) => {
    const o = i * WORST.WORDS;
    return {
      sample: frame_at(w, o + WORST.sample),
      delta:
        w[o + WORST.has_prev] === 0
          ? null
          : {
              uploads: w[o + WORST.uploads_delta],
              uploads_bytes: w[o + WORST.uploads_delta_bytes],
              heap_used_bytes: w[o + WORST.heap_used_delta_bytes],
            },
    };
  });
}

/** What the profiler reads where the kernel's memory is. */
const READERS = {
  summary: perf_summary,
  view: perf_view,
  counts: perf_counts,
  frames: perf_frames,
  tasks: perf_tasks,
  worst: perf_worst,
};

export type PerfRead = keyof typeof READERS;

/** A reading, where the kernel's memory is: beside the UI on a device, in
 * the kernel's worker on a page. */
export function read<K extends PerfRead>(
  what: K,
  ...args: Parameters<(typeof READERS)[K]>
): Promise<ReturnType<(typeof READERS)[K]>> {
  return control.perf(what, args) as Promise<ReturnType<(typeof READERS)[K]>>;
}

/** `read`'s answer, read here, beside the kernel's memory. */
export function answer(what: PerfRead, args: unknown[]): unknown {
  return (READERS[what] as (...a: unknown[]) => unknown)(...args);
}
