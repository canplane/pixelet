// @pxl/web/feature/dev/ledger/report.ts
//
// The ledger's report, written from the numbers the kernel measured and the
// shell read in place (view.ts). What a lane reads as is lane.ts's, the one
// table this, the panel and the trace share; what is written here are the
// lines that are more than a lane — percentiles, ratios and the asides that
// say what a line means. The kernel carries no word of it, the way libc
// rather than the kernel carries what an errno reads as.
//
// Pasteable text, sized to land in a findings doc; trace.ts is the
// machine-readable path.

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";

import { fieldText, type Frame, GROUPS, groupText, n1, tris } from "./lane.ts";
import { loafSupported, type LongFrame } from "./long-frame.ts";
import type { LedgerReport, Spread, TaskRecord, TaskSummary } from "./view.ts";

const share = (part: number, whole: number) => (whole > 0 ? round((part / whole) * 100) : 0);
const signed = (n: number) => (n >= 0 ? `+${n}` : String(n));
const spread = (s: Spread) => `p50 ${n1(s.p50)}  p95 ${n1(s.p95)}  max ${n1(s.max)}`;

/** A wait long enough to be starvation rather than queueing. */
const STARVED_MS = 2000;
const starved = (ms: number) => (ms >= STARVED_MS ? "  STARVED" : "");

// ---- the words ----

/** Why a fetch failed, as `spec/fetch.ts` numbers it. A refusal says its
 * status instead, which is the part that tells one from another. */
export function failText(fail: u32, status: u32): string {
  switch (fail) {
    case 0x0101:
      return "network";
    case 0x0102:
      return "shell";
    case 0x0201:
      return `status ${status}`;
    case 0x0202:
      return "no_room";
    case 0x0301:
      return "decode";
    default:
      return "none";
  }
}

/** Which optimization lever a hotspot in a category needs — set once from
 * profiling, never fed back into scheduling. The measurements behind each are
 * the note beside it; they were taken on the date it names. */
export const BOUND: Record<string, { bound: string; note: string }> = {
  terrain: {
    bound: "network",
    note: "Measured 2026-09-09: 4.2MB/page, mean net 322.5ms vs decode 0.6ms — decode_terrain is an XTP cursor read and is free. The demand is network-bound end to end.",
  },
  poi: {
    bound: "network",
    note: "Measured 2026-09-09: mean net 269.3ms vs JSON.parse 2.2ms on ~114KB pages (295KB worst). Latency-bound; lever is priority order, not parallelism.",
  },
  geocode: {
    bound: "network",
    note: "single-slot latest-wins; one small page lookup, pure RTT. Already debounced upstream, so the lever left is queue priority.",
  },
  search: {
    bound: "network",
    note: "the one feature with no static bake — always a live API round trip, and the user is watching. Runs ahead of every streaming category on purpose.",
  },
};

const phasesText = (p: [string, f64][]) => p.length === 0 ? "" : `  [${p.map(([k, v]) => `${k.replace(/_ms$/, "")} ${k === "bytes" ? v : n1(v)}`).join(" ")}]`;

export function taskLine(r: TaskRecord): string {
  const total = r.end_ms - r.enqueue_ms;
  const timing = r.start_ms === null ? "" : `  q ${n1(r.start_ms - r.enqueue_ms)} + run ${n1(r.end_ms - r.start_ms)}`;
  const why = r.fail === 0 ? "" : `  [${failText(r.fail, r.status)}]`;
  return `[${n1(r.enqueue_ms).padStart(9)}] ${`${r.category}:${r.subject}`.padEnd(34)}${r.outcome.padEnd(7)}${n1(total).padStart(8)}ms${timing} p${r.priority}${phasesText(r.phases)}${why}`;
}

/** A frame's headline — the pacing split and what it drew — and, with the
 * frame before it, what moved: geometry, textures and the heap only mean
 * anything as deltas. */
