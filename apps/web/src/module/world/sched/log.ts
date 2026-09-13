// @pxl/web/module/world/sched/log.ts

/**
 * The ledger's text side — the two things the gauge panel
 * (ledger_overlay.tsx) can't do:
 *
 * - a **live console stream** of records as they land, so a stutter can be
 *   watched happening and correlated against whatever else is logging,
 *   rather than read off a panel that only shows the settled aggregate.
 * - a **text report** of the current rings, for pasting into a findings doc
 *   (issue/20260909-cooperative-scheduler-and-task-ledger.md's step 2).
 *
 * Both are dev instruments and neither feeds back into scheduling. The live
 * stream thresholds frames deliberately: an unfiltered per-frame line would
 * bury everything else in the console within seconds, so only frames that
 * missed a vsync while moving, or overran the budget, are worth a line —
 * summary.ts's `missed` and `overran`, the same tests the report counts.
 */

import { CORE_BUILD } from "@/render/layer/pipeline/core_build.ts";
import { poi_field_stats, poi_place_stats } from "@/module/world/poi/poi_layer.tsx";
import { tick_step_stats, tick_step_stats_reset } from "@/module/world/sched/tick.ts";
import { fetch_counts, MAX_INFLIGHT } from "@/module/world/sched/fetch_scheduler.ts";
import { poi_index_stats } from "@/module/world/poi/poi_index.ts";
import { badge_atlas, poi_badge_build_stats, poi_badge_count } from "@/module/world/poi/poi_badge.ts";
import { billboard_dropped } from "@/module/world/billboard/frustum_budget.ts";
import { CORE_MEMORY, core_pool_stat, core_sections_stat, MODULE_SECTION } from "@/render/layer/pipeline/core_pool.ts";
import { band_pool_stats } from "@/render/layer/pipeline/volume.ts";
import { chunk_retained_bytes, chunk_retention, lod_build_stats, RETAIN_BUDGET_BYTES } from "@/module/world/chunk_node.ts";
import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/lib/env/config.ts";
import { category_info } from "./category.ts";
import {
  type FrameSample,
  ledger_frame_peak,
  ledger_frames_snapshot,
  ledger_long_frames_snapshot,
  ledger_recording_start,
  ledger_set_sink,
  ledger_snapshot,
  ledger_worst_frames,
  type LongFrameRecord,
  type TaskRecord,
} from "./ledger.ts";
import { census_rows, type CensusRow, missed, overran, type Spread, STARVED_MS, summarize_frames, summarize_tasks, TILE_DECODED_MB, tris_text } from "./summary.ts";

const RECENT_TASKS = 20;

const n1 = (n: number) => n.toFixed(1);
const share = (part: number, whole: number) => (whole > 0 ? Math.round(part / whole * 100) : 0);
const census_text = (rows: CensusRow[]) => rows.map(([tag, d, t]) => `${tag} ${Math.round(d)}/${tris_text(t)}`).join("  ");
const spread_text = (s: Spread) => `p50 ${n1(s.p50)}  p95 ${n1(s.p95)}  max ${n1(s.max)}`;
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
    `  gpu ${n1(s.gpu_ms)}  sched ${n1(s.sched_ms)}/${n1(s.budget_ms)}${s.settled ? " still" : ""} ` +
    `(rck ${n1(s.cpu.recheck_ms)} cmt ${n1(s.cpu.commit_ms)})  ` +
    `draws ${s.draw_calls} tris ${Math.round(s.triangles / 1000)}k  geom ${s.geometries}  heap ${s.heap_mb}MB  ` +
    `mount +${s.mounts.added}/-${s.mounts.removed} rerender ${s.mounts.rendered}${d}`;
}

function long_frame_line(l: LongFrameRecord): string {
  return `[${lpad(n1(l.t), 9)}] ${pad(l.source, 8)} ${n1(l.duration_ms)}ms  script ${n1(l.script_ms)}  render ${n1(l.render_ms)}  blocking ${n1(l.blocking_ms)}`;
}

/**
 * The side counters printed beside the rings. The modules that keep them
 * count from load; read here as differences from the moment the window
 * began, so a line of the report and the frames beside it cover the same
 * stretch of time — after a restart they used to carry every earlier window
 * too. Read-only, like the rest of the instrument: nothing is written back.
 */
