// @pxl/client/shell/debug/profiler/report.ts

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { type Errno, strerror } from "@xpute/kit/status/errno.spec.ts";

import type { Frame, Spread, TaskRecord, TaskSummary } from "@/abi/profiler.ts";
import { drawn, gpuMs, n1, pct, phaseText, tris, valueText } from "./lane.ts";
import { fps, type ProfilerReport } from "./view.ts";
import { fixed } from "@/shell/fixed.ts";
import { bracket, type LongFrame } from "@/bracket.tsx";

const signed = (n: number) => (n >= 0 ? `+${n}` : String(n));
/** A count of bytes in the one unit the figures are shown in, signed. */
const signedBytes = (b: number) =>
  `${b >= 0 ? "+" : ""}${valueText("_bytes", b)}`;
const bytes = (b: number) => valueText("_bytes", b);
/** Where the interval's turns ended: before it, by the time left, or past it,
 * by how much (the kernel's profiler/frame.rs `over_ms`). */
const pastText = (
  f: Frame,
) => (f.busy_ms > f.interval_ms
  ? `over by ${n1(f.busy_ms - f.interval_ms)}`
  : `off-turn ${n1(f.interval_ms - f.busy_ms)}`);
const spread = (s: Spread) =>
  `p50 ${n1(s.p50)}  p95 ${n1(s.p95)}  max ${n1(s.max)}`;
/** The `ray_counts` setting's sums since boot as the last frame holds them
 * (profiler.json FRAME's rays, eye and sun lanes), where any frame's counts
 * were read back: an observer's line each where it cast — the nodes its
 * beam rays, its samples and its walk again asked a cast, its pixels or
 * slots that took the raster's pick and that walked, the nodes its walk
 * again asked a walking one, and the boxes it drew a cast. */
const raysText = (f: Frame) => {
  if (f.rays_frames === 0) return [];
  const observer = (
    name: string,
    casts: number,
    rays: number,
    o: {
      beam: number;
      sample: number;
      miss: number;
      picked: number;
      walked: number;
      boxes: number;
    },
  ) => {
    if (casts === 0) return [];
    const perCast = (n: number) => n1(n / casts);
    const share = (n: number) => `${fixed((100 * n) / Math.max(1, rays), 2)}%`;
    return [
      `  ${
        name.padEnd(7)
      } cast in ${casts} of ${f.rays_frames} frames   nodes a cast: beam ${
        perCast(o.beam)
      }  sample ${perCast(o.sample)}  miss ${
        perCast(o.miss)
      }   picked ${share(o.picked)}  walked ${share(o.walked)} (${
        n1(o.miss / Math.max(1, o.walked))
      } nodes each)   boxes ${perCast(o.boxes)} a cast`,
    ];
  };
  return [
    ...observer("eye", f.eye_casts, f.eye_rays, {
      beam: f.eye_beam,
      sample: f.eye_sample,
      miss: f.eye_miss,
      picked: f.eye_picked,
      walked: f.eye_walked,
      boxes: f.eye_boxes,
    }),
    ...observer("sun", f.sun_casts, f.sun_rays, {
      beam: f.sun_beam,
      sample: f.sun_sample,
      miss: f.sun_miss,
      picked: f.sun_picked,
      walked: f.sun_walked,
      boxes: f.sun_boxes,
    }),
  ];
};

export function failText(fail: u32, detail: u32): string {
  const name = strerror(fail as Errno).split(":")[0];
  return detail === 0 ? name : `${name} ${detail}`;
}

/** Set once from profiling, never fed back into scheduling. */
export const BOUND: Record<string, string> = {
  terrain: "network",
  division: "network",
};

const phasesText = (p: [string, f64][]) =>
  p.length === 0 ? "" : `  [${p.map(phaseText).join(" ")}]`;

export function taskLine(r: TaskRecord): string {
  const total = r.end_ms - r.enqueue_ms;
  const timing = r.start_ms === null
    ? ""
    : `  q ${n1(r.start_ms - r.enqueue_ms)} + run ${n1(r.end_ms - r.start_ms)}`;
  const why = r.fail === 0
    ? ""
    : `  [${failText(r.fail, r.detail)}${r.at === null ? "" : ` at ${r.at}`}]`;
  return `[${n1(r.enqueue_ms).padStart(9)}] ${
    `${r.stream}:${r.subject}`.padEnd(34)
  }${r.outcome.padEnd(7)}${n1(total).padStart(8)}ms${timing} p${r.priority}${
    phasesText(r.phases)
  }${why}`;
}

/** Geometry, textures and heap only mean anything as deltas from `prev`. */
export function frameLine(s: Frame, prev: Frame | null): string {
  const [draws, triangles] = drawn(s);
  const d = prev === null
    ? ""
    : `  Δuploads ${signed(s.uploads - prev.uploads)} Δuploads bytes ${
      signedBytes(s.uploads_bytes - prev.uploads_bytes)
    } Δheap used ${signedBytes(s.heap_used_bytes - prev.heap_used_bytes)}`;
  return `[${n1(s.t).padStart(9)}] ${
    valueText("kind_name", s.kind_name)
  }  interval ${n1(s.interval_ms)} = ${s.turns} turns ${n1(s.busy_ms)} + ${
    pastText(s)
  }  tick ${n1(s.tick_ms)}/${n1(s.budget_ms)}${
    s.settled ? " (still)" : ""
  }  gpu ${s.gpu_read ? n1(gpuMs(s)) : "-"}  draws ${draws} tris ${
    tris(triangles)
  }  uploads ${s.uploads}  heap used ${bytes(s.heap_used_bytes)}${d}`;
}

