// @pxl/client/ui/feature/debug/perf/report.ts

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { strerror } from "@xpute/kit/status/error.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";

import type {
  Frame,
  Percentile,
  RayCount,
  RecordingSection,
  TaskRecord,
  TaskSummary,
  ViewSection,
  Worst,
} from "@/service/core/perf.ts";
import { n1, phase_text, tris, value_text } from "./column.ts";
import { fps, type PerfReport } from "./view.ts";
import { fixed } from "@/ui/shared/fixed.ts";
import { EFFECT_NAMES } from "@/abi/step.spec.ts";
import { type LongFrame } from "@/service/platform/types.ts";
import * as long_frame from "@/service/platform/long-frame.ts";
import * as share from "@/service/platform/share.ts";

const signed = (n: number) => (n >= 0 ? `+${n}` : String(n));
const signed_bytes = (b: number) =>
  `${b >= 0 ? "+" : ""}${value_text("_bytes", b)}`;
const bytes = (b: number) => value_text("_bytes", b);
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
const rays_text = (f: RayCount) => {
  const observer = (name: "eye" | "light") => {
    const at = (column: RayColumn) => f[`${name}_${column}`];
    const casts = at("cast");
    if (casts === 0) return [];
    const per_cast = (n: number) => n1(n / casts);
    const ray_count = at("ray_cnt");
    const part = (n: number) =>
      ray_count > 0
        ? `${fixed((100 * n) / ray_count, 2)}%`
        : `${per_cast(n)} a cast`;
    const again = at("voxel") + at("hole") + at("sky");
    return [
      `  ${name.padEnd(7)} cast in ${casts} frames   nodes a cast: beam ${per_cast(
        at("beam"),
      )}  sample ${per_cast(at("sample"))}  hole walk ${per_cast(
        at("hole_walk"),
      )} (${per_cast(at("ask_cnt"))} asked of the world)   cast again ${part(again)}: voxel ${part(at("voxel"))}  hole ${part(
        at("hole"),
      )} (${n1(
        at("hole_walk") / Math.max(1, at("hole")),
      )} nodes each)  sky ${part(at("sky"))}   boxes ${per_cast(at("box_cnt"))} a cast`,
    ];
  };
  return [...observer("eye"), ...observer("light")];
};

const deg = (rad: number) => (rad * 180) / Math.PI;

function view_text(v: ViewSection, gpu_px: f64): string {
  const pivot = Number.isNaN(v.pivot_lat)
    ? "-"
    : `${fixed(v.pivot_lat, 5)}°, ${fixed(v.pivot_lon, 5)}° ${n1(v.pivot_alt)} m`;
  return `  view    pivot ${pivot}  distance ${n1(v.dist)} u  polar ${n1(
    deg(v.polar),
  )}°  azimuth ${n1(deg(v.azimuth))}°   drawn ${
    gpu_px > 0 ? value_text("gpu_px", gpu_px) : "-"
  }`;
}

function recording_text(r: RecordingSection): string {
  if (!r.frame_cnt) return "  since   no frame folded";
  return `  since   heap max live ${bytes(r.heap_live_max_bytes)}  run ${bytes(
    r.heap_run_max_bytes,
  )}  slot ${bytes(r.heap_slot_max_bytes)}  range ${bytes(
    r.heap_range_max_bytes,
  )}   largest free min ${bytes(
    r.heap_largest_free_min_bytes,
  )}   refusals ${r.heap_refusal_cnt}   body slots peak ${
    r.body_slots_peak
  }   terrain refused ${r.terrain_refusal_cnt}   (over the ${
    r.frame_cnt
  } frames since the recording started)`;
}

export function fail_text(fail: u32, detail: u32): string {
  const name = strerror(fail as i32).split(":")[0];
  return detail === 0 ? name : `${name} ${detail}`;
}

/** What each task stream waits on, judged by hand from profiling. It only
 * labels the report: scheduling never reads it. */
const BOUND: Record<string, string> = {
  terrain: "network",
  division: "network",
};

const phases_text = (p: [string, f64][]) =>
  p.length === 0 ? "" : `  [${p.map(phase_text).join(" ")}]`;

function task_line(r: TaskRecord): string {
  const total = r.end_ms - r.enqueue_ms;
  const timing =
    r.start_ms === null
      ? ""
      : `  q ${n1(r.start_ms - r.enqueue_ms)} + run ${n1(r.end_ms - r.start_ms)}`;
  const why =
    r.fail === 0
      ? ""
      : `  [${fail_text(r.fail, r.detail)}${r.at === null ? "" : ` at ${r.at}`}]`;
  return `[${n1(r.enqueue_ms).padStart(9)}] ${`${r.stream}:${r.path}`.padEnd(
    34,
  )}${r.outcome.padEnd(7)}${n1(total).padStart(8)}ms${timing} p${r.priority}${phases_text(
    r.phases,
  )}${why}`;
}

const worst_effect_text = (f: Frame) => {
  const k = f.effects.reduce((w, ms, i) => (ms > f.effects[w] ? i : w), 0);
  return f.effects_ms > 0
    ? `  worst ${EFFECT_NAMES[k] ?? "-"} ${n1(f.effects[k])}`
    : "";
};

/** For one frame, the uploads and the heap mean something only as a change
 * from the frame before, so the line adds their deltas (`Δ`). */
