// @pxl/web/modules/world/sched/log.ts

/**
 * The ledger's text side — the two things the gauge panel
 * (ledger_overlay.tsx) can't do:
 *
 * - a **live console stream** of records as they land, so a stutter can be
 *   watched happening and correlated against whatever else is logging,
 *   rather than read off a panel that only shows the settled aggregate.
 * - a **text report** of the current rings, for pasting into a findings doc
 *   (discussions/20260909-cooperative-scheduler-and-task-ledger.md's step 2).
 *
 * Both are dev instruments and neither feeds back into scheduling. The live
 * stream thresholds frames deliberately: at 60fps an unfiltered per-frame
 * line would bury everything else in the console within seconds, so only
 * frames that actually missed are worth a line.
 */

import { occlusion_stats } from "@/render/layer/pipeline/chunk_mesher.ts";
import { poi_place_stats } from "@/modules/world/poi/poi_layer.tsx";
import { poi_index_stats } from "@/modules/world/poi/poi_index.ts";
import { badge_atlas, poi_badge_build_stats, poi_badge_count } from "@/modules/world/poi/poi_badge.ts";
import { billboard_dropped } from "@/modules/world/billboard/frustum_budget.ts";
import { mesh_pool_stat } from "@/render/layer/pipeline/mesh_pool.ts";
import { band_pool_stats } from "@/render/layer/pipeline/volume.ts";
import { chunk_retention, lod_build_stats } from "@/modules/world/chunk_node.ts";
import { category_info } from "./categories.ts";
import {
  type FrameSample,
  ledger_frame_peak,
  ledger_frames_snapshot,
  ledger_long_frames_snapshot,
  ledger_set_sink,
  ledger_snapshot,
  ledger_worst_frames,
  type LongFrameRecord,
  type TaskRecord,
} from "./ledger.ts";

const FRAME_BUDGET_MS = 1000 / 60;
// A frame is only worth a line if it visibly missed: either it took longer
// than two budgets end to end, or its own scheduler pass alone blew one.
const LOG_FRAME_DT_MS = FRAME_BUDGET_MS * 2;
const RECENT_TASKS = 20;
const SLOWEST_FRAMES = 10;
// one decoded terrain page, measured live (terrain/config.ts: ~4.03MB).
const TILE_DECODED_MB = 4;

/** Percentile over an already-sorted array; 0 for an empty one. Shared with
 * ledger_overlay.tsx so the panel and the report never disagree. */
export function pct(sorted: number[], q: number): number {
  if (sorted.length === 0) return 0;
  return sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * q))];
}

const n1 = (n: number) => n.toFixed(1);
const pad = (s: string, w: number) => s.padEnd(w);
const lpad = (s: string, w: number) => s.padStart(w);

function phases_text(phases: Record<string, number> | undefined): string {
  if (!phases) return "";
  const parts = Object.entries(phases).map(([k, v]) => `${k.replace(/_ms$/, "")} ${k === "bytes" ? v : n1(v)}`);
  return parts.length === 0 ? "" : `  [${parts.join(" ")}]`;
}

function task_line(rec: TaskRecord): string {
  const total = rec.end_ms - rec.enqueue_ms;
  const queued = rec.start_ms !== null ? rec.start_ms - rec.enqueue_ms : null;
  const ran = rec.start_ms !== null ? rec.end_ms - rec.start_ms : null;
  const timing = queued === null ? "" : ` q${n1(queued)} r${n1(ran!)}`;
  return `[${lpad(n1(rec.enqueue_ms), 9)}] ${pad(rec.key, 34)} ${pad(rec.outcome, 7)} ${lpad(n1(total), 8)}ms${timing} p${rec.priority}${phases_text(rec.phases)}${
    rec.error ? `  [${rec.error}]` : ""
  }`;
}

function signed(n: number): string {
  return n >= 0 ? `+${n}` : `${n}`;
}

/** `prev` is the frame immediately before this one in the ring, when known —
 * geometry/texture/heap only mean something as deltas. A frame that stalls
 * with no mounts and a small scheduler pass has to be spending the time on
 * something it *created*, and this is where that shows up. */
