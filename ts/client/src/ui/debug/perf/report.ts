// @pxl/client/ui/debug/perf/report.ts

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";

import type {
  Frame,
  Percentiles,
  RayCounts,
  RecordingSection,
  TaskRecord,
  TaskSummary,
  ViewSection,
  Worst,
} from "@/port/perf.ts";
import { n1, pct, phaseText, tris, valueText } from "./column.ts";
import { fps, type PerfReport } from "./view.ts";
import { fixed } from "@/ui/fixed.ts";
import { EFFECT_NAMES } from "@/abi/step.spec.ts";
import { type LongFrame } from "@/service/types.ts";
import * as long_frame from "@/service/long-frame.ts";
import * as share from "@/service/share.ts";

const signed = (n: number) => (n >= 0 ? `+${n}` : String(n));
const signedBytes = (b: number) =>
  `${b >= 0 ? "+" : ""}${valueText("_bytes", b)}`;
const bytes = (b: number) => valueText("_bytes", b);
/** Says how far the interval's turns ran past the interval's end, or, when
 * they ended inside it, how much of the interval was left with no turn. */
const pastText = (f: Frame) =>
  f.over_ms > 0
    ? `over by ${n1(f.over_ms)}`
    : `off-turn ${n1(f.interval_ms - f.busy_ms)}`;
const percentiles = (s: Percentiles) =>
  `p50 ${n1(s.p50)}  p95 ${n1(s.p95)}  max ${n1(s.max)}`;
/** A ray column's name without its observer's prefix, such as `cast` for
 * `eye_cast` (FRAME_GROUP.rays). */
type RayColumn<K = keyof RayCounts> = K extends `eye_${infer C}` ? C : never;

/** Formats the counts the `ray_counts` setting collects, which the core sums
 * over the window's frames (the rays columns of FRAME in perf.json). Each
 * observer, the eye and the light, gets one line when it cast in the window.
 * Per cast, the line gives the nodes asked by its beam rays, by its samples
 * and by its walks again (`miss`), and how many of the walks' nodes were
 * asked of the world (`asked`). It gives the pixels or map slots cast again,
 * split into those that reused the visible sample, those that walked and
 * those that took the sky, each as a share of all its pixels or slots. Last
 * come the nodes each walking pixel or slot asked, and the boxes drawn per
 * cast. */
const raysText = (f: RayCounts) => {
  const observer = (name: "eye" | "light") => {
    const at = (column: RayColumn) => f[`${name}_${column}`];
    const casts = at("cast");
    if (casts === 0) return [];
    const perCast = (n: number) => n1(n / casts);
    const rays = at("rays");
    const part = (n: number) =>
      rays > 0 ? `${fixed((100 * n) / rays, 2)}%` : `${perCast(n)} a cast`;
    const again = at("reused") + at("walked") + at("sky");
    return [
      `  ${name.padEnd(7)} cast in ${casts} frames   nodes a cast: beam ${perCast(
        at("beam"),
      )}  sample ${perCast(at("sample"))}  miss ${perCast(
        at("miss"),
      )} (${perCast(at("asked"))} asked of the world)   cast again ${part(again)}: reused ${part(at("reused"))}  walked ${part(
        at("walked"),
      )} (${n1(
        at("miss") / Math.max(1, at("walked")),
      )} nodes each)  sky ${part(at("sky"))}   boxes ${perCast(at("boxes"))} a cast`,
    ];
  };
  return [...observer("eye"), ...observer("light")];
};

const deg = (rad: number) => (rad * 180) / Math.PI;

/** Where the view stood when the report was read, in the core's numbers
 * (perf.json NOW.view), and how many of the world's pixels the last frame
 * that a GPU read timed drew. */
function viewText(v: ViewSection, gpuPx: f64): string {
  const pivot = Number.isNaN(v.pivot_lat)
    ? "-"
    : `${fixed(v.pivot_lat, 5)}°, ${fixed(v.pivot_lon, 5)}° ${n1(v.pivot_alt)} m`;
  return `  view    pivot ${pivot}  distance ${n1(v.dist)} u  polar ${n1(
    deg(v.polar),
  )}°  azimuth ${n1(deg(v.azimuth))}°   drawn ${
    gpuPx > 0 ? valueText("gpu_px", gpuPx) : "-"
  }`;
}

/** Each frame figure's most or least since the recording started (perf.json
 * STAT.recording). */
function recordingText(r: RecordingSection): string {
  if (!r.frames) return "  since   no frame folded";
  return `  since   heap max live ${bytes(r.heap_live_max_bytes)}  run ${bytes(
    r.heap_run_max_bytes,
  )}  slot ${bytes(r.heap_slot_max_bytes)}  range ${bytes(
    r.heap_range_max_bytes,
  )}   largest free min ${bytes(
    r.heap_largest_free_min_bytes,
  )}   refusals ${r.heap_refusals}   ring peak cq ${r.ring_cq_peak} sq ${
    r.ring_sq_peak
  } left ${r.ring_left_peak}   body slots peak ${
    r.body_slots_peak
  }   terrain refused ${r.cap_terrain_refused}   (over the ${
    r.frames
  } frames since the recording started)`;
}

