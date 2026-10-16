// @pxl/web/feature/debug/profiler/report.ts

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { type Errno, strerror } from "@xpute/kit/status/errno.spec.ts";
import { copyText } from "@platform/save.ts";

import { fieldText, type Frame, GROUPS, groupText, n1, pct, phaseText, tris, valueText } from "./lane.ts";
import { loafSupported, type LongFrame } from "./long-frame.ts";
import type { ProfilerReport, Spread, TaskRecord, TaskSummary } from "./view.ts";

const signed = (n: number) => (n >= 0 ? `+${n}` : String(n));
/** A count of bytes as the one MB the figures are shown in, signed. */
const signedBytes = (b: number) => `${b >= 0 ? "+" : ""}${valueText("_bytes", b)}`;
const bytes = (b: number) => valueText("_bytes", b);
/** Where the interval's turns ended: before it, by the time left, or past it, by how much. */
const pastText = (f: Frame) => (f.over_ms > 0 ? `over by ${n1(f.over_ms)}` : `off-turn ${n1(f.off_turn_ms)}`);
const spread = (s: Spread) => `p50 ${n1(s.p50)}  p95 ${n1(s.p95)}  max ${n1(s.max)}`;

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
  return `[${n1(r.enqueue_ms).padStart(9)}] ${`${r.stream}:${r.subject}`.padEnd(34)}${r.outcome.padEnd(7)}${n1(total).padStart(8)}ms${timing} p${r.priority}${phasesText(r.phases)}${why}`;
}

/** Geometry, textures and heap only mean anything as deltas from `prev`. */
export function frameLine(s: Frame, prev: Frame | null): string {
  const d = prev === null ? "" : `  Δbuf ${signed(s.buffers - prev.buffers)} Δbuf_mb ${signed(s.buffer_mb - prev.buffer_mb)} Δheap peak ${signedBytes(s.heap_peak_bytes - prev.heap_peak_bytes)}`;
  return `[${n1(s.t).padStart(9)}] ${valueText("kind_name", s.kind_name)}  interval ${n1(s.interval_ms)} = ${s.turns} turns ${n1(s.busy_ms)} + ${pastText(s)}  tick ${n1(s.tick_ms)}/${
    n1(s.budget_ms)
  }${s.settled ? " (still)" : ""}  gpu ${n1(s.gpu_ms)}  draws ${s.draw_calls} tris ${tris(s.triangles)}  buf ${s.buffers}  heap peak ${bytes(s.heap_peak_bytes)}${d}`;
}

export const longFrameLine = (l: LongFrame) =>
  `[${n1(l.t).padStart(9)}] ${l.source.padEnd(8)} ${n1(l.durationMs)} ms  blocking ${n1(l.blockingMs)}  script ${n1(l.scriptMs)}  render ${n1(l.renderMs)}`;

function windowSection(r: ProfilerReport): string[] {
  const f = r.frames;
  if (f === null) return ["frames n=0"];
  const last = f.last;
  const spans = f.gpu_spans.filter((s) => s.spread.max > 0).map((s) => `${s.name} ${n1(s.spread.p95)}`).join("  ");
  const steps = r.steps.length === 0 ? "none" : [...r.steps].sort((a, b) => b.ms / b.count - a.ms / a.count).map((s) => `${s.phase}/${s.name} ${n1(s.ms / s.count)}/${n1(s.max_ms)}`).join("  ");

  const counted = (rows: { name: string; n: f64 }[]) => rows.filter((r) => r.n > 0).map((r) => `${r.name} ${r.n}`).join("  ") || "-";

  return [
    `frames n=${f.n}  over ${n1(f.seconds)}s  moving ${f.moving}  still ${f.n - f.moving}   display ${n1(last.display_ms)} ms   turns ${f.turns}`,
    `  kinds   ${counted(f.kinds)}   wakes ${counted(f.wakes)}`,
    `  interval ${spread(f.interval)} ms   missed ${f.missed} (${pct(f.missed, f.moving)}% of moving frames)`,
    `  busy    ${spread(f.busy)} ms   (the interval's turns, rise to fall)`,
    `  off-turn ${spread(f.off_turn)} ms   long ${f.long} (turns past their interval)`,
    `  tick    ${spread(f.tick)} ms   overran ${f.overran}   budget ${n1(last.budget_ms)}${last.settled ? " (still)" : ""}   phase p95 ${
      f.phases.map((p) => `${p.name} ${n1(p.spread.p95)}`).join("  ")
    }`,
    `  tasks   ${spread(f.tasks)} ms   (the turns' tasks past the frame)`,
    `  gpu     ${spread(f.gpu)} ms${last.gpu_ms === 0 ? "   (no timestamps: no renderer yet, or the device keeps none)" : ""}`,
    `  spans   p95 ${spans || "-"} ms   (over the frames that ran each)`,
    // Per step: a cooperative scheduler is only as cooperative as its longest step.
    `  steps   ${steps}   (ms per frame, max)`,
    `  by tag  ${f.census.map((t) => `${t.tag} ${round(t.draws)}/${tris(t.tris)}`).join("  ")}   (draws/tris a frame, mean)`,
    `  heap peak Δ ${signedBytes(f.heap_peak_delta_bytes)}`,
    // `lent` counts a carved run whole, so it is committed, not held.
    `  heap    lent ${pct(last.heap_lent_bytes, last.heap_range_bytes)}% of the range   ${bytes(last.heap_lent_bytes - last.heap_run_bytes)} of it whole pages   runs hold ${
      pct(last.heap_held_bytes, last.heap_run_bytes)
    }% of themselves, the other ${bytes(last.heap_run_bytes - last.heap_held_bytes)} what the carving costs`,
  ];
}

function lastSection(f: Frame): string[] {
  return ["last frame", ...GROUPS.filter((g) => g !== "time").map((g) => `  ${g.padEnd(7)} ${groupText(g, f)}`)];
}

function sideSection(r: ProfilerReport): string[] {
  const lines = [];
  for (const [name, fields] of Object.entries(r.sections)) {
    if (name === "counters" || Object.values(fields).every((v) => v === 0)) continue;
    lines.push(`  ${name.padEnd(7)} ${Object.entries(fields).map(([k, v]) => fieldText(k, v)).join("  ")}`);
  }
  return lines.length === 0 ? [] : ["state", ...lines];
}

const peakSection = (p: Record<string, f64>) => ["peak (since recording)", `  ${Object.entries(p).map(([k, v]) => fieldText(k, v)).join("  ")}`];

function tasksSection(tasks: TaskSummary[]): string[] {
  const n = tasks.reduce((sum, t) => sum + t.n, 0);
  if (n === 0) return ["tasks n=0"];
  const lines = [`tasks n=${n}`];
  for (const t of tasks) {
    const bound = BOUND[t.stream] ?? "?";
    const reasons = t.reasons.length === 0 ? "" : ` (${t.reasons.map((x) => `${failText(x.fail, x.detail)} x${x.n}`).join(", ")})`;
    lines.push(
      `  ${t.stream.padEnd(9)}${bound.padEnd(9)}n=${t.n}   p50 ${n1(t.lat.p50)}  p95 ${n1(t.lat.p95)} ms   ok ${t.ok}  ab ${t.aborted}  err ${t.error}${reasons}`,
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