function frame_line(s: FrameSample, prev?: FrameSample): string {
  const d = prev ? `  Δgeom ${signed(s.geometries - prev.geometries)} Δtex ${signed(s.textures - prev.textures)} Δheap ${signed(s.heap_mb - prev.heap_mb)}MB` : "";
  return `[${lpad(n1(s.t), 9)}] dt ${n1(s.dt_ms)} = task ${n1(s.task_ms)} + offthread ${n1(Math.max(0, s.dt_ms - s.task_ms))}` +
    `  gpu ${n1(s.gpu_ms)}  sched ${n1(s.sched_ms)} ` +
    `(rck ${n1(s.cpu.recheck_ms)} cmt ${n1(s.cpu.commit_ms)})  ` +
    `draws ${s.draw_calls} tris ${Math.round(s.triangles / 1000)}k  geom ${s.geometries}  heap ${s.heap_mb}MB  ` +
    `mount +${s.mounts.added}/-${s.mounts.removed} rerender ${s.mounts.rendered}${d}`;
}

function long_frame_line(l: LongFrameRecord): string {
  return `[${lpad(n1(l.t), 9)}] ${pad(l.source, 8)} ${n1(l.duration_ms)}ms  script ${n1(l.script_ms)}  render ${n1(l.render_ms)}  blocking ${n1(l.blocking_ms)}`;
}

let logging = false;

/** Turns the live console stream on/off by installing (or clearing) the
 * ledger's single sink. */
export function ledger_set_logging(on: boolean): void {
  if (on === logging) return;
  logging = on;
  if (!on) {
    ledger_set_sink(null);
    return;
  }
  ledger_set_sink({
    task: (rec) => console.log(`[ledger] ${task_line(rec)}`),
    frame: (s) => {
      if (s.dt_ms > LOG_FRAME_DT_MS || s.sched_ms > FRAME_BUDGET_MS) {
        console.log(`[ledger] ${frame_line(s)}`);
      }
    },
    long_frame: (l) => console.log(`[ledger] ${long_frame_line(l)}`),
  });
}