export function failText(fail: u32, detail: u32): string {
  const name = strerror(fail as i32).split(":")[0];
  return detail === 0 ? name : `${name} ${detail}`;
}

/** What each task stream waits on, judged by hand from profiling. It only
 * labels the report: scheduling never reads it. */
export const BOUND: Record<string, string> = {
  terrain: "network",
  division: "network",
};

const phasesText = (p: [string, f64][]) =>
  p.length === 0 ? "" : `  [${p.map(phaseText).join(" ")}]`;

export function taskLine(r: TaskRecord): string {
  const total = r.end_ms - r.enqueue_ms;
  const timing =
    r.start_ms === null
      ? ""
      : `  q ${n1(r.start_ms - r.enqueue_ms)} + run ${n1(r.end_ms - r.start_ms)}`;
  const why =
    r.fail === 0
      ? ""
      : `  [${failText(r.fail, r.detail)}${r.at === null ? "" : ` at ${r.at}`}]`;
  return `[${n1(r.enqueue_ms).padStart(9)}] ${`${r.stream}:${r.path}`.padEnd(
    34,
  )}${r.outcome.padEnd(7)}${n1(total).padStart(8)}ms${timing} p${r.priority}${phasesText(
    r.phases,
  )}${why}`;
}

const worstEffectText = (f: Frame) => {
  const k = f.effects.reduce((w, ms, i) => (ms > f.effects[w] ? i : w), 0);
  return f.effects_ms > 0
    ? `  worst ${EFFECT_NAMES[k] ?? "-"} ${n1(f.effects[k])}`
    : "";
};

/** For one frame, the uploads and the heap mean something only as a change
 * from the frame before, so the line adds their deltas (`Δ`). */
export function frameLine({ sample: s, delta }: Worst): string {
  const d =
    delta === null
      ? ""
      : `  Δuploads ${signed(delta.uploads)} Δuploads bytes ${signedBytes(
          delta.uploads_bytes,
        )} Δheap used ${signedBytes(delta.heap_used_bytes)}`;
  return `[${n1(s.t).padStart(9)}] ${valueText(
    "kind_name",
    s.kind_name,
  )}  interval ${n1(s.interval_ms)} = ${s.turns} turns ${n1(s.busy_ms)} + ${pastText(
    s,
  )}  effects ${n1(s.effects_ms)}${worstEffectText(s)}  tick ${n1(s.tick_ms)}/${n1(
    s.budget_ms,
  )}${s.settled ? " (still)" : ""}  stage ${n1(s.stage_ms)}  tasks ${n1(
    s.tasks_ms,
  )}  perf ${n1(s.perf_ms)}  gpu busy ${
    s.gpu_read ? n1(s.gpu_busy_ms) : "-"
  }  draws ${s.draws} tris ${tris(s.triangles)}  uploads ${s.uploads}  heap used ${bytes(
    s.heap_used_bytes,
  )}${d}`;
}

/** A note for a platform that does not report long animation frames (LoAF):
 * its long frames are then long tasks only, or none. */
function longSourcesText(): string {
  const sources = long_frame.sources();
  if (sources.includes("loaf")) return "";
  return sources.includes("longtask")
    ? " (LoAF unsupported; longtask only)"
    : " (none reported here)";
}

export const longFrameLine = (l: LongFrame) =>
  `[${n1(l.t).padStart(9)}] ${l.source.padEnd(8)} ${n1(
    l.durationMs,
  )} ms  blocking ${n1(l.blockingMs)}  script ${n1(l.scriptMs)}  render ${n1(
    l.renderMs,
  )}`;