function frame_line({ sample: s, delta }: Worst): string {
  const d =
    delta === null
      ? ""
      : `  Δuploads ${signed(delta.upload_cnt)} Δuploads bytes ${signed_bytes(
          delta.upload_bytes,
        )} Δheap used ${signed_bytes(delta.heap_used_bytes)}`;
  return `[${n1(s.t).padStart(9)}] ${value_text(
    "kind_name",
    s.kind_name,
  )}  interval ${n1(s.interval_ms)}  effects ${n1(s.effects_ms)}${worst_effect_text(s)}  tick ${n1(s.tick_ms)}/${n1(
    s.budget_ms,
  )}${s.settled ? " (still)" : ""}  stage ${n1(s.stage_ms)}  perf ${n1(s.perf_ms)}  gpu busy ${
    s.gpu_timed ? n1(s.gpu_busy_ms) : "-"
  }  draws ${s.draw_cnt} tris ${tris(s.triangle_cnt)}  uploads ${s.upload_cnt}  heap used ${bytes(
    s.heap_used_bytes,
  )}${d}`;
}

function long_sources_text(): string {
  const sources = long_frame.sources();
  if (sources.includes("loaf")) return "";
  return sources.includes("longtask")
    ? " (LoAF unsupported; longtask only)"
    : " (none reported here)";
}

const long_frame_line = (l: LongFrame) =>
  `[${n1(l.t).padStart(9)}] ${l.source.padEnd(8)} ${n1(
    l.duration_ms,
  )} ms  blocking ${n1(l.blocking_ms)}  script ${n1(l.script_ms)}  render ${n1(
    l.render_ms,
  )}`;

function window_section(r: PerfReport): string[] {
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
    )} ms   paced ${n1(last.paced_ms)} ms   scale ${fixed(last.scale, 2)}`,
    `  kinds   ${counted(f.kinds)}`,
    `  interval ${percentile(f.interval)} ms`,
    `  tick    ${percentile(f.tick)} ms   overran ${f.overran}   budget ${n1(
      last.budget_ms,
    )}${last.settled ? " (still)" : ""}   phase p95 ${f.phases
      .map((p) => `${p.name} ${n1(p.p95)}`)
      .join("  ")}`,
    `  effects ${percentile(f.effect)} ms   p95 ${
      f.effects_p95
        .filter((e) => e.p95 > 0)
        .map((e) => `${e.name} ${n1(e.p95)}`)
        .join("  ") || "-"
    }`,
    `  stage   ${percentile(f.stage)} ms   (the pump and the render)`,
    `  perf ${percentile(f.perf)} ms   (its own record and the UI's reads)`,
    f.gpu_timed > 0
      ? `  gpu     ${percentile(
          f.gpu_busy,
        )} ms busy   (over the ${f.gpu_timed} frames a read timed)`
      : "  gpu     no timestamps: no read landed (no renderer yet, or the device keeps none)",
    `  busy by kind  ${
      f.gpu_busy_kinds
        .filter((r) => r.n > 0)
        .map((r) => `${r.name} p50 ${n1(r.p50)} p95 ${n1(r.p95)} (${r.n})`)
        .join("  ") || "-"
    } ms   (n: the kind's frames a read timed)`,
    `  spans   p95 ${spans || "-"} ms   (n: the frames a read timed that ran each)`,
    ...rays_text(f.ray_cnt),
    view_text(r.view, f.gpu_px),
    // Steps are listed one by one, because a cooperative scheduler is only as
    // cooperative as its longest step.
    `  steps   ${steps}   (ms a run and max, since the recording started)`,
    `  heap used Δ ${signed_bytes(
      f.heap_used_delta_bytes,
    )}   (the high-water mark)`,
    recording_text(r.recording),
  ];
}

function tasks_section(tasks: TaskSummary[]): string[] {
  const n = tasks.reduce((sum, t) => sum + t.n, 0);
  if (n === 0) return ["tasks n=0"];
  const lines = [`tasks n=${n}`];
  for (const t of tasks) {
    const bound = BOUND[t.stream] ?? "?";
    const reasons =
      t.reasons.length === 0
        ? ""
        : ` (${t.reasons
            .map((x) => `${fail_text(x.fail, x.detail)} x${x.n}`)
            .join(", ")})`;
    lines.push(
      `  ${t.stream.padEnd(9)}${bound.padEnd(9)}n=${t.n}   p50 ${n1(
        t.lat.p50,
      )}  p95 ${n1(
        t.lat.p95,
      )} ms   ok ${t.ok}  ab ${t.aborted}  err ${t.err}${reasons}`,
    );
    if (t.phase_mean !== null) {
      const means = t.phase_mean.map(phase_text).join("  ");
      lines.push(`  ${"".padEnd(18)}mean ${means}`);
    }
  }
  return lines;
}

export function perf_report_text(r: PerfReport): string {
  const out = [
    `=== perf @ t=${n1(
      r.t,
    )}ms  build ${r.build.number} ${r.build.id} (${r.build.time}) ===`,
  ];
  out.push(...window_section(r));
  out.push(...tasks_section(r.tasks));
  out.push(`long frames n=${r.longs.length}${long_sources_text()}`);

  if (r.recent.length > 0) {
    out.push(`--- last ${r.recent.length} tasks ---`);
    for (const rec of r.recent) out.push(task_line(rec));
  }
  if (r.worst.length > 0) {
    out.push(`--- worst ${r.worst.length} moving frames since recording ---`);
    for (const w of r.worst) out.push(frame_line(w));
  }
  if (r.longs.length > 0) {
    out.push("--- long frames ---");
    for (const l of r.longs) out.push(long_frame_line(l));
  }
  return out.join("\n");
}

export async function perf_report_copy(text: string): Promise<string> {
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