function frames_section(frames: readonly FrameSample[]): string[] {
  if (frames.length === 0) return ["frames n=0"];

  const sorted = (pick: (f: FrameSample) => number) => frames.map(pick).sort((a, b) => a - b);
  const dt = sorted((f) => f.dt_ms);
  const sched = sorted((f) => f.sched_ms);
  const rck = sorted((f) => f.cpu.recheck_ms);
  const cmt = sorted((f) => f.cpu.commit_ms);
  const rnd = sorted((f) => f.cpu.render_ms);
  const over = frames.filter((f) => f.dt_ms > FRAME_BUDGET_MS + 1).length;
  const last = frames[frames.length - 1];
  const heap_delta = last.heap_mb - frames[0].heap_mb;

  // Normalised per-unit-of-work costs. Every absolute number above mixes
  // "how much work this window happened to contain" with "what one unit of
  // it costs", which makes two sessions incomparable — and comparing two
  // sessions is the whole job. commit_ms went 4.5 -> 5.6ms across a change
  // and it took a separate argument to work out whether that was the change
  // or simply a busier pan. These say it directly.
  const scans = frames.reduce((a, f) => a + f.work.scans, 0);
  const bakes = frames.reduce((a, f) => a + f.work.bakes, 0);
  const runs = frames.reduce((a, f) => a + f.work.runs, 0);
  const commit_total = frames.reduce((a, f) => a + f.cpu.commit_ms, 0);
  const seconds = Math.max(1e-3, (last.t - frames[0].t) / 1000);
  const per = (total: number, n: number) => (n > 0 ? n1(total / n) : "-");

  return [
    `frames n=${frames.length}  over ${n1(seconds)}s`,
    `  dt      p50 ${n1(pct(dt, 0.5))}  p95 ${n1(pct(dt, 0.95))}  max ${n1(dt[dt.length - 1])} ms   over-budget ${over} (${Math.round(over / frames.length * 100)}%)`,
    `  task    p50 ${n1(pct(sorted((f) => f.task_ms), 0.5))}  p95 ${n1(pct(sorted((f) => f.task_ms), 0.95))} ms   (main thread's own rAF task)`,
    `  offthd  p50 ${n1(pct(sorted((f) => Math.max(0, f.dt_ms - f.task_ms)), 0.5))}  p95 ${n1(pct(sorted((f) => Math.max(0, f.dt_ms - f.task_ms)), 0.95))} ms   (vsync/compositor/GPU/GC)`,
    `  gpu     p50 ${n1(pct(sorted((f) => f.gpu_ms), 0.5))}  p95 ${n1(pct(sorted((f) => f.gpu_ms), 0.95))} ms${last.gpu_ms === 0 ? "   (timer unsupported or unresolved)" : ""}`,
    `  sched   p95 ${n1(pct(sched, 0.95))}  max ${n1(sched[sched.length - 1])} ms   budget ${n1(last.budget_ms)}   phase p95 ${
      Object.keys(last.phases ?? {}).map((k) => `${k} ${n1(pct(sorted((f) => f.phases?.[k] ?? 0), 0.95))}`).join("  ")
    }`,
    `  cpu     rck p95 ${n1(pct(rck, 0.95))}  cmt p95 ${n1(pct(cmt, 0.95))}  rnd p95 ${n1(pct(rnd, 0.95))} ms`,
    `  gpu     draws ${last.draw_calls}  tris ${Math.round(last.triangles / 1000)}k  tex ${last.textures}  fill ${(last.px / 1e6).toFixed(1)}Mpx x${last.fx_passes}`,
    `  mem     heap ${last.heap_mb}MB (${heap_delta >= 0 ? "+" : ""}${heap_delta})  tiles ${last.tile_pages} (~${last.tile_pages * TILE_DECODED_MB}MB)  poi ${last.poi_count}`,
    `  place   poi ${n1(poi_place_stats.ms)} max ${
      n1(poi_place_stats.max_ms)
    } ms over ${poi_place_stats.runs} runs   hit ${poi_place_stats.hits} (max ${poi_place_stats.max_hits}) -> cand ${poi_place_stats.candidates} -> kept ${poi_place_stats.kept} (+${poi_place_stats.absorbed} absorbed, biggest ${poi_place_stats.biggest}${
      billboard_dropped() > 0 ? `, DROPPED ${billboard_dropped()}` : ""
    })`,
    `  split   sync ${n1(poi_place_stats.sync_ms)} max ${n1(poi_place_stats.max_sync_ms)} ms   badge ${poi_badge_build_stats.metrics} measured / ${poi_badge_build_stats.builds} built ${
      n1(poi_badge_build_stats.ms)
    }ms total, ${poi_badge_count()} cached`,
    `  atlas   ${badge_atlas.stats().size}px  ${badge_atlas.stats().entries} entries  ${badge_atlas.stats().shelves} shelves  ${
      Math.round(badge_atlas.stats().used_px / (badge_atlas.stats().size * badge_atlas.stats().size) * 100)
    }% used  ${badge_atlas.stats().repacks} repacks`,
    `  faces   kept ${occlusion_stats.faces_kept} / ${occlusion_stats.faces_possible} (${
      occlusion_stats.faces_possible > 0 ? Math.round(occlusion_stats.faces_kept / occlusion_stats.faces_possible * 100) : 0
    }%)  runs culled ${occlusion_stats.culled}/${occlusion_stats.total_runs}`,
    `  index   poi ${poi_index_stats().indexed} resident  ${poi_index_stats().rebuilds} rebuilds${poi_index_stats().sweeping > 0 ? `  sweeping ${poi_index_stats().sweeping} pages` : ""}${
      poi_index_stats().dropped > 0 ? `  DROPPED ${poi_index_stats().dropped}` : ""
    }`,
    `  pool    live ${last.pool.live_mb}MB  idle ${last.pool.idle_mb}MB  hit ${last.pool.hit_pct}%  biggest ${Math.round(mesh_pool_stat().max_block_bytes / 1048576)}MB   window scans ${
      frames.reduce((a, f) => a + f.work.scans, 0)
    } bakes ${frames.reduce((a, f) => a + f.work.bakes, 0)} runs ${frames.reduce((a, f) => a + f.work.runs, 0)}`,
    `  per     commit/bake ${per(commit_total, bakes)}ms  build/krun ${per(lod_build_stats.build_ms, lod_build_stats.build_runs / 1000)}ms  scan/chunk ${
      per(lod_build_stats.scan_ms, lod_build_stats.scan_count)
    }ms   (workload-normalised; comparable across sessions)`,
    `  rate    ${n1(bakes / seconds)} bakes/s  ${n1(scans / seconds)} scans/s  ${Math.round(runs / seconds / 1000)}k runs/s`,
    `  bands   reused ${band_pool_stats.reused} / ${band_pool_stats.reused + band_pool_stats.allocated} (${
      band_pool_stats.reused + band_pool_stats.allocated > 0 ? Math.round(band_pool_stats.reused / (band_pool_stats.reused + band_pool_stats.allocated) * 100) : 0
    }%)  ~1.31MB each; a miss is that much garbage`,
    `  retain  ${
      (() => {
        const r = chunk_retention();
        return `nodes ${r.nodes}  runsets ${r.run_sets} (${r.run_set_mb}MB)  tiers ${r.tiers}  decor ${r.decor_steps} steps / ${r.decor_instances} instances`;
      })()
    }`,
    `  lod     chunks ${last.lod.chunks}  in-frustum ${last.lod.in_frustum}  pending ${last.lod.pending}  settling ${last.lod.settling}  blending ${last.lod.blend_active}`,
    `  wait    releases ${last.lod.offscreen_pending} pending, oldest ${Math.round(last.lod.oldest_release_ms)} ms   on-screen oldest ${Math.round(last.lod.oldest_visible_ms)} ms`,
    `  mounts  this frame +${last.mounts.added}/-${last.mounts.removed} rerender ${last.mounts.rendered}   window total +${frames.reduce((a, f) => a + f.mounts.added, 0)}/-${
      frames.reduce((a, f) => a + f.mounts.removed, 0)
    }`,
    `  queue   terrain ${last.q_terrain}  poi ${last.q_poi}`,
  ];
}