/** What the place hears its long frames by, where that is less than both sources. */
function longSourcesText(): string {
  const sources = bracket.longFrame.sources();
  if (sources.includes("loaf")) return "";
  return sources.includes("longtask")
    ? " (LoAF unsupported; longtask only)"
    : " (none reported here)";
}

export const longFrameLine = (l: LongFrame) =>
  `[${n1(l.t).padStart(9)}] ${l.source.padEnd(8)} ${
    n1(l.durationMs)
  } ms  blocking ${n1(l.blockingMs)}  script ${n1(l.scriptMs)}  render ${
    n1(l.renderMs)
  }`;

function windowSection(r: ProfilerReport): string[] {
  const f = r.frames;
  if (f === null) return ["frames n=0"];
  const last = f.last;
  const spans = f.gpu_spans.filter((s) => s.p95 > 0).map((s) =>
    `${s.name} ${n1(s.p95)}`
  ).join("  ");
  const steps = r.steps.length === 0
    ? "none"
    : [...r.steps].sort((a, b) => b.ms / b.count - a.ms / a.count).map((s) =>
      `${s.phase}/${s.name} ${n1(s.ms / s.count)}/${n1(s.max_ms)}`
    ).join("  ");

  const counted = (rows: { name: string; n: f64 }[]) =>
    rows.filter((r) => r.n > 0).map((r) => `${r.name} ${r.n}`).join("  ") ||
    "-";

  return [
    `frames n=${f.n}  over ${n1(f.seconds)}s  fps ${
      n1(fps(f) ?? NaN)
    }  moving ${f.moving}  still ${f.n - f.moving}   refresh ${
      n1(last.refresh_ms)
    } ms   paced ${n1(last.paced_ms)} ms   scale ${
      fixed(last.scale, 2)
    }   turns ${f.turns}`,
    `  kinds   ${counted(f.kinds)}   wakes ${counted(f.wakes)}`,
    `  interval ${spread(f.interval)} ms   missed ${f.missed} (${
      pct(f.missed, f.moving)
    }% of moving frames)`,
    `  busy    ${spread(f.busy)} ms   (the interval's turns, rise to fall)`,
    `  long    ${f.long} (turns past their interval)`,
    `  tick    ${spread(f.tick)} ms   overran ${f.overran}   budget ${
      n1(last.budget_ms)
    }${last.settled ? " (still)" : ""}   phase p95 ${
      f.phases.map((p) => `${p.name} ${n1(p.p95)}`).join("  ")
    }`,
    f.gpu_read > 0
      ? `  gpu     ${
        spread(f.gpu)
      } ms   (over the ${f.gpu_read} frames a read landed with)`
      : "  gpu     no timestamps: no read landed (no renderer yet, or the device keeps none)",
    `  spans   p95 ${spans || "-"} ms   (over the frames that ran each)`,
    ...raysText(last),
    // Per step: a cooperative scheduler is only as cooperative as its longest step.
    `  steps   ${steps}   (ms per frame, max)`,
    `  heap used Δ ${
      signedBytes(f.heap_used_delta_bytes)
    }   (the high-water mark)`,
  ];
}

function tasksSection(tasks: TaskSummary[]): string[] {
  const n = tasks.reduce((sum, t) => sum + t.n, 0);
  if (n === 0) return ["tasks n=0"];
  const lines = [`tasks n=${n}`];
  for (const t of tasks) {
    const bound = BOUND[t.stream] ?? "?";
    const reasons = t.reasons.length === 0
      ? ""
      : ` (${
        t.reasons.map((x) => `${failText(x.fail, x.detail)} x${x.n}`).join(", ")
      })`;
    lines.push(
      `  ${t.stream.padEnd(9)}${bound.padEnd(9)}n=${t.n}   p50 ${
        n1(t.lat.p50)
      }  p95 ${
        n1(t.lat.p95)
      } ms   ok ${t.ok}  ab ${t.aborted}  err ${t.error}${reasons}`,
    );
    if (t.phase_mean !== null) {
      const means = t.phase_mean.map(phaseText).join("  ");
      lines.push(`  ${"".padEnd(18)}mean ${means}`);
    }
  }
  return lines;
}

export function profilerReportText(r: ProfilerReport): string {
  const out = [
    `=== profiler @ t=${
      n1(r.t)
    }ms  build ${r.build.number} ${r.build.id} (${r.build.time}) ===`,
  ];
  out.push(...windowSection(r));
  out.push(...tasksSection(r.tasks));
  out.push(`long frames n=${r.longs.length}${longSourcesText()}`);

  if (r.recent.length > 0) {
    out.push(`--- last ${r.recent.length} tasks ---`);
    for (const rec of r.recent) out.push(taskLine(rec));
  }
  if (r.worst.length > 0) {
    out.push(`--- worst ${r.worst.length} frames since recording ---`);
    for (const w of r.worst) out.push(frameLine(w.sample, w.prev));
  }
  if (r.longs.length > 0) {
    out.push("--- long frames ---");
    for (const l of r.longs) out.push(longFrameLine(l));
  }
  return out.join("\n");
}

export async function profilerReportCopy(text: string): Promise<string> {
  console.log(text);
  const kb = `${Math.max(1, round(text.length / 1024))} KiB`;
  let said: string;
  try {
    await bracket.share.copy(text);
    said = `report copied · ${kb}`;
  } catch {
    said = `report printed to the console · ${kb} (the clipboard refused it)`;
  }
  console.log(`[profiler] ${said}`);
  return said;
}