function windowSection(r: PerfReport): string[] {
  const f = r.frames;
  if (f === null) return ["frames n=0"];
  const last = f.last;
  const spans = f.gpu_spans
    .filter((s) => s.n > 0)
    .map((s) => `${s.name} ${n1(s.p95)} (${s.n})`)
    .join("  ");
  const steps =
    r.steps.length === 0
      ? "none"
      : [...r.steps]
          .sort((a, b) => b.ms / b.cnt - a.ms / a.cnt)
          .map(
            (s) => `${s.phase}/${s.name} ${n1(s.ms / s.cnt)}/${n1(s.max_ms)}`,
          )
          .join("  ");

  const counted = (rows: { name: string; n: f64 }[]) =>
    rows
      .filter((r) => r.n > 0)
      .map((r) => `${r.name} ${r.n}`)
      .join("  ") || "-";

  return [
    `frames n=${f.n}  over ${n1(f.seconds)}s  fps ${n1(
      fps(f) ?? NaN,
    )}  moving ${f.moving}  still ${f.n - f.moving}   refresh ${n1(
      last.refresh_ms,
    )} ms   paced ${n1(last.paced_ms)} ms   rate ${n1(
      1000 / last.paced_ms,
    )} Hz   scale ${fixed(last.scale, 2)}   turns ${f.turns}`,
    `  kinds   ${counted(f.kinds)}   wakes ${counted(f.wakes)}`,
    `  interval ${percentiles(f.interval)} ms   missed ${f.missed} (${pct(
      f.missed,
      f.moving,
    )}% of moving frames)`,
    `  busy    ${percentiles(f.busy)} ms   (the interval's turns, rise to fall)`,
    `  long    ${f.long} (turns past their interval)`,
    `  tick    ${percentiles(f.tick)} ms   overran ${f.overran}   budget ${n1(
      last.budget_ms,
    )}${last.settled ? " (still)" : ""}   phase p95 ${f.phases
      .map((p) => `${p.name} ${n1(p.p95)}`)
      .join("  ")}`,
    `  effects ${percentiles(f.effects)} ms   p95 ${
      f.effects_p95
        .filter((e) => e.p95 > 0)
        .map((e) => `${e.name} ${n1(e.p95)}`)
        .join("  ") || "-"
    }`,
    `  stage   ${percentiles(f.stage)} ms   (the pump and the render)`,
    `  tasks   ${percentiles(f.tasks)} ms`,
    `  perf ${percentiles(f.perf)} ms   (its own record and the UI's reads)`,
    `  rest    ${percentiles(
      f.rest,
    )} ms   (busy unmeasured: the door's commands, rise and fall)`,
    f.gpu_read > 0
      ? `  gpu     ${percentiles(
          f.gpu_busy,
        )} ms busy   (over the ${f.gpu_read} frames a read timed)`
      : "  gpu     no timestamps: no read landed (no renderer yet, or the device keeps none)",
    `  busy by kind  ${
      f.gpu_busy_kinds
        .filter((r) => r.n > 0)
        .map((r) => `${r.name} p50 ${n1(r.p50)} p95 ${n1(r.p95)} (${r.n})`)
        .join("  ") || "-"
    } ms   (n: the kind's frames a read timed)`,
    `  spans   p95 ${spans || "-"} ms   (n: the frames a read timed that ran each)`,
    ...raysText(f.rays),
    viewText(r.view, f.gpu_px),
    // Steps are listed one by one, because a cooperative scheduler is only as
    // cooperative as its longest step.
    `  steps   ${steps}   (ms a run and max, since the recording started)`,
    `  heap used Δ ${signedBytes(
      f.heap_used_delta_bytes,
    )}   (the high-water mark)`,
    recordingText(r.recording),
  ];
}

function tasksSection(tasks: TaskSummary[]): string[] {
  const n = tasks.reduce((sum, t) => sum + t.n, 0);
  if (n === 0) return ["tasks n=0"];
  const lines = [`tasks n=${n}`];
  for (const t of tasks) {
    const bound = BOUND[t.stream] ?? "?";
    const reasons =
      t.reasons.length === 0
        ? ""
        : ` (${t.reasons
            .map((x) => `${failText(x.fail, x.detail)} x${x.n}`)
            .join(", ")})`;
    lines.push(
      `  ${t.stream.padEnd(9)}${bound.padEnd(9)}n=${t.n}   p50 ${n1(
        t.lat.p50,
      )}  p95 ${n1(
        t.lat.p95,
      )} ms   ok ${t.ok}  ab ${t.aborted}  err ${t.err}${reasons}`,
    );
    if (t.phase_mean !== null) {
      const means = t.phase_mean.map(phaseText).join("  ");
      lines.push(`  ${"".padEnd(18)}mean ${means}`);
    }
  }
  return lines;
}

export function perfReportText(r: PerfReport): string {
  const out = [
    `=== perf @ t=${n1(
      r.t,
    )}ms  build ${r.build.number} ${r.build.id} (${r.build.time}) ===`,
  ];
  out.push(...windowSection(r));
  out.push(...tasksSection(r.tasks));
  out.push(`long frames n=${r.longs.length}${longSourcesText()}`);

  if (r.recent.length > 0) {
    out.push(`--- last ${r.recent.length} tasks ---`);
    for (const rec of r.recent) out.push(taskLine(rec));
  }
  if (r.worst.length > 0) {
    out.push(`--- worst ${r.worst.length} moving frames since recording ---`);
    for (const w of r.worst) out.push(frameLine(w));
  }
  if (r.longs.length > 0) {
    out.push("--- long frames ---");
    for (const l of r.longs) out.push(longFrameLine(l));
  }
  return out.join("\n");
}

export async function perfReportCopy(text: string): Promise<string> {
  console.log(text);
  const kb = `${Math.max(1, round(text.length / 1024))} KiB`;
  let said: string;
  try {
    await share.copy(text);
    said = `report copied · ${kb}`;
  } catch {
    said = `report printed to the console · ${kb} (the clipboard refused it)`;
  }
  console.log(`[perf] ${said}`);
  return said;
}