export function frameLine(s: Frame, prev: Frame | null): string {
  const d = prev === null ? "" : `  Δbuf ${signed(s.buffers - prev.buffers)} Δbuf_mb ${signed(s.buffer_mb - prev.buffer_mb)} Δheap ${signed(s.heap_used_mb - prev.heap_used_mb)}MB`;
  return `[${n1(s.t).padStart(9)}] dt ${n1(s.dt_ms)} = task ${n1(s.task_ms)} + offthread ${n1(s.dt_ms - s.task_ms)}  gpu ${n1(s.gpu_ms)}  sched ${n1(s.sched_ms)}/${n1(s.budget_ms)}${
    s.settled ? " (still)" : ""
  } (tick ${n1(s.tick_ms)})  draws ${s.draw_calls} tris ${
    tris(s.triangles)
  }  buf ${s.buffers}  heap ${s.heap_used_mb}MB  mount +${s.mounts_added}/-${s.mounts_removed} layouts ${s.mounts_layouts}${d}`;
}

export const longFrameLine = (l: LongFrame) =>
  `[${n1(l.t).padStart(9)}] ${l.source.padEnd(8)} ${n1(l.durationMs)} ms  blocking ${n1(l.blockingMs)}  script ${n1(l.scriptMs)}  render ${n1(l.renderMs)}`;

// ---- the sections ----

/** The window's percentiles and rates: what no one frame says. */
function windowSection(r: LedgerReport): string[] {
  const f = r.frames;
  if (f === null) return ["frames n=0"];
  const last = f.last;
  const c = r.sections.counters;
  const bands = c.bands_reused + c.bands_allocated;
  const spans = f.gpu_spans.filter((s) => s.spread.max > 0).map((s) => `${s.name} ${n1(s.spread.p95)}`).join("  ");
  const levels = f.level_tris.map((t, k) => [k, t] as const).filter(([, t]) => t > 0).map(([k, t]) => `L${k} ${tris(t)}`).join("  ");
  const steps = r.steps.length === 0 ? "none" : [...r.steps].sort((a, b) => b.ms / b.count - a.ms / a.count).map((s) => `${s.phase}/${s.name} ${n1(s.ms / s.count)}/${n1(s.max_ms)}`).join("  ");

  return [
    `frames n=${f.n}  over ${n1(f.seconds)}s  moving ${f.moving}  still ${f.n - f.moving}   display ${n1(last.frame_ms)} ms`,
    `  dt      ${spread(f.dt)} ms   missed ${f.missed} (${share(f.missed, f.moving)}% of moving frames)`,
    `  task    p50 ${n1(f.task.p50)}  p95 ${n1(f.task.p95)} ms   (the kernel's turn: tick, draw, submit)`,
    `  offthd  p50 ${n1(f.offthread.p50)}  p95 ${n1(f.offthread.p95)} ms   (vsync/compositor/GPU/GC and the shell's glue)`,
    `  gpu     ${spread(f.gpu)} ms${last.gpu_ms === 0 ? "   (no timestamps: no renderer yet, or the device keeps none)" : ""}`,
    `  spans   p95 ${spans || "-"} ms   (over the frames that ran each)`,
    `  sched   p95 ${n1(f.sched.p95)}  max ${n1(f.sched.max)} ms   overran ${f.overran}   budget ${n1(last.budget_ms)}${last.settled ? " (still)" : ""}   phase p95 ${
      f.phases.map((p) => `${p.name} ${n1(p.spread.p95)}`).join("  ")
    }`,
    `  cpu     tick p95 ${n1(f.tick.p95)} ms   (the kernel's tick inside the visible phase)`,
    // Per step, so a phase that costs more than its steps should does not hide
    // which step is not yielding: a cooperative scheduler is only as
    // cooperative as its longest step.
    `  steps   ${steps}   (ms per frame, max)`,
    `  by tag  ${f.census.map((t) => `${t.tag} ${round(t.draws)}/${tris(t.tris)}`).join("  ")}   (draws/tris a frame, mean)`,
    `  by lvl  ${levels || "-"}   (terrain tris a frame, mean)`,
    `  build   ${c.built_direct} direct  ${c.built_scans} scans  ${c.built_bakes} bakes  ${c.built_starved} refused a block${c.decor_dropped > 0 ? `   DECOR DROPPED ${c.decor_dropped}` : ""}`,
    `  per     tick/bake ${f.tick_per_bake === null ? "-" : n1(f.tick_per_bake)}ms   (workload-normalised; comparable across sessions)`,
    `  rate    ${n1(f.perS[1])} bakes/s  ${n1(f.perS[0])} scans/s   while backlog ${n1(f.backlogPerS[1])} bakes/s  ${n1(f.backlogPerS[0])} scans/s over ${n1(f.backlog_s)}s`,
    `  faces   kept ${c.faces_kept} / ${c.faces_possible} (${share(c.faces_kept, c.faces_possible)}%)  runs culled ${c.runs_culled}/${c.runs_total}`,
    `  coarse  boxes ${c.coarse_boxes}  low fill ${c.coarse_low_fill} (${share(c.coarse_low_fill, c.coarse_boxes)}%; a threshold cull would have skipped these)`,
    `  bands   reused ${c.bands_reused} / ${bands} (${share(c.bands_reused, bands)}%)  ~1.31MB each; a miss borrows from the heap`,
    `  mounts  window +${f.mounts[0]}/-${f.mounts[1]}   heap ${signed(f.used_delta_mb)}MB`,
    `  waits   releases ${last.lod_offscreen_pending} pending, oldest ${round(last.lod_oldest_release_ms)} ms${starved(last.lod_oldest_release_ms)}   on-screen oldest ${
      round(last.lod_oldest_visible_ms)
    } ms${starved(last.lod_oldest_visible_ms)}`,
  ];
}

