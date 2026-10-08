// @pxl/web/feature/debug/profiler/report.ts

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { type Errno, strerror } from "@xpute/kit/status/errno.spec.ts";
import { copyText } from "@platform/save.ts";

import { fieldText, type Frame, GROUPS, groupText, n1, offthreadMs, pct, phaseText, STARVED_MS, tris, zoomName } from "./lane.ts";
import { loafSupported, type LongFrame } from "./long-frame.ts";
import type { ProfilerReport, Spread, TaskRecord, TaskSummary } from "./view.ts";

const signed = (n: number) => (n >= 0 ? `+${n}` : String(n));
const spread = (s: Spread) => `p50 ${n1(s.p50)}  p95 ${n1(s.p95)}  max ${n1(s.max)}`;

const starved = (ms: number) => (ms >= STARVED_MS ? "  STARVED" : "");

export function failText(fail: u32, detail: u32): string {
  const name = strerror(fail as Errno).split(":")[0];
  return detail === 0 ? name : `${name} ${detail}`;
}

/** Set once from profiling, never fed back into scheduling. */
export const BOUND: Record<string, string> = { terrain: "network", division: "network" };

const phasesText = (p: [string, f64][]) => p.length === 0 ? "" : `  [${p.map(phaseText).join(" ")}]`;

export function taskLine(r: TaskRecord): string {
  const total = r.end_ms - r.enqueue_ms;
  const timing = r.start_ms === null ? "" : `  q ${n1(r.start_ms - r.enqueue_ms)} + run ${n1(r.end_ms - r.start_ms)}`;
  const why = r.fail === 0 ? "" : `  [${failText(r.fail, r.detail)}${r.at === null ? "" : ` at ${r.at}`}]`;
  return `[${n1(r.enqueue_ms).padStart(9)}] ${`${r.category}:${r.subject}`.padEnd(34)}${r.outcome.padEnd(7)}${n1(total).padStart(8)}ms${timing} p${r.priority}${phasesText(r.phases)}${why}`;
}

/** Geometry, textures and heap only mean anything as deltas from `prev`. */
export function frameLine(s: Frame, prev: Frame | null): string {
  const d = prev === null ? "" : `  Δbuf ${signed(s.buffers - prev.buffers)} Δbuf_mb ${signed(s.buffer_mb - prev.buffer_mb)} Δheap ${signed(s.heap_used_mb - prev.heap_used_mb)}MB`;
  return `[${n1(s.t).padStart(9)}] dt ${n1(s.dt_ms)} = task ${n1(s.task_ms)} + offthread ${n1(offthreadMs(s))}  gpu ${n1(s.gpu_ms)}  sched ${n1(s.sched_ms)}/${n1(s.budget_ms)}${
    s.settled ? " (still)" : ""
  } (chunks ${n1(s.chunks_ms)})  draws ${s.draw_calls} tris ${
    tris(s.triangles)
  }  buf ${s.buffers}  heap ${s.heap_used_mb}MB  mount +${s.mounts_added}/-${s.mounts_removed} layouts ${s.mounts_layouts}${d}`;
}

export const longFrameLine = (l: LongFrame) =>
  `[${n1(l.t).padStart(9)}] ${l.source.padEnd(8)} ${n1(l.durationMs)} ms  blocking ${n1(l.blockingMs)}  script ${n1(l.scriptMs)}  render ${n1(l.renderMs)}`;

