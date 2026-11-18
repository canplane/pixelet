// @pxl/client/ui/feature/debug/perf/report.ts

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { strerror } from "@xpute/kit/status/error.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";

import type {
  Frame,
  ListColumn,
  Percentile,
  RayCount,
  RecordingSection,
  TaskRecord,
  TaskSummary,
  ViewSection,
  Worst,
} from "@/service/core/perf.ts";
import {
  bitsOf,
  effectsMs,
  LIST_NAMES,
  n1,
  phaseName,
  phaseText,
  stageMs,
  tris,
  valueText,
} from "./column.ts";
import { fps, type PerfReport } from "./view.ts";
import { fixed } from "@/ui/shared/fixed.ts";
import { EFFECT_NAMES } from "@/abi/step.spec.ts";
import { LIGHT_CHANGEDS } from "@/abi/span.spec.ts";
import { type LongFrame } from "@/service/platform/types.ts";
import * as longFrame from "@/service/platform/long-frame.ts";
import * as share from "@/service/platform/share.ts";

const signed = (n: number) => (n >= 0 ? `+${n}` : String(n));
const signedBytes = (b: number) =>
  `${b >= 0 ? "+" : ""}${valueText("_bytes", b)}`;
const bytes = (b: number) => valueText("_bytes", b);
const percentile = (s: Percentile) =>
  `p50 ${n1(s.p50)}  p95 ${n1(s.p95)}  max ${n1(s.max)}`;
type RayColumn<K = keyof RayCount> = K extends `eye_${infer C}` ? C : never;

/** The counts are the ones the `ray_cnt` setting collects, which the core
 * sums over the window's frames. Each observer, the eye and the light, gets one
 * line when it cast in the window. `beam`, `sample` and `hole_walk` are the
 * nodes that its beam rays, its samples and its holes' walks asked per cast,
 * and `ask_cnt` is how many of the walks' nodes were asked of the world. `voxel`,
 * `hole` and `sky` count the pixels or map slots cast again that took the
 * visible voxel, walked on their own, or took the sky, each shown as a share
 * of all its pixels or slots (`ray_cnt`). */
const raysText = (rays: RayCount) => {
  const observer = (name: "eye" | "light") => {
    const at = (column: RayColumn) => rays[`${name}_${column}`];
    const casts = at("cast");
    if (casts === 0) return [];
    const perCast = (n: number) => n1(n / casts);
    const rayCount = at("ray_cnt");
    const part = (n: number) =>
      rayCount > 0
        ? `${fixed((100 * n) / rayCount, 2)}%`
        : `${perCast(n)} a cast`;
    const again = at("voxel") + at("hole") + at("sky");
    return [
      `  ${name.padEnd(7)} cast in ${casts} frames   nodes a cast: beam ${perCast(
        at("beam"),
      )}  sample ${perCast(at("sample"))}  hole walk ${perCast(
        at("hole_walk"),
      )} (${perCast(at("ask_cnt"))} asked of the world)   cast again ${part(again)}: voxel ${part(at("voxel"))}  hole ${part(
        at("hole"),
      )} (${n1(
        at("hole_walk") / Math.max(1, at("hole")),
      )} nodes each)  sky ${part(at("sky"))}   boxes ${perCast(at("box_cnt"))} a cast`,
    ];
  };
  return [...observer("eye"), ...observer("light")];
};

const degrees = (radians: number) => (radians * 180) / Math.PI;

function viewText(v: ViewSection, gpuPx: f64): string {
  const pivot = Number.isNaN(v.pivot_lat)
    ? "-"
    : `${fixed(v.pivot_lat, 5)}°, ${fixed(v.pivot_lon, 5)}° ${n1(v.pivot_alt)} m`;
  return `  view    pivot ${pivot}  distance ${n1(v.dist)} u  polar ${n1(
    degrees(v.polar),
  )}°  azimuth ${n1(degrees(v.azimuth))}°   drawn ${
    gpuPx > 0 ? valueText("gpu_px", gpuPx) : "-"
  }`;
}

const counted = (rows: { name: string; n: f64 }[]) =>
  rows
    .filter((r) => r.n > 0)
    .map((r) => `${r.name} ${r.n}`)
    .join("  ") || "-";

function recordingText(recording: RecordingSection): string {
  if (!recording.frame_cnt) return "  since   no frame folded";
  return `  since   heap max live ${bytes(recording.heap_live_max_bytes)}  run ${bytes(
    recording.heap_run_max_bytes,
  )}  slot ${bytes(recording.heap_slot_max_bytes)}  range ${bytes(
    recording.heap_range_max_bytes,
  )}   largest free min ${bytes(
    recording.heap_largest_free_min_bytes,
  )}   refusals ${recording.heap_refusal_cnt}   body slots peak ${
    recording.body_slots_peak
  }   tiles refused ${counted(recording.refusals)}   (over the ${
    recording.frame_cnt
  } frames since the recording started)`;
}