function counters() {
  return {
    faces_kept: CORE_BUILD.stat().faces_kept,
    faces_possible: CORE_BUILD.stat().faces_possible,
    runs_culled: CORE_BUILD.stat().culled,
    runs_total: CORE_BUILD.stat().total_runs,
    coarse_boxes: CORE_BUILD.stat().coarse_total,
    coarse_low_fill: CORE_BUILD.stat().coarse_low_fill,
    bands_reused: band_pool_stats().reused,
    bands_allocated: band_pool_stats().allocated,
    build_ms: lod_build_stats.build_ms,
    build_runs: lod_build_stats.build_runs,
    scan_ms: lod_build_stats.scan_ms,
    scan_count: lod_build_stats.scan_count,
    built_direct: CORE_BUILD.stat().direct,
    built_scans: CORE_BUILD.stat().scans,
    built_bakes: CORE_BUILD.stat().bakes,
    built_starved: CORE_BUILD.stat().starved,
    badge_metrics: poi_badge_build_stats.metrics,
    badge_builds: poi_badge_build_stats.builds,
    badge_ms: poi_badge_build_stats.ms,
    place_runs: poi_place_stats.runs,
    atlas_repacks: badge_atlas.stats().repacks,
    atlas_uploads: badge_atlas.stats().uploads,
    index_rebuilds: poi_index_stats().rebuilds,
  };
}

type Counters = ReturnType<typeof counters>;

let baseline: Counters = counters();

/** The side counters since the window began. */
export function window_counters(): Counters {
  const now = counters();
  for (const k of Object.keys(now) as (keyof Counters)[]) now[k] -= baseline[k];
  return now;
}

/** Clears the ledger and opens a fresh window, side counters included. */
export function ledger_window_start(): void {
  ledger_recording_start();
  tick_step_stats_reset();
  baseline = counters();
}

const starved = (ms: number) => (ms > STARVED_MS ? " STARVED" : "");

/** Every registered step by mean ms per run, heaviest first. */
function steps_text(): string {
  const rows = tick_step_stats().filter((s) => s.count > 0).sort((a, b) => b.ms / b.count - a.ms / a.count);
  if (rows.length === 0) return "none";
  return rows.map((s) => `${s.name} ${n1(s.ms / s.count)}/${n1(s.max_ms)}`).join("  ");
}

let logging = false;

export function ledger_logging(): boolean {
  return logging;
}

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
      if (missed(s) || overran(s)) {
        console.log(`[ledger] ${frame_line(s)}`);
      }
    },
    long_frame: (l) => console.log(`[ledger] ${long_frame_line(l)}`),
  });
}