/** Session worst-seen, which the ~4s ring above can no longer show once a
 * stutter has scrolled out of it. */
function peak_section(): string[] {
  const p = ledger_frame_peak();
  return [
    `peak (session)`,
    `  dt ${n1(p.dt_ms)}  sched ${n1(p.sched_ms)}  rck ${n1(p.recheck_ms)}  cmt ${n1(p.commit_ms)}  rnd ${n1(p.render_ms)} ms`,
    `  draws ${p.draw_calls}  tris ${Math.round(p.triangles / 1000)}k  heap ${p.heap_mb}MB  queue t${p.q_terrain}/p${p.q_poi}`,
    `  lod chunks ${p.chunks}  in-frustum ${p.in_frustum}  pending ${p.pending}  settling ${p.settling}  blending ${p.blend_active}`,
    `  wait releases ${p.offscreen_pending} pending, oldest ${Math.round(p.oldest_release_ms)} ms   on-screen oldest ${Math.round(p.oldest_visible_ms)} ms`,
  ];
}

function tasks_section(tasks: readonly TaskRecord[]): string[] {
  if (tasks.length === 0) return ["tasks n=0"];

  type Group = {
    lat: number[];
    ok: number;
    aborted: number;
    error: number;
    /** Failures by reason — see fetch_scheduler.ts's error_reason. */
    reasons: Map<string, number>;
    /** summed phase values + how many records contributed, for a mean. */
    phase_sum: Map<string, number>;
    phase_n: number;
  };
  const by = new Map<string, Group>();
  for (const rec of tasks) {
    let g = by.get(rec.category);
    if (!g) by.set(rec.category, g = { lat: [], ok: 0, aborted: 0, error: 0, reasons: new Map(), phase_sum: new Map(), phase_n: 0 });
    if (rec.outcome === "ok") {
      g.ok++;
      g.lat.push(rec.end_ms - rec.enqueue_ms);
    } else if (rec.outcome === "aborted") g.aborted++;
    else {
      g.error++;
      const reason = rec.error ?? "unrecorded";
      g.reasons.set(reason, (g.reasons.get(reason) ?? 0) + 1);
    }
    if (rec.phases) {
      g.phase_n++;
      for (const [k, v] of Object.entries(rec.phases)) g.phase_sum.set(k, (g.phase_sum.get(k) ?? 0) + v);
    }
  }

  const lines = [`tasks n=${tasks.length}`];
  for (const [cat, g] of by) {
    const lat = g.lat.sort((a, b) => a - b);
    const bound = category_info(cat)?.bound ?? "?";
    lines.push(
      `  ${pad(cat, 9)}${pad(bound, 9)}n=${g.ok + g.aborted + g.error}   p50 ${n1(pct(lat, 0.5))}  p95 ${n1(pct(lat, 0.95))} ms   ok ${g.ok}  ab ${g.aborted}  err ${g.error}${
        g.reasons.size > 0 ? ` (${[...g.reasons.entries()].sort((a, b) => b[1] - a[1]).map(([r, n]) => `${r} x${n}`).join(", ")})` : ""
      }`,
    );
    // Mean phase split — the number that says whether the static bound-class
    // label above is actually true for this run.
    if (g.phase_n > 0) {
      const means = [...g.phase_sum.entries()]
        .map(([k, sum]) => `${k.replace(/_ms$/, "")} ${k === "bytes" ? Math.round(sum / g.phase_n) : n1(sum / g.phase_n)}`)
        .join("  ");
      lines.push(`  ${pad("", 18)}mean ${means}`);
    }
  }
  return lines;
}

