// @pxl/web/module/world/sched/ledger_report.ts
//
// The ledger's report, written from the numbers the kernel measured
// (crates/pxl/core/src/module/world/sched/report.rs). Every word here is
// this side's: the column titles, the units, and the asides that say what a
// line means. The kernel carries none of them, the way libc rather than the
// kernel carries what an errno reads as.
//
// Pasteable text, sized to land in a findings doc; trace.rs is the
// machine-readable path.

import { loaf_supported } from "./long_frame.ts";

const MB = 1048576;

const n1 = (n: number) => (n ?? 0).toFixed(1);
const round = (n: number) => Math.round(n ?? 0);
const share = (part: number, whole: number) => (whole > 0 ? Math.round((part / whole) * 100) : 0);
const signed = (n: number) => (n >= 0 ? `+${n}` : String(n));

/** Triangles read as thousands past a thousand, as the kernel's own summary
 * rounded them. */
const tris = (t: number) => (t >= 1000 ? `${round(t / 1000)}k` : String(round(t)));

type Spread = { p50: number; p95: number; max: number };
const spread = (s: Spread) => `p50 ${n1(s.p50)}  p95 ${n1(s.p95)}  max ${n1(s.max)}`;

/** A wait long enough to be starvation rather than queueing. The kernel's own
 * threshold rides in the report; this only says so. */
const STARVED_MS = 2000;
const starved = (ms: number) => (ms >= STARVED_MS ? "  STARVED" : "");

// ---- what the kernel sends ----

export type LedgerReport = {
  t: number;
  build: { number: string; id: string; time: string };
  frames: FrameSummary | null;
  counters: Record<string, number>;
  atlas: { size: number; entries: number; shelves: number; used_px: number };
  retain: Record<string, number>;
  fetch: { queued: number; running: number; credits: number };
  poi: PoiStats;
  steps: { phase: string; name: string; ms: number; count: number; max_ms: number }[];
  peak: Peak | null;
  task_n: number;
  tasks: TaskSummary[];
  recent: TaskRecord[];
  worst: { sample: Frame; prev: Frame | null }[];
  longs: LongFrame[];
};

type Frame = {
  t: number;
  dt_ms: number;
  frame_ms: number;
  settled: boolean;
  sched_ms: number;
  budget_ms: number;
  task_ms: number;
  gpu_ms: number;
  tick_ms: number;
  phases: number[];
  draw_calls: number;
  triangles: number;
  buffers: number;
  buffer_mb: number;
  px: number;
  heap_mb: number;
  tile_pages: number;
  poi_count: number;
  textures_mb: { depth: number; msaa: number; shadow: number; layers: number };
  census: [number, number][];
  pool: { live_mb: number; idle_mb: number; hits: number; misses: number };
  mounts: { added: number; removed: number; layouts: number };
  lod: Record<string, number>;
  q_terrain: number;
  q_poi: number;
};

type FrameSummary = {
  n: number;
  seconds: number;
  moving: number;
  missed: number;
  overran: number;
  dt: Spread;
  task: Spread;
  offthread: Spread;
  gpu: Spread;
  sched: Spread;
  tick: Spread;
  phases: { name: string; spread: Spread }[];
  work: [number, number];
  per_s: [number, number];
  backlog_s: number;
  backlog_per_s: [number, number];
  tick_per_bake: number | null;
  mounts: [number, number];
  pool_hit_pct: number | null;
  heap_delta: number;
  census: { tag: string; draws: number; tris: number }[];
  last: Frame;
};

type Peak = Record<string, number>;
type PoiStats = { badge_count: number; dropped: number; place: Record<string, number>; index: Record<string, number> };
type TaskRecord = {
  category: string;
  subject: number;
  priority: number;
  enqueue_ms: number;
  start_ms: number | null;
  end_ms: number;
  transport_ms: number;
  outcome: string;
  fail: number;
  status: number;
  phases: [string, number][];
};
type TaskSummary = {
  category: string;
  n: number;
  ok: number;
  aborted: number;
  error: number;
  lat: Spread;
  reasons: { fail: number; status: number; n: number }[];
  phase_mean: [string, number][] | null;
};
type LongFrame = { source: string; t: number; duration_ms: number; blocking_ms: number; script_ms: number; render_ms: number };

// ---- the words ----

/** Why a fetch failed, as `spec/fetch.ts` numbers it. A refusal says its
 * status instead, which is the part that tells one from another. */
function fail_text(fail: number, status: number): string {
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
    case 0x0302:
      return "bad_subject";
    case 0x0303:
      return "bad_category";
    default:
      return "none";
  }
}