/** The last frame, a line a group of its lanes. */
function lastSection(f: Frame): string[] {
  return ["last frame", ...GROUPS.filter((g) => g !== "pace").map((g) => `  ${g.padEnd(7)} ${groupText(g, f)}`)];
}

/** The summary's side sections, a field each, as the kernel names them; a
 * section with nothing in it is left out (POI, while it is not wired). */
function sideSection(r: LedgerReport): string[] {
  const lines = [];
  for (const [name, fields] of Object.entries(r.sections)) {
    if (name === "counters" || Object.values(fields).every((v) => v === 0)) continue;
    lines.push(`  ${name.padEnd(7)} ${Object.entries(fields).map(([k, v]) => fieldText(k, v)).join("  ")}`);
  }
  return lines.length === 0 ? [] : ["state", ...lines];
}

const peakSection = (p: Record<string, f64>) => ["peak (window)", `  ${Object.entries(p).map(([k, v]) => fieldText(k, v)).join("  ")}`];

function tasksSection(n: u32, tasks: TaskSummary[]): string[] {
  if (n === 0) return ["tasks n=0"];
  const lines = [`tasks n=${n}`];
  for (const t of tasks) {
    const bound = BOUND[t.category]?.bound ?? "?";
    const reasons = t.reasons.length === 0 ? "" : ` (${t.reasons.map((x) => `${failText(x.fail, x.status)} x${x.n}`).join(", ")})`;
    lines.push(
      `  ${t.category.padEnd(9)}${bound.padEnd(9)}n=${t.n}   p50 ${n1(t.lat.p50)}  p95 ${n1(t.lat.p95)} ms   ok ${t.ok}  ab ${t.aborted}  err ${t.error}${reasons}`,
    );
    if (t.phase_mean !== null) {
      const means = t.phase_mean.map(([k, v]) => `${k.replace(/_ms$/, "")} ${k === "bytes" ? round(v) : n1(v)}`).join("  ");
      lines.push(`  ${"".padEnd(18)}mean ${means}`);
    }
  }
  return lines;
}

/** The whole instrument as pasteable text: the window, the last frame, the
 * state beside the rings, then the raw tail of each ring. */
export function ledgerReportText(r: LedgerReport): string {
  const out = [`=== sched/ledger @ t=${n1(r.t)}ms  build ${r.build.number} ${r.build.id} (${r.build.time}) ===`];
  out.push(...windowSection(r));
  if (r.frames !== null) out.push(...lastSection(r.frames.last));
  out.push(...sideSection(r));
  if (r.peak !== null) out.push(...peakSection(r.peak));
  out.push(...tasksSection(r.taskN, r.tasks));
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

/** Prints the report and puts it on the clipboard where the page may. */
export async function ledgerReportPrint(text: string): Promise<void> {
  console.log(text);
  let copied = false;
  try {
    await navigator.clipboard.writeText(text);
    copied = true;
  } catch {
    // The clipboard can refuse without a user gesture or on an insecure
    // origin; the console print above is the fallback.
  }
  console.log(`[ledger] report ${copied ? "copied to clipboard" : "printed above (clipboard refused)"} — ${text.length} bytes`);
}