function windowSection(r: ProfilerReport): string[] {
  const f = r.frames;
  if (f === null) return ["frames n=0"];
  const last = f.last;
  const c = r.sections.counters;
  const spans = f.gpu_spans.filter((s) => s.spread.max > 0).map((s) => `${s.name} ${n1(s.spread.p95)}`).join("  ");
  const zooms = f.zoom_tris.map((t, z) => [z, t] as const).filter(([, t]) => t > 0).map(([z, t]) => `${zoomName(z)} ${tris(t)}`).join("  ");
  const steps = r.steps.length === 0 ? "none" : [...r.steps].sort((a, b) => b.ms / b.count - a.ms / a.count).map((s) => `${s.phase}/${s.name} ${n1(s.ms / s.count)}/${n1(s.max_ms)}`).join("  ");

  return [
    `frames n=${f.n}  over ${n1(f.seconds)}s  moving ${f.moving}  still ${f.n - f.moving}   display ${n1(last.frame_ms)} ms`,
    `  dt      ${spread(f.dt)} ms   missed ${f.missed} (${pct(f.missed, f.moving)}% of moving frames)`,
    `  task    p50 ${n1(f.task.p50)}  p95 ${n1(f.task.p95)} ms   (the kernel's turn: tick, draw, submit)`,
    `  offthd  p50 ${n1(f.offthread.p50)}  p95 ${n1(f.offthread.p95)} ms   (vsync/compositor/GPU/GC and the shell's glue)`,
    `  gpu     ${spread(f.gpu)} ms${last.gpu_ms === 0 ? "   (no timestamps: no renderer yet, or the device keeps none)" : ""}`,
    `  spans   p95 ${spans || "-"} ms   (over the frames that ran each)`,
    `  sched   p95 ${n1(f.sched.p95)}  max ${n1(f.sched.max)} ms   overran ${f.overran}   budget ${n1(last.budget_ms)}${last.settled ? " (still)" : ""}   phase p95 ${
      f.phases.map((p) => `${p.name} ${n1(p.spread.p95)}`).join("  ")
    }`,
    `  cpu     chunks p95 ${n1(f.chunks.p95)} ms   (the chunk scheduler's pass inside the visible phase)`,
    // Per step: a cooperative scheduler is only as cooperative as its longest step.
    `  steps   ${steps}   (ms per frame, max)`,
    `  by tag  ${f.census.map((t) => `${t.tag} ${round(t.draws)}/${tris(t.tris)}`).join("  ")}   (draws/tris a frame, mean)`,
    `  by zoom ${zooms || "-"}   (terrain tris a frame, mean)`,
    `  build   ${c.built_runs} runs  ${c.built_bakes} bakes  ${c.built_starved} refused a block`,
    `  per     chunks/bake ${f.chunks_per_bake === null ? "-" : n1(f.chunks_per_bake)}ms   (workload-normalized; comparable across sessions)`,
    `  rate    ${n1(f.perS[1])} bakes/s  ${n1(f.perS[0])} runs/s   while backlog ${n1(f.backlogPerS[1])} bakes/s  ${n1(f.backlogPerS[0])} runs/s over ${n1(f.backlog_s)}s`,
    `  faces   kept ${c.faces_kept} / ${c.faces_possible} (${pct(c.faces_kept, c.faces_possible)}%)  runs culled ${c.runs_culled}/${c.runs_total}`,
    `  coarse  boxes ${c.coarse_boxes}  low fill ${c.coarse_low_fill} (${pct(c.coarse_low_fill, c.coarse_boxes)}%; a cull at this fill was measured and did not pay)`,
    `  mounts  window +${f.mounts[0]}/-${f.mounts[1]}   heap ${signed(f.used_delta_mb)}MB`,
    `  waits   releases ${last.lod_offscreen_pending} pending, oldest ${round(last.lod_oldest_release_ms)} ms${starved(last.lod_oldest_release_ms)}   on-screen oldest ${
      round(last.lod_oldest_visible_ms)
    } ms${starved(last.lod_oldest_visible_ms)}`,
    // `live` counts a carved run whole, so it is committed, not held.
    `  heap    live ${pct(last.heap_live_mb, last.heap_range_mb)}% of the range   ${n1(last.heap_live_mb - last.heap_run_mb)}MB of it whole pages   runs hold ${
      pct(last.heap_slot_mb, last.heap_run_mb)
    }% of themselves, the other ${n1(last.heap_run_mb - last.heap_slot_mb)}MB what the carving costs`,
  ];
}

function lastSection(f: Frame): string[] {
  return ["last frame", ...GROUPS.filter((g) => g !== "pace").map((g) => `  ${g.padEnd(7)} ${groupText(g, f)}`)];
}

function sideSection(r: ProfilerReport): string[] {
  const lines = [];
  for (const [name, fields] of Object.entries(r.sections)) {
    if (name === "counters" || Object.values(fields).every((v) => v === 0)) continue;
    lines.push(`  ${name.padEnd(7)} ${Object.entries(fields).map(([k, v]) => fieldText(k, v)).join("  ")}`);
  }
  return lines.length === 0 ? [] : ["state", ...lines];
}

const peakSection = (p: Record<string, f64>) => ["peak (window)", `  ${Object.entries(p).map(([k, v]) => fieldText(k, v)).join("  ")}`];

function tasksSection(tasks: TaskSummary[]): string[] {
  const n = tasks.reduce((sum, t) => sum + t.n, 0);
  if (n === 0) return ["tasks n=0"];
  const lines = [`tasks n=${n}`];
  for (const t of tasks) {
    const bound = BOUND[t.category] ?? "?";
    const reasons = t.reasons.length === 0 ? "" : ` (${t.reasons.map((x) => `${failText(x.fail, x.detail)} x${x.n}`).join(", ")})`;
    lines.push(
      `  ${t.category.padEnd(9)}${bound.padEnd(9)}n=${t.n}   p50 ${n1(t.lat.p50)}  p95 ${n1(t.lat.p95)} ms   ok ${t.ok}  ab ${t.aborted}  err ${t.error}${reasons}`,
    );
    if (t.phase_mean !== null) {
      const means = t.phase_mean.map(phaseText).join("  ");
      lines.push(`  ${"".padEnd(18)}mean ${means}`);
    }
  }
  return lines;
}

export function profilerReportText(r: ProfilerReport): string {
  const out = [`=== profiler @ t=${n1(r.t)}ms  build ${r.build.number} ${r.build.id} (${r.build.time}) ===`];
  out.push(...windowSection(r));
  if (r.frames !== null) out.push(...lastSection(r.frames.last));
  out.push(...sideSection(r));
  if (r.peak !== null) out.push(...peakSection(r.peak));
  out.push(...tasksSection(r.tasks));
  out.push(`long frames n=${r.longs.length}${loafSupported() ? "" : " (LoAF unsupported; longtask only)"}`);

  if (r.recent.length > 0) {
    out.push(`--- last ${r.recent.length} tasks ---`);
    for (const rec of r.recent) out.push(taskLine(rec));
  }
  if (r.worst.length > 0) {
    out.push(`--- worst ${r.worst.length} frames in the window ---`);
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
  const kb = `${Math.max(1, round(text.length / 1024))} KB`;
  let said: string;
  try {
    await copyText(text);
    said = `report copied · ${kb}`;
  } catch {
    said = `report printed to the console · ${kb} (the clipboard refused it)`;
  }
  console.log(`[profiler] ${said}`);
  return said;
}