/** Which optimization lever a hotspot in a category needs — set once from
 * profiling, never fed back into scheduling. The measurements behind each are
 * the note beside it; they were taken on the date it names. */
const BOUND: Record<string, { bound: string; note: string }> = {
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

const CENSUS_TAGS = ["chunk", "decor", "ghost_box", "ghost_coplanar", "ghost_decor", "skinned", "puck_zone", "puck_beacon", "badge", "debug"];

const census_text = (rows: { tag: string; draws: number; tris: number }[]) => rows.map((r) => `${r.tag} ${round(r.draws)}/${tris(r.tris)}`).join("  ");

/** One frame's own census, heaviest first — the same order the summary's
 * mean is in, worked out here so the kernel ships the lanes and nothing more. */
function frame_census(c: [number, number][]): string {
  const rows = c.map(([draws, t], i) => ({ tag: CENSUS_TAGS[i] ?? String(i), draws, tris: t })).filter((r) => r.draws > 0 || r.tris > 0);
  rows.sort((a, b) => b.tris - a.tris || b.draws - a.draws);
  return census_text(rows);
}

const phases_text = (p: [string, number][]) =>
  p.length === 0 ? "" : `  [${p.map(([k, v]) => `${k.replace(/_ms$/, "")} ${k === "bytes" ? v : n1(v)}`).join(" ")}]`;

function task_line(r: TaskRecord): string {
  const total = r.end_ms - r.enqueue_ms;
  const timing = r.start_ms === null ? "" : `  q ${n1(r.start_ms - r.enqueue_ms)} + run ${n1(r.end_ms - r.start_ms)}`;
  const why = r.fail === 0 ? "" : `  [${fail_text(r.fail, r.status)}]`;
  return `[${n1(r.enqueue_ms).padStart(9)}] ${`${r.category}:${r.subject}`.padEnd(34)}${r.outcome.padEnd(7)}${n1(total).padStart(8)}ms${timing} p${r.priority}${
    phases_text(r.phases)
  }${why}`;
}

function frame_line(s: Frame, prev: Frame | null): string {
  const d = prev === null
    ? ""
    : `  Δbuf ${signed(s.buffers - prev.buffers)} Δbuf_mb ${signed(s.buffer_mb - prev.buffer_mb)} Δheap ${signed(s.heap_mb - prev.heap_mb)}MB`;
  const offthread = s.dt_ms - s.task_ms;
  return `[${n1(s.t).padStart(9)}] dt ${n1(s.dt_ms)} = task ${n1(s.task_ms)} + offthread ${n1(offthread)}  gpu ${n1(s.gpu_ms)}  sched ${n1(s.sched_ms)}/${
    n1(s.budget_ms)
  }${s.settled ? " (still)" : ""} (tick ${n1(s.tick_ms)})  draws ${s.draw_calls} tris ${tris(s.triangles)}  buf ${s.buffers}  heap ${s.heap_mb}MB  mount +${s.mounts.added}/-${s.mounts.removed} layouts ${s.mounts.layouts}${d}`;
}

const long_frame_line = (l: LongFrame) =>
  `[${n1(l.t).padStart(9)}] ${l.source.padEnd(8)} ${n1(l.duration_ms)} ms  blocking ${n1(l.blocking_ms)}  script ${n1(l.script_ms)}  render ${n1(l.render_ms)}`;

// ---- the sections ----

function frames_section(r: LedgerReport): string[] {
  const f = r.frames;
  if (f === null) return ["frames n=0"];
  const last = f.last;
  const c = r.counters;
  const { atlas, retain, poi, fetch } = r;
  const bands = c.bands_reused + c.bands_allocated;
  const steps = r.steps.length === 0
    ? "none"
    : [...r.steps].sort((a, b) => b.ms / b.count - a.ms / a.count).map((s) => `${s.phase}/${s.name} ${n1(s.ms / s.count)}/${n1(s.max_ms)}`).join("  ");

  return [
    `frames n=${f.n}  over ${n1(f.seconds)}s  moving ${f.moving}  still ${f.n - f.moving}   display ${n1(last.frame_ms)} ms`,
    `  dt      ${spread(f.dt)} ms   missed ${f.missed} (${share(f.missed, f.moving)}% of moving frames)`,
    `  task    p50 ${n1(f.task.p50)}  p95 ${n1(f.task.p95)} ms   (main thread's own rAF task)`,
    `  offthd  p50 ${n1(f.offthread.p50)}  p95 ${n1(f.offthread.p95)} ms   (vsync/compositor/GPU/GC)`,
    `  gpu     p50 ${n1(f.gpu.p50)}  p95 ${n1(f.gpu.p95)} ms${last.gpu_ms === 0 ? "   (no renderer yet)" : ""}`,
    `  sched   p95 ${n1(f.sched.p95)}  max ${n1(f.sched.max)} ms   overran ${f.overran}   budget ${n1(last.budget_ms)}${
      last.settled ? " (still)" : ""
    }   phase p95 ${f.phases.map((p) => `${p.name} ${n1(p.spread.p95)}`).join("  ")}`,
    `  cpu     tick p95 ${n1(f.tick.p95)} ms   (the kernel's tick inside the visible phase)`,
    // Per step, so a phase that costs more than its steps should does not hide
    // which step is not yielding: a cooperative scheduler is only as
    // cooperative as its longest step.
    `  steps   ${steps}   (ms per frame, max)`,
    `  gpu     draws ${last.draw_calls}  tris ${round(last.triangles / 1000)}k  buffers ${last.buffers} ${last.buffer_mb}MB  fill ${(last.px / 1e6).toFixed(1)}Mpx`,
    `  targets depth ${last.textures_mb.depth}MB  msaa ${last.textures_mb.msaa}MB  shadow ${last.textures_mb.shadow}MB  layers ${last.textures_mb.layers}MB`,
    `  by tag  mean ${census_text(f.census)}   (draws/tris per frame)`,
    `          last ${frame_census(last.census)}`,
    `  mem     heap ${last.heap_mb}MB (${signed(f.heap_delta)})  tiles ${last.tile_pages}  poi ${last.poi_count}`,
    `  place   poi ${n1(poi.place.ms)} ms over ${c.place_runs} runs   hit ${poi.place.hits} -> cand ${poi.place.candidates} -> kept ${poi.place.kept} (+${poi.place.absorbed} absorbed, biggest ${poi.place.biggest}${
      poi.dropped > 0 ? `, DROPPED ${poi.dropped}` : ""
    })   max since load ${n1(poi.place.max_ms)} ms, hit ${poi.place.max_hits}`,
    // Ask-to-commit and frames spanned: a placement is a job of the content
    // step, so this is its latency under the budget, and the max is the
    // starvation question.
    `  wait    place ${n1(poi.place.wait_ms)} ms over ${poi.place.frames} frames (max ${n1(poi.place.max_wait_ms)} ms, ${poi.place.max_frames} frames)   index ${
      n1(poi.index.wait_ms)
    } ms over ${poi.index.frames} frames (max ${n1(poi.index.max_wait_ms)} ms, ${poi.index.max_frames} frames)`,
    `  split   sync ${n1(poi.place.sync_ms)} ms (max since load ${n1(poi.place.max_sync_ms)})   badge ${c.badge_metrics} measured / ${c.badge_builds} built ${
      n1(c.badge_ms)
    }ms total, ${poi.badge_count} cached`,
    `  atlas   ${atlas.size}px  ${atlas.entries} entries  ${atlas.shelves} shelves  ${share(atlas.used_px, atlas.size * atlas.size)}% used  ${c.atlas_repacks} repacks  ${c.atlas_uploads} rect uploads`,
    `  faces   kept ${c.faces_kept} / ${c.faces_possible} (${share(c.faces_kept, c.faces_possible)}%)  runs culled ${c.runs_culled}/${c.runs_total}`,
    `  coarse  boxes ${c.coarse_boxes}  low fill ${c.coarse_low_fill} (${
      share(c.coarse_low_fill, c.coarse_boxes)
    }%; a threshold cull would have skipped these)`,
    `  index   poi ${poi.index.indexed} resident  ${c.index_rebuilds} rebuilds${poi.index.sweeping > 0 ? `  sweeping ${poi.index.sweeping} units` : ""}${
      poi.index.dropped > 0 ? `  DROPPED ${poi.index.dropped}` : ""
    }`,
    // Idle inside the core reservation is the working budget, kept on purpose.
    `  pool    live ${last.pool.live_mb}MB  idle ${last.pool.idle_mb}MB  hit ${f.pool_hit_pct === null ? "-" : f.pool_hit_pct}%   window scans ${f.work[0]} bakes ${
      f.work[1]
    }`,
    `  fetch   queued ${fetch.queued} running ${fetch.running} of ${fetch.credits}`,
    // The builds run in the kernel under the tick; what they did this window.
    `  build   ${c.built_direct} direct  ${c.built_scans} scans  ${c.built_bakes} bakes  ${c.built_starved} refused a block${
      c.decor_dropped > 0 ? `   DECOR DROPPED ${c.decor_dropped}` : ""
    }`,
    `  per     tick/bake ${f.tick_per_bake === null ? "-" : n1(f.tick_per_bake)}ms   (workload-normalised; comparable across sessions)`,
    `  rate    ${n1(f.per_s[1])} bakes/s  ${n1(f.per_s[0])} scans/s   while backlog ${n1(f.backlog_per_s[1])} bakes/s  ${n1(f.backlog_per_s[0])} scans/s over ${
      n1(f.backlog_s)
    }s`,
    `  bands   reused ${c.bands_reused} / ${bands} (${share(c.bands_reused, bands)}%)  ~1.31MB each; a miss borrows from the core pool`,
    `  retain  nodes ${retain.nodes}  runsets ${retain.run_sets} (${retain.run_set_mb}MB)  tiers ${retain.tiers}  decor ${retain.decor_steps} steps ${retain.decor_instances} instances   kept on-screen ${
      round(retain.on_bytes / MB)
    }MB, off-screen ${round(retain.off_bytes / MB)} / ${round(retain.budget_bytes / MB)}MB budget`,
    `  lod     chunks ${last.lod.chunks}  in-frustum ${last.lod.in_frustum}  pending ${last.lod.pending}  settling ${last.lod.settling}  blending ${last.lod.blend_active}  pressure ${last.lod.pressure}`,
    `  wait    releases ${last.lod.offscreen_pending} pending, oldest ${round(last.lod.oldest_release_ms)} ms${
      starved(last.lod.oldest_release_ms)
    }   on-screen oldest ${round(last.lod.oldest_visible_ms)} ms${starved(last.lod.oldest_visible_ms)}`,
    `  mounts  this frame +${last.mounts.added}/-${last.mounts.removed} layouts ${last.mounts.layouts}   window total +${f.mounts[0]}/-${f.mounts[1]}`,
    `  queue   terrain ${last.q_terrain}  poi ${last.q_poi}`,
  ];
}

/** The window's worst-seen values, including the LOD gauges the frame summary
 * only reads off the last frame. */
function peak_section(p: Peak): string[] {
  return [
    "peak (window)",
    `  dt ${n1(p.dt_ms)}  sched ${n1(p.sched_ms)}  tick ${n1(p.tick_ms)} ms`,
    `  draws ${p.draw_calls}  tris ${round(p.triangles / 1000)}k  heap ${p.heap_mb}MB  queue t${p.q_terrain}/p${p.q_poi}`,
    `  lod chunks ${p.chunks}  in-frustum ${p.in_frustum}  pending ${p.pending}  settling ${p.settling}  blending ${p.blend_active}  pressure ${p.pressure}`,
    `  wait releases ${p.offscreen_pending} pending, oldest ${round(p.oldest_release_ms)} ms${starved(p.oldest_release_ms)}   on-screen oldest ${
      round(p.oldest_visible_ms)
    } ms${starved(p.oldest_visible_ms)}`,
  ];
}

function tasks_section(r: LedgerReport): string[] {
  if (r.task_n === 0) return ["tasks n=0"];
  const lines = [`tasks n=${r.task_n}`];
  for (const t of r.tasks) {
    const bound = BOUND[t.category]?.bound ?? "?";
    const reasons = t.reasons.length === 0 ? "" : ` (${t.reasons.map((x) => `${fail_text(x.fail, x.status)} x${x.n}`).join(", ")})`;
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

/** The whole instrument as pasteable text: aggregate summary, then the raw
 * tail of each ring. */
export function ledger_report_text(r: LedgerReport): string {
  const out = [`=== sched/ledger @ t=${n1(r.t)}ms  build ${r.build.number} ${r.build.id} (${r.build.time}) ===`];
  out.push(...frames_section(r));
  if (r.peak !== null) out.push(...peak_section(r.peak));
  out.push(...tasks_section(r));
  out.push(`long frames n=${r.longs.length}${loaf_supported() ? "" : " (LoAF unsupported; longtask only)"}`);

  if (r.recent.length > 0) {
    out.push(`--- last ${r.recent.length} tasks ---`);
    for (const rec of r.recent) out.push(task_line(rec));
  }
  if (r.worst.length > 0) {
    out.push(`--- worst ${r.worst.length} frames in the window ---`);
    for (const w of r.worst) out.push(frame_line(w.sample, w.prev));
  }
  if (r.longs.length > 0) {
    out.push("--- long frames ---");
    for (const l of r.longs) out.push(long_frame_line(l));
  }
  return out.join("\n");
}