/**
 * The whole instrument as pasteable text: aggregate summary, then the raw
 * tail of each ring. Sized to land in a findings doc, not to be a trace —
 * trace.ts is the machine-readable path.
 */
export function ledger_report_text(): string {
  const frames = ledger_frames_snapshot();
  const tasks = ledger_snapshot();
  const longs = ledger_long_frames_snapshot();
  const loaf_supported = typeof PerformanceObserver !== "undefined" &&
    !!PerformanceObserver.supportedEntryTypes?.includes("long-animation-frames");

  const out = [
    `=== sched/ledger @ t=${n1(performance.now())}ms ===`,
    ...frames_section(frames),
    ...(frames.length > 0 ? peak_section() : []),
    ...tasks_section(tasks),
    `long frames n=${longs.length}${loaf_supported ? "" : " (LoAF unsupported; longtask only)"}`,
  ];

  if (tasks.length > 0) {
    out.push(`--- last ${Math.min(RECENT_TASKS, tasks.length)} tasks ---`);
    for (const rec of tasks.slice(-RECENT_TASKS)) out.push(task_line(rec));
  }
  if (frames.length > 0) {
    const idx = new Map(frames.map((f, i) => [f, i]));
    const slowest = [...frames].sort((a, b) => b.dt_ms - a.dt_ms).slice(0, SLOWEST_FRAMES);
    out.push(`--- slowest ${slowest.length} frames in the ~4s ring ---`);
    for (const s of slowest) out.push(frame_line(s, frames[(idx.get(s) ?? 0) - 1]));
  }
  const worst = ledger_worst_frames();
  if (worst.length > 0) {
    out.push(`--- worst ${worst.length} frames of the whole session ---`);
    for (const w of worst) out.push(frame_line(w.sample, w.prev ?? undefined));
  }
  if (longs.length > 0) {
    out.push(`--- long frames ---`);
    for (const l of longs) out.push(long_frame_line(l));
  }

  return out.join("\n");
}

/** Prints the report and puts it on the clipboard, returning it either way.
 * Wired to `window.__pxl_ledger_log` in dev (ledger_overlay.tsx). */
export async function ledger_report_to_console(): Promise<string> {
  const text = ledger_report_text();
  console.log(text);
  let copied = false;
  try {
    await navigator.clipboard.writeText(text);
    copied = true;
  } catch {
    // clipboard can reject without a user gesture / on an insecure origin —
    // the console print above is the fallback.
  }
  console.log(`[ledger] report ${copied ? "copied to clipboard" : "printed above (clipboard refused)"} — ${text.length} bytes`);
  return text;
}