export function failText(fail: u32, detail: u32): string {
  const name = strerror(fail as i32).split(":")[0];
  return detail === 0 ? name : `${name} ${detail}`;
}

/** What a stream's runs waited on over the window: the phase its runs' mean
 * time is longest in, or none where they reported no time. */
const boundText = (means: [string, f64][] | null) => {
  const timed = (means ?? []).filter(([k]) => k.endsWith("_ms"));
  return timed.length === 0
    ? "-"
    : phaseName(timed.reduce((a, b) => (b[1] > a[1] ? b : a))[0]);
};

const phasesText = (p: [string, f64][]) =>
  p.length === 0 ? "" : `  [${p.map(phaseText).join(" ")}]`;

function taskLine(task: TaskRecord): string {
  const total = task.endMs - task.enqueueMs;
  const timing =
    task.startMs === null
      ? ""
      : `  q ${n1(task.startMs - task.enqueueMs)} + run ${n1(task.endMs - task.startMs)}`;
  const why =
    task.fail === 0
      ? ""
      : `  [${failText(task.fail, task.detail)}${task.at === null ? "" : ` at ${task.at}`}]`;
  return `[${n1(task.enqueueMs).padStart(9)}] ${`${task.stream}:${task.path}`.padEnd(
    34,
  )}${task.outcome.padEnd(7)}${n1(total).padStart(8)}ms${timing} p${task.priority}${phasesText(
    task.phases,
  )}${why}`;
}

const worstEffectText = (f: Frame) => {
  const k = f.effects.reduce((w, ms, i) => (ms > f.effects[w] ? i : w), 0);
  return effectsMs(f) > 0
    ? `  worst ${EFFECT_NAMES[k] ?? "-"} ${n1(f.effects[k])}`
    : "";
};

/** A list column's slots that are not 0, each by its name. */
const listText = (
  f: Frame,
  column: ListColumn,
  text: (v: f64) => string = String,
) =>
  f[column]
    .map((v, k) => (v > 0 ? `${LIST_NAMES[column][k]} ${text(v)}` : ""))
    .filter(Boolean)
    .join(" ") || "-";

/** For one frame, the uploads and the heap mean something only as a change
 * from the frame before, so the line adds their deltas (`Δ`). */
function frameLine({ sample, delta }: Worst): string {
  const deltaText =
    delta === null
      ? ""
      : `  Δuploads ${signed(delta.uploadCount)} Δuploads bytes ${signedBytes(
          delta.uploadBytes,
        )} Δheap used ${signedBytes(delta.heapUsedBytes)}`;
  return `[${n1(sample.t).padStart(9)}] frame ${sample.gpu_frame} ${valueText(
    "kind_name",
    sample.kind_name,
  )}  interval ${n1(sample.interval_ms)}  asked by ${listText(sample, "invalidations")}  effects ${n1(effectsMs(sample))}${worstEffectText(sample)}  tick ${n1(sample.tick_ms)}${sample.overran ? " overran" : ""}  deadline ${n1(
    sample.budget_ms,
  )} from the entry's start${sample.settled ? " (still)" : ""}  pump ${n1(sample.pump_ms)}  stage ${n1(stageMs(sample))} (${listText(sample, "stage_parts", n1)})  fill ${n1(sample.fill_ms)} (${sample.fill_cnt} bricks)  perf ${n1(sample.perf_ms)}  gpu busy ${
    sample.gpu_timed ? n1(sample.gpu_busy_ms) : "-"
  }  light cast by ${
    bitsOf(LIGHT_CHANGEDS, sample.light_changed).join(" ") || "-"
  }  draws ${sample.draw_cnt} tris ${tris(sample.triangle_cnt)}  tiles ${listText(sample, "loads")}  uploads ${sample.upload_cnt} (${bytes(
    sample.upload_bytes,
  )})  heap used ${bytes(sample.heap_used_bytes)}${deltaText}`;
}

function longSourcesText(): string {
  const sources = longFrame.sources();
  if (sources.includes("loaf")) return "";
  return sources.includes("longtask")
    ? " (LoAF unsupported; longtask only)"
    : " (none reported here)";
}

const longFrameLine = (l: LongFrame) =>
  `[${n1(l.t).padStart(9)}] ${l.source.padEnd(8)} ${n1(
    l.durationMs,
  )} ms  blocking ${n1(l.blockingMs)}  script ${n1(l.scriptMs)}  render ${n1(
    l.renderMs,
  )}`;