function frames_section(frames: readonly FrameSample[]): string[] {
  const f = summarize_frames(frames);
  if (!f) return ["frames n=0"];
  const { last } = f;
  const c = window_counters();
  const atlas = badge_atlas.stats();
  const index = poi_index_stats();
  const retain = chunk_retention();
  const bands = c.bands_reused + c.bands_allocated;
  const per = (total: number, n: number) => (n > 0 ? n1(total / n) : "-");

  return [
    `frames n=${f.n}  over ${n1(f.seconds)}s  moving ${f.moving}  still ${f.n - f.moving}   display ${n1(last.frame_ms)} ms`,
    `  dt      ${spread_text(f.dt)} ms   missed ${f.missed} (${share(f.missed, f.moving)}% of moving frames)`,
    `  task    p50 ${n1(f.task.p50)}  p95 ${n1(f.task.p95)} ms   (main thread's own rAF task)`,
    `  offthd  p50 ${n1(f.offthread.p50)}  p95 ${n1(f.offthread.p95)} ms   (vsync/compositor/GPU/GC)`,
    `  gpu     p50 ${n1(f.gpu.p50)}  p95 ${n1(f.gpu.p95)} ms${last.gpu_ms === 0 ? "   (timer unsupported or unresolved)" : ""}`,
    `  sched   p95 ${n1(f.sched.p95)}  max ${n1(f.sched.max)} ms   overran ${f.overran}   budget ${n1(last.budget_ms)}${last.settled ? " (still)" : ""}   phase p95 ${
      Object.entries(f.phases).map(([k, v]) => `${k} ${n1(v.p95)}`).join("  ")
    }`,
    `  cpu     rck p95 ${n1(f.recheck.p95)}  cmt p95 ${n1(f.commit.p95)} ms`,
    // Per step, so a phase that costs more than its steps should does not
    // hide which step is not yielding: a cooperative scheduler is only as
    // cooperative as its longest step.
    `  steps   ${steps_text()}   (ms per frame, max)`,
    `  gpu     draws ${last.draw_calls}  tris ${Math.round(last.triangles / 1000)}k  tex ${last.textures}  fill ${(last.px / 1e6).toFixed(1)}Mpx x${last.fx_passes}`,
    `  by tag  mean ${census_text(f.census)}   (draws/tris per frame)`,
    `          last ${census_text(census_rows(last.census))}`,
    `  mem     heap ${last.heap_mb}MB (${signed(f.heap_delta)})  tiles ${last.tile_pages} (~${last.tile_pages * TILE_DECODED_MB}MB)  poi ${last.poi_count}`,
    `  place   poi ${
      n1(poi_place_stats.ms)
    } ms over ${c.place_runs} runs   hit ${poi_place_stats.hits} -> cand ${poi_place_stats.candidates} -> kept ${poi_place_stats.kept} (+${poi_place_stats.absorbed} absorbed, biggest ${poi_place_stats.biggest}${
      billboard_dropped() > 0 ? `, DROPPED ${billboard_dropped()}` : ""
    })   max since load ${n1(poi_place_stats.max_ms)} ms, hit ${poi_place_stats.max_hits}`,
    // Ask-to-commit and frames spanned: a placement is a job of the content
    // step now, so this is its latency under the budget, and the max is the
    // starvation question — a content step that waits out every visible
    // build would show here first.
    `  wait    place ${n1(poi_place_stats.wait_ms)} ms over ${poi_place_stats.frames} frames (max ${n1(poi_place_stats.max_wait_ms)} ms, ${poi_place_stats.max_frames} frames)   index ${
      n1(index.wait_ms)
    } ms over ${index.frames} frames (max ${n1(index.max_wait_ms)} ms, ${index.max_frames} frames)`,
    `  field   ${
      (() => {
        const f = poi_field_stats();
        return `${f.live} live of ${f.slots} slots, ${f.visible} visible, ${f.mapped} mapped, group ${f.mounted ? "mounted" : "NOT mounted"}`;
      })()
    }`,
    `  split   sync ${n1(poi_place_stats.sync_ms)} ms (max since load ${n1(poi_place_stats.max_sync_ms)})   badge ${c.badge_metrics} measured / ${c.badge_builds} built ${
      n1(c.badge_ms)
    }ms total, ${poi_badge_count()} cached`,
    `  atlas   ${atlas.size}px  ${atlas.entries} entries  ${atlas.shelves} shelves  ${
      share(atlas.used_px, atlas.size * atlas.size)
    }% used  ${c.atlas_repacks} repacks  ${c.atlas_uploads} rect uploads`,
    `  faces   kept ${c.faces_kept} / ${c.faces_possible} (${share(c.faces_kept, c.faces_possible)}%)  runs culled ${c.runs_culled}/${c.runs_total}`,
    `  coarse  boxes ${c.coarse_boxes}  low fill ${c.coarse_low_fill} (${share(c.coarse_low_fill, c.coarse_boxes)}%; a threshold cull would have skipped these)`,
    `  index   poi ${index.indexed} resident  ${c.index_rebuilds} rebuilds${index.sweeping > 0 ? `  sweeping ${index.sweeping} units` : ""}${index.dropped > 0 ? `  DROPPED ${index.dropped}` : ""}`,
    // Idle inside the core reservation is the working budget, kept on purpose
    // (core_pool.ts); only what is reserved past it is ever handed back.
    `  pool    live ${last.pool.live_mb}MB  idle ${last.pool.idle_mb}MB  reserved ${Math.round(core_pool_stat().reserved_bytes / 1048576)}MB (core ${
      Math.round((CORE_MEMORY?.buffer.byteLength ?? 0) / 1048576)
    }MB kept)  hit ${f.pool_hit_pct ?? "-"}%  biggest ${Math.round(core_pool_stat().max_block_bytes / 1048576)}MB   window scans ${f.work.scans} bakes ${f.work.bakes} runs ${f.work.runs}`,
    // Each section against its own range: the fixed one refuses past it, the
    // others spill to plain regions the collector can take back — a spill or
    // a refusal here is the memory map's own signal, per kind.
    `  section module ${
      (() => {
        return (CORE_BUILD.stat().heap_used / 1048576).toFixed(1);
      })()
    } heap of ${Math.round(MODULE_SECTION.bytes / 1048576)}MB   ${
      core_sections_stat().map(({ name, bytes, stat }) =>
        `${name} ${Math.round(stat.live_bytes / 1048576)}/${Math.round(stat.backing_used / 1048576)} of ${Math.round(bytes / 1048576)}MB${stat.spills > 0 ? ` SPILLED ${stat.spills}` : ""}${
          stat.refusals > 0 ? ` REFUSED ${stat.refusals}` : ""
        }`
      ).join("   ")
    }   (live/cut of range)`,
    `  fetch   queued ${fetch_counts().queued} running ${fetch_counts().running} of ${MAX_INFLIGHT}`,
    // The builds run in the module under the tick; what they did this window.
    `  build   ${c.built_direct} direct  ${c.built_scans} scans  ${c.built_bakes} bakes  ${c.built_starved} refused a block`,
    `  per     commit/bake ${f.commit_per_bake === null ? "-" : n1(f.commit_per_bake)}ms  build/krun ${per(c.build_ms, c.build_runs / 1000)}ms  scan/chunk ${
      per(c.scan_ms, c.scan_count)
    }ms   (workload-normalised; comparable across sessions)`,
    `  rate    ${n1(f.per_s.bakes)} bakes/s  ${n1(f.per_s.scans)} scans/s  ${Math.round(f.per_s.runs / 1000)}k runs/s   while backlog ${n1(f.backlog_per_s.bakes)} bakes/s  ${
      n1(f.backlog_per_s.scans)
    } scans/s over ${n1(f.backlog_s)}s`,
    `  bands   reused ${c.bands_reused} / ${bands} (${share(c.bands_reused, bands)}%)  ~1.31MB each; a miss borrows from the core pool`,
    `  retain  nodes ${retain.nodes}  runsets ${retain.run_sets} (${retain.run_set_mb}MB)  tiers ${retain.tiers}  decor ${retain.decor_steps} steps / ${retain.decor_instances} instances   kept on-screen ${
      Math.round(chunk_retained_bytes().on / 1048576)
    }MB, off-screen ${Math.round(chunk_retained_bytes().off / 1048576)} / ${Math.round(RETAIN_BUDGET_BYTES / 1048576)}MB budget`,
    `  lod     chunks ${last.lod.chunks}  in-frustum ${last.lod.in_frustum}  pending ${last.lod.pending}  settling ${last.lod.settling}  blending ${last.lod.blend_active}`,
    `  wait    releases ${last.lod.offscreen_pending} pending, oldest ${Math.round(last.lod.oldest_release_ms)} ms${starved(last.lod.oldest_release_ms)}   on-screen oldest ${
      Math.round(last.lod.oldest_visible_ms)
    } ms${starved(last.lod.oldest_visible_ms)}`,
    `  mounts  this frame +${last.mounts.added}/-${last.mounts.removed} rerender ${last.mounts.rendered}   window total +${f.mounts.added}/-${f.mounts.removed}`,
    `  queue   terrain ${last.q_terrain}  poi ${last.q_poi}`,
  ];
}

