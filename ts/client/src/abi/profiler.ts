// @pxl/client/abi/profiler.ts

/**
 * The kernel's profiler, read in place through views over its memory, so
 * reading allocates nothing from the heap it measures, and decoded by the
 * layout profiler.json gives (profiler.spec.ts). The views hold only until
 * the next turn, so each read copies into plain values; what is made of
 * them and how it is shown is the shell's (shell/debug/profiler/).
 */

import type { F64Array } from "@xpute/kit/abi/array.ts";
import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { td } from "@xpute/kit/codec/encoding.ts";
import { FaultError } from "@xpute/kit/status/error.ts";
import type { Errno } from "@xpute/kit/status/errno.spec.ts";
import type { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { PHASES } from "@xpute/host/sched/tick.spec.ts";
import { Command, encode_profiler_read } from "@/abi/cmd.spec.ts";
import {
  OUTCOME_NAMES,
  STREAM_NAMES,
  TASK_PHASE_NAMES,
} from "@/abi/demand.spec.ts";
import {
  FRAME,
  LIMIT,
  NOW,
  READ,
  RING,
  RING_NAMES,
  RINGS,
  SPREAD,
  STAT,
  STEP_STAT,
  STREAM_STAT,
  TASK,
  WAKES,
  WORST,
} from "@/abi/profiler.spec.ts";
import { EFFECT_NAMES, STEP_NAMES } from "@/abi/step.spec.ts";
import { FRAME_KINDS, GPU_SPAN_NAMES } from "@/abi/span.spec.ts";
import { HOST } from "@/host/mount.ts";

const WORD = 8;

/** A command answered inside the call: the address of its reply in the kernel's memory. */
function call(cmd: Command, pkt: PacketWriter | null = null): u32 {
  return HOST.doorbell.call(cmd, pkt, (at) => {
    // A refusal is -errno, never an address.
    if (at < 0) throw new FaultError(-at as Errno);
    return at >>> 0;
  });
}

const words = (at: u32, n: u32) => new Float64Array(HOST.guest.mem, at, n);
const u32s = (at: u32, n: u32) => new Uint32Array(HOST.guest.mem, at, n);
const site = (file: u32, bytes: u32, line: u32) =>
  file === 0
    ? null
    : `${td.decode(new Uint8Array(HOST.guest.mem, file, bytes))}:${line}`;

/** The lanes that are lists: a word a name, a census entry two (its draws and
 * triangles). The names are their owners', and shown by the shell. */
export const LIST_LANES = [
  "phases",
  "effects",
  "gpu_spans",
  "census",
  "zoom_tiles",
  "gpu_objects",
  "waiting",
  "out",
] as const;
export type ListLane = typeof LIST_LANES[number];
export type FrameLane = Exclude<keyof typeof FRAME, "WORDS">;
type ScalarLane = Exclude<FrameLane, ListLane>;

export type Frame =
  & { [K in ScalarLane]: f64 }
  & {
    phases: f64[];
    effects: f64[];
    gpu_spans: f64[];
    census: [f64, f64][];
    zoom_tiles: f64[];
    gpu_objects: f64[];
    waiting: f64[];
    out: f64[];
  };

/** Each lane at its word, and the words it spans: to the next lane's, the last to FRAME's end. */
export const LANES: readonly { name: FrameLane; at: u32; width: u32 }[] =
  (() => {
    const at = (Object.entries(FRAME) as [FrameLane | "WORDS", u32][]).filter((
      [k],
    ) => k !== "WORDS") as [FrameLane, u32][];
    at.sort((a, b) => a[1] - b[1]);
    return at.map(([name, a], i) => ({
      name,
      at: a,
      width: (i + 1 < at.length ? at[i + 1][1] : FRAME.WORDS) - a,
    }));
  })();

const LISTS: ReadonlySet<string> = new Set(LIST_LANES);
export const is_list = (lane: FrameLane): lane is ListLane => LISTS.has(lane);

/** The FRAME record at word `o` of `words`. */
export function frame_at(words: ArrayLike<f64>, o: u32): Frame {
  const f: Record<string, f64 | f64[] | [f64, f64][]> = {};
  for (const { name, at, width } of LANES) {
    const w = (k: u32) => words[o + at + k];
    if (name === "census") {
      f[name] = Array.from(
        { length: width / 2 },
        (_, k) => [w(2 * k), w(2 * k + 1)] as [f64, f64],
      );
    } else if (is_list(name)) {
      f[name] = Array.from({ length: width }, (_, k) => w(k));
    } else f[name] = w(0);
  }
  return f as Frame;
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
  /** The moving frames' intervals summed. */
  moving_ms: f64;
  missed: f64;
  overran: f64;
  /** Intervals whose turns ran past them. */
  long: f64;
  turns: f64;
  /** Frames a GPU read timed, which the GPU's figures are over. */
  gpu_read: f64;
  interval: Spread;
  busy: Spread;
  tick: Spread;
  /** The other parts of `busy`, and what they leave of it, unmeasured. */
  effects: Spread;
  stage: Spread;
  tasks: Spread;
  profiler: Spread;
  rest: Spread;
  gpu: Spread;
  /** Each phase's, each effect's and each span's p95: what is read of them. */
  phases: { name: string; p95: f64 }[];
  effects_p95: { name: string; p95: f64 }[];
  gpu_spans: { name: string; p95: f64 }[];
  /** Frames by the renderer's kind, and the turns' last ask by wake. */
  kinds: { name: string; n: f64 }[];
  wakes: { name: string; n: f64 }[];
  heap_used_delta_bytes: f64;
  /** The window's first frame and its last. */
  first: Frame;
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

/** A worst frame the kernel kept, with the one before it where there was one. */
export interface Worst {
  sample: Frame;
  prev: Frame | null;
}

/** What PROFILER_READ sums over its window, section by section. */
export interface Summary {
  rec: { recording: boolean; elapsed_ms: f64; truncated: boolean };
  frames: FrameSummary | null;
  tasks: TaskSummary[];
  steps: Step[];
  view: Record<string, f64>;
}

/** The records each ring holds, as RINGS counts them. */
export interface RingCounts {
  frames: u32;
  tasks: u32;
  worst: u32;
}

const spread_at = (s: F64Array, at: u32): Spread => ({
  p50: s[at + SPREAD.p50],
  p95: s[at + SPREAD.p95],
  max: s[at + SPREAD.max],
});

function frame_summary(s: F64Array): FrameSummary | null {
  const F = STAT.frames;
  const a = F.AT;
  if (s[a + F.present] === 0) return null;
  return {
    n: s[a + F.n],
    seconds: s[a + F.seconds],
    moving: s[a + F.moving],
    moving_ms: s[a + F.moving_ms],
    missed: s[a + F.missed],
    overran: s[a + F.overran],
    long: s[a + F.long],
    turns: s[a + F.turns],
    gpu_read: s[a + F.gpu_read],
    interval: spread_at(s, a + F.interval),
    busy: spread_at(s, a + F.busy),
    tick: spread_at(s, a + F.tick),
    effects: spread_at(s, a + F.effects),
    stage: spread_at(s, a + F.stage),
    tasks: spread_at(s, a + F.tasks),
    profiler: spread_at(s, a + F.profiler),
    rest: spread_at(s, a + F.rest),
    gpu: spread_at(s, a + F.gpu),
    phases: PHASES.map((name, k) => ({ name, p95: s[a + F.phases_p95 + k] })),
    effects_p95: EFFECT_NAMES.map((name, k) => ({
      name,
      p95: s[a + F.effects_p95 + k],
    })),
    gpu_spans: GPU_SPAN_NAMES.map((name, k) => ({
      name,
      p95: s[a + F.gpu_spans_p95 + k],
    })),
    kinds: FRAME_KINDS.map((name, k) => ({ name, n: s[a + F.kinds + k] })),
    wakes: WAKES.map((name, k) => ({ name, n: s[a + F.wakes + k] })),
    heap_used_delta_bytes: s[a + F.heap_used_delta_bytes],
    first: frame_at(s, a + F.first),
    last: frame_at(s, a + F.last),
  };
}

function task_summaries(s: F64Array): TaskSummary[] {
  const out: TaskSummary[] = [];
  STREAM_NAMES.forEach((stream, c) => {
    const a = STAT.tasks.AT + STAT.tasks.slots + c * STREAM_STAT.WORDS;
    const n = s[a + STREAM_STAT.n];
    if (n === 0) return;
    const reasons = Array.from(
      { length: s[a + STREAM_STAT.reason_cnt] },
      (_, i) => {
        const w = a + STREAM_STAT.reasons + LIMIT.REASON_WORDS * i;
        return { fail: s[w], detail: s[w + 1], n: s[w + 2] };
      },
    );
    const means = TASK_PHASE_NAMES.map((name, p) =>
      [name, s[a + STREAM_STAT.phase_mean + p]] as [string, f64]
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

function section(s: F64Array, sec: Record<string, u32>): Record<string, f64> {
  const out: Record<string, f64> = {};
  for (const [k, v] of Object.entries(sec)) {
    if (k !== "AT" && k !== "WORDS") out[k] = s[sec.AT + v];
  }
  return out;
}

/** STAT's words over `window_ms` and NOW's, as PROFILER_READ lays them out. */
function read(window_ms: f64 | null): { stats: F64Array; now: F64Array } {
  const r = u32s(
    call(Command.PROFILER_READ, encode_profiler_read(HOST.doorbell, window_ms)),
    READ.WORDS,
  );
  return {
    stats: words(r[READ.stats_at], r[READ.stats_words]),
    now: words(r[READ.now_at], r[READ.now_words]),
  };
}

/** The summary over the last `window_ms`, or the whole recording at null. */
export function profiler_summary(window_ms: f64 | null): Summary {
  const { stats: s, now: n } = read(window_ms);
  const R = NOW.rec;
  return {
    rec: {
      recording: n[R.AT + R.recording] !== 0,
      elapsed_ms: n[R.AT + R.elapsed_ms],
      truncated: n[R.AT + R.truncated] !== 0,
    },
    frames: frame_summary(s),
    tasks: task_summaries(s),
    steps: steps_at(s),
    view: section(n, NOW.view),
  };
}

/** The view alone, NOW's, read over an empty window, which folds no frame or
 * task. */
export function profiler_view(): Summary["view"] {
  return section(read(0).now, NOW.view);
}

/** The rings as they lie, each its address and count, with nothing reduced. */
function rings(): { frames: [u32, u32]; tasks: [u32, u32]; worst: [u32, u32] } {
  const r = u32s(call(Command.PROFILER_RINGS), RINGS.WORDS);
  const ring = (name: (typeof RING_NAMES)[number]): [u32, u32] => {
    const at = RINGS.slots + RING_NAMES.indexOf(name) * RING.WORDS;
    return [r[at + RING.at], r[at + RING.cnt]];
  };
  return { frames: ring("frames"), tasks: ring("tasks"), worst: ring("worst") };
}

export function profiler_counts(): RingCounts {
  const { frames, tasks, worst } = rings();
  return { frames: frames[1], tasks: tasks[1], worst: worst[1] };
}

/** Every frame the ring holds, oldest first. */
export function profiler_frames(): Frame[] {
  const [at, cnt] = rings().frames;
  const w = words(at, cnt * FRAME.WORDS);
  return Array.from({ length: cnt }, (_, i) => frame_at(w, i * FRAME.WORDS));
}

/** The tasks the ring holds, oldest first: its last `last`, or every one at null. */
export function profiler_tasks(last: u32 | null = null): TaskRecord[] {
  const [ring, held] = rings().tasks;
  const from = last === null ? 0 : Math.max(0, held - last);
  const at = ring + from * TASK.WORDS * WORD;
  const cnt = held - from;
  const w = words(at, cnt * TASK.WORDS);
  const subjects = new BigUint64Array(
    HOST.guest.mem,
    at,
    cnt * TASK.WORDS,
  );
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
      subject: subjects[o + TASK.subject].toString(),
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
export function profiler_worst(): Worst[] {
  const [at, cnt] = rings().worst;
  const w = words(at, cnt * WORST.WORDS);
  return Array.from({ length: cnt }, (_, i) => {
    const o = i * WORST.WORDS;
    return {
      sample: frame_at(w, o + WORST.sample),
      prev: w[o + WORST.has_prev] === 0 ? null : frame_at(w, o + WORST.prev),
    };
  });
}