function windowSection(report: PerfReport): string[] {
  const frames = report.frames;
  if (frames === null) return ["frames n=0"];
  const last = frames.last;
  const spans = frames.gpuSpans
    .filter((s) => s.n > 0)
    .map((s) => `${s.name} ${n1(s.p95)} (${s.n})`)
    .join("  ");
  const steps =
    report.steps.length === 0
      ? "none"
      : [...report.steps]
          .sort((a, b) => b.ms / b.count - a.ms / a.count)
          .map(
            (s) => `${s.phase}/${s.name} ${n1(s.ms / s.count)}/${n1(s.maxMs)}`,
          )
          .join("  ");

  return [
    `frames n=${frames.n}  over ${n1(frames.seconds)}s  fps ${n1(
      fps(frames) ?? NaN,
    )}  moving ${frames.moving}  still ${frames.n - frames.moving}   refresh ${n1(
      last.refresh_ms,
    )} ms   paced ${n1(last.paced_ms)} ms   scale ${fixed(last.scale, 2)}`,
    `  kinds   ${counted(frames.kinds)}`,
    `  interval ${percentile(frames.interval)} ms`,
    `  tick    ${percentile(frames.tick)} ms   overran ${frames.overran}   budget ${n1(
      last.budget_ms,
    )}${last.settled ? " (still)" : ""}   phase p95 ${frames.phases
      .map((p) => `${p.name} ${n1(p.p95)}`)
      .join("  ")}`,
    `  effects ${percentile(frames.effect)} ms   p95 ${
      frames.effectsP95
        .filter((e) => e.p95 > 0)
        .map((e) => `${e.name} ${n1(e.p95)}`)
        .join("  ") || "-"
    }`,
    `  invalidations ${frames.invalidations.map((a) => `${a.name} ${a.n}`).join("  ")}`,
    `  pump    ${percentile(frames.pump)} ms   (the requests', after the tick)`,
    `  stage   ${percentile(frames.stage)} ms   p95 ${frames.stagePartsP95
      .map((s) => `${s.name} ${n1(s.p95)}`)
      .join("  ")}`,
    `  fill    ${percentile(frames.fill)} ms   bricks ${frames.fillCount}   (the ground's, after the frame)`,
    `  perf ${percentile(frames.perf)} ms   (its own record and the UI's reads)`,
    frames.gpuTimed > 0
      ? `  gpu     ${percentile(
          frames.gpuBusy,
        )} ms busy   (over the ${frames.gpuTimed} frames a read timed)`
      : "  gpu     no timestamps: no read landed (no renderer yet, or the device keeps none)",
    `  busy by kind  ${
      frames.gpuBusyKinds
        .filter((r) => r.n > 0)
        .map((r) => `${r.name} p50 ${n1(r.p50)} p95 ${n1(r.p95)} (${r.n})`)
        .join("  ") || "-"
    } ms   (n: the kind's frames a read timed)`,
    `  spans   p95 ${spans || "-"} ms   (n: the frames a read timed that ran each)`,
    ...raysText(frames.rayCount),
    viewText(report.view, frames.gpuPx),
    // Steps are listed one by one, because a cooperative scheduler is only as
    // cooperative as its longest step.
    `  steps   ${steps}   (ms a run and max, since the recording started)`,
    `  heap used Δ ${signedBytes(
      frames.heapUsedDeltaBytes,
    )}   (the high-water mark)`,
    recordingText(report.recording),
  ];
}

function tasksSection(tasks: TaskSummary[]): string[] {
  const total = tasks.reduce((sum, t) => sum + t.n, 0);
  if (total === 0) return ["tasks n=0"];
  const lines = [`tasks n=${total}`];
  for (const summary of tasks) {
    const bound = boundText(summary.phaseMean);
    const reasons =
      summary.reasons.length === 0
        ? ""
        : ` (${summary.reasons
            .map((x) => `${failText(x.fail, x.detail)} x${x.n}`)
            .join(", ")})`;
    lines.push(
      `  ${summary.stream.padEnd(9)}${bound.padEnd(9)}n=${summary.n}   p50 ${n1(
        summary.latency.p50,
      )}  p95 ${n1(
        summary.latency.p95,
      )} ms   ok ${summary.ok}  ab ${summary.aborted}  err ${summary.failed}${reasons}`,
    );
    if (summary.phaseMean !== null) {
      const means = summary.phaseMean.map(phaseText).join("  ");
      lines.push(`  ${"".padEnd(18)}mean ${means}`);
    }
  }
  return lines;
}

export function perfReportText(report: PerfReport): string {
  const out = [
    `=== perf @ t=${n1(
      report.t,
    )}ms  build ${report.build.number} ${report.build.id} (${report.build.time}) ===`,
  ];
  if (report.truncated)
    out.push("truncated: the window lost records it had no room for");
  out.push(...windowSection(report));
  out.push(...tasksSection(report.tasks));
  out.push(`long frames n=${report.longs.length}${longSourcesText()}`);

  if (report.recent.length > 0) {
    out.push(`--- last ${report.recent.length} tasks ---`);
    for (const task of report.recent) out.push(taskLine(task));
  }
  if (report.worst.length > 0) {
    out.push(
      `--- worst ${report.worst.length} moving frames since recording ---`,
    );
    for (const worst of report.worst) out.push(frameLine(worst));
  }
  if (report.longs.length > 0) {
    out.push("--- long frames ---");
    for (const long of report.longs) out.push(longFrameLine(long));
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