/** The window's worst-seen values, including the LOD gauges the frame
 * summary above only reads off the last frame. */
function peak_section(): string[] {
  const p = ledger_frame_peak();
  return [
    `peak (window)`,
    `  dt ${n1(p.dt_ms)}  sched ${n1(p.sched_ms)}  rck ${n1(p.recheck_ms)}  cmt ${n1(p.commit_ms)} ms`,
    `  draws ${p.draw_calls}  tris ${Math.round(p.triangles / 1000)}k  heap ${p.heap_mb}MB  queue t${p.q_terrain}/p${p.q_poi}`,
    `  lod chunks ${p.chunks}  in-frustum ${p.in_frustum}  pending ${p.pending}  settling ${p.settling}  blending ${p.blend_active}`,
    `  wait releases ${p.offscreen_pending} pending, oldest ${Math.round(p.oldest_release_ms)} ms${starved(p.oldest_release_ms)}   on-screen oldest ${Math.round(p.oldest_visible_ms)} ms${
      starved(p.oldest_visible_ms)
    }`,
  ];
}

function tasks_section(tasks: readonly TaskRecord[]): string[] {
  if (tasks.length === 0) return ["tasks n=0"];
  const lines = [`tasks n=${tasks.length}`];
  for (const t of summarize_tasks(tasks)) {
    const bound = category_info(t.cat)?.bound ?? "?";
    lines.push(
      `  ${pad(t.cat, 9)}${pad(bound, 9)}n=${t.n}   p50 ${n1(t.lat.p50)}  p95 ${n1(t.lat.p95)} ms   ok ${t.ok}  ab ${t.aborted}  err ${t.error}${
        t.reasons.length > 0 ? ` (${t.reasons.map(([r, n]) => `${r} x${n}`).join(", ")})` : ""
      }`,
    );
    if (t.phase_mean) {
      const means = Object.entries(t.phase_mean).map(([k, v]) => `${k.replace(/_ms$/, "")} ${k === "bytes" ? Math.round(v) : n1(v)}`).join("  ");
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
    `=== sched/ledger @ t=${n1(performance.now())}ms  build ${BUILD_NUMBER} ${BUILD_ID} (${BUILD_TIME}) ===`,
    ...frames_section(frames),
    ...(frames.length > 0 ? peak_section() : []),
    ...tasks_section(tasks),
    `long frames n=${longs.length}${loaf_supported ? "" : " (LoAF unsupported; longtask only)"}`,
  ];

  if (tasks.length > 0) {
    out.push(`--- last ${Math.min(RECENT_TASKS, tasks.length)} tasks ---`);
    for (const rec of tasks.slice(-RECENT_TASKS)) out.push(task_line(rec));
  }
  const worst = ledger_worst_frames();
  if (worst.length > 0) {
    out.push(`--- worst ${worst.length} frames in the window ---`);
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
