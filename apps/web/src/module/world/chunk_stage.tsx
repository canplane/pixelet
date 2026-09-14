// @pxl/web/module/world/chunk_stage.tsx

/**
 * Unified terrain/object/decor renderer.
 *
 * One Chunk component represents one renderer-owned L2 XY column and all
 * occupied signed 64-high L2 bands beneath it — AND, as of 2026-09-05, every
 * decor kind's own instances for that same column too. A temporary dense
 * signed-Z field is composited synchronously, converted to a baked mesh, and
 * immediately returned to the scratch pool. In-frustum chunks retain a
 * 36 KiB top-surface summary for coarse LOD rebuilds, never the dense bands.
 *
 * Decor used to be a fully separate, independently-scheduled component tree
 * (decor_layer.ts's own ChunkDecor/PageDecor/DecorLayer, one registrant per
 * (chunk, decor kind) in the shared scheduler below) — retired
 * once Chunk absorbed decor's own build into its own commit(), the fix for
 * the documented terrain/decor ready-state desync (decor could render a
 * frame before its own co-located terrain). See decor_layer.ts's own top
 * doc comment and issue/20260905-tier1-geometry-baking-implemented.md
 * (and its own same-day follow-up) for the fuller account.
 */

import { memo, type ReactNode, useLayoutEffect, useRef } from "react";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { fvec, type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useSettingsStore } from "@/app/setting/store.ts";
import { useMapStore } from "@/module/map/store.ts";

import { chunk_nodes, ChunkNode, lod_build_stats, RETAIN_BUDGET_BYTES, take_node_counts } from "@/module/world/chunk_node.ts";
import { CORE_BUILD, FRAME } from "@/render/layer/pipeline/core_build.ts";
import { world_wakers, WorldTopic } from "@/module/world/change.ts";
import { tile_pages } from "@/module/world/terrain/store.ts";
import { poi_pages } from "@/module/world/poi/store.ts";
import { fetch_pending_count } from "@/module/world/sched/fetch_scheduler.ts";
import { ledger_last_frame, ledger_record_frame } from "@/module/world/sched/ledger.ts";
import { HARD_SLACK_MS, useTick } from "@/module/world/sched/tick.ts";
import { lead_point } from "@/module/world/sched/priority.ts";
import { gpu_frame, gpu_renderer } from "@/render/gpu/stage.tsx";
import { view, view_resize } from "@/module/world/view.ts";
import { range_anchor_of, RANGE_HYSTERESIS } from "@/render/chunk_range.ts";
import { render_cfg } from "@/module/world/environment/config.ts";
import { set_lod_projection } from "@/render/lod.ts";
import { core_pool_stat } from "@/render/layer/pipeline/core_pool.ts";

/* The per-pass budgets that used to sit here — a soft allowance per pass and
 * a 16ms hard ceiling each — are gone. Both passes now draw on the one
 * FrameBudget the tick hands every phase (sched/tick.ts), so the frame's
 * allowance is shared rather than granted twice, and the ceiling is slack
 * over that pool rather than a second constant. The 4711ms frame the hard
 * ceiling was measured against is recorded in frame_budget.ts. */

/* Session-peak scheduler load is sched/ledger.ts's ledger_frame_peak():
 * this loop reports each frame's raw numbers and the ledger keeps both the
 * ring and the window peak, which the report prints. A window peak is what
 * catches a stutter, since a reading taken by hand during one lands a frame
 * or two after the storm has passed. */

/* The render itself is not timed by wrapping the renderer's frame: the
 * whole rAF task, measured from a microtask below, is what covers it and
 * everything else the frame ran. */
/** Previous frame's whole-rAF-task duration; see FrameSample.task_ms. */
let last_task_ms = 0;

const MB = 1_048_576;

const last_work = { scans: 0, bakes: 0, runs: 0 };

function work_sample(): { scans: number; bakes: number; runs: number } {
  const sample = {
    scans: lod_build_stats.scan_count - last_work.scans,
    bakes: lod_build_stats.build_count - last_work.bakes,
    runs: lod_build_stats.build_runs - last_work.runs,
  };
  last_work.scans = lod_build_stats.scan_count;
  last_work.bakes = lod_build_stats.build_count;
  last_work.runs = lod_build_stats.build_runs;
  return sample;
}

/** The core pool's gauges and its cumulative counters; a hit share is a
 * difference over a window, taken by summary.ts. */
function pool_sample(): { live_mb: number; idle_mb: number; hits: number; misses: number } {
  const stat = core_pool_stat();
  return { live_mb: Math.round(stat.live_bytes / MB), idle_mb: Math.round(stat.idle_bytes / MB), hits: stat.hits, misses: stat.misses };
}

/** The flag lane's census for the ledger, in one pass: how many are on
 * screen, settling, blending, and who is waiting and for how long — the
 * progress rule guarantees a step a frame, not which one, so the waits are
 * what show whether something is being passed over indefinitely. */
function lod_census(): { in_frustum: u32; settling: u32; blend_active: u32; offscreen_pending: u32; oldest_release_ms: f64; oldest_visible_ms: f64 } {
  const c = CORE_BUILD.census();
  return {
    in_frustum: c.in_frustum,
    settling: c.settling,
    blend_active: c.blend_active,
    offscreen_pending: c.offscreen_pending,
    oldest_release_ms: c.oldest_release_ms,
    oldest_visible_ms: c.oldest_visible_ms,
  };
}

/** The module's mounts: a node for each chunk it mounts, gone with it. */
function mount_chunk(id: u32, page_idx: IVec, base_x: u32, base_y: u32): void {
  new ChunkNode({ page_idx, offset: { key: `${base_x},${base_y}`, base_x, base_y }, id });
}

/**
 * Runs the scheduler's passes over lod_scheduler's lanes — mounted once
 * above ChunkStage (world.tsx). A ChunkNode is the only kind of slot now:
 * decor used to register its own ChunkDecor per (chunk, decor kind) here
 * too, competing for this same budget as an independent component with its
 * own `ready` state; as of 2026-09-05 a node builds decor alongside its own
 * cube content in one commit() instead (see decor_layer.ts's own top doc
 * comment). Everything here — the passes, the frustum, the dev stats — used
 * to live inside ChunkStage itself; none of it is terrain-specific, so it
 * moved out rather than ChunkStage keeping ownership of something it no
 * longer exclusively uses.
 */
export function LodSchedulerProvider({ children }: { children: ReactNode }) {
  const lead = useRef({ x: 0, y: 0, z: 0 }).current;

  // What the visible phase measured, handed to the report phase below. A
  // ref rather than state: it is written and read inside one tick.
  const visible_pass = useRef<
    {
      pending_total: u32;
      committed: u32;
      recheck_ms: number;
      commit_ms: number;
    } | null
  >(null);

  useTick("visible", ({ state, delta, budget }) => {
    visible_pass.current = null;
    view_resize(state.size.width, state.size.height);
    // The tier criterion is a screen-space one (render/lod.ts), and the
    // other layers that read it (decor, badges) read it from here; the
    // module takes the same projection through the frame.
    set_lod_projection(view.height, view.fov);

    // Where the camera is heading — see sched/priority.ts. The module orders
    // the work by it; the tier each chunk picks still comes from the camera
    // as drawn.
    const target = useMapStore.getState().map_center;
    const origin = view.origin;
    if (target) {
      const local = ivec.sub(target.i, origin);
      lead_point(
        lead,
        view.eye,
        ivec.to_real(ivec.X(local)) + fvec.to_real(fvec.X(target.f)),
        ivec.to_real(ivec.Y(local)) + fvec.to_real(fvec.Y(target.f)),
        ivec.to_real(ivec.Z(local)) + fvec.to_real(fvec.Z(target.f)),
        view.root,
      );
    } else {
      lead.x = view.eye.x;
      lead.y = view.eye.y;
      lead.z = view.eye.z;
    }

    // The frame, written where the module reads it (core_build.ts's FRAME),
    // then the module's pass over every chunk: measure, recheck, commit,
    // build and reclaim, under what is left of the budget. One matrix for
    // every chunk: the root frame's, which every chunk is a pure
    // translation inside.
    const f = CORE_BUILD.frame;
    f.set(view.root, FRAME.root);
    f[FRAME.origin] = ivec.to_real(ivec.X(origin));
    f[FRAME.origin + 1] = ivec.to_real(ivec.Y(origin));
    f[FRAME.origin + 2] = ivec.to_real(ivec.Z(origin));
    f.set(view.planes, FRAME.planes);
    f[FRAME.camera] = view.eye.x;
    f[FRAME.camera + 1] = view.eye.y;
    f[FRAME.camera + 2] = view.eye.z;
    f[FRAME.lead] = lead.x;
    f[FRAME.lead + 1] = lead.y;
    f[FRAME.lead + 2] = lead.z;
    f[FRAME.viewport_h] = view.height;
    f[FRAME.fov_deg] = view.fov;
    f[FRAME.delta_s] = delta;
    f[FRAME.budget_ms] = budget.remaining();
    f[FRAME.hard_slack_ms] = HARD_SLACK_MS;
    f[FRAME.pixel_dither] = useSettingsStore.getState().pixel_dither ? 1 : 0;
    f[FRAME.retain_budget_bytes] = RETAIN_BUDGET_BYTES;
    // The range the module mounts chunks within, from the chunk the map
    // centre is in: render/chunk_range.ts's anchor, radius and hysteresis.
    const range_anchor = target ? range_anchor_of(target.i) : null;
    f[FRAME.has_anchor] = range_anchor === null ? 0 : 1;
    f[FRAME.anchor_x] = range_anchor === null ? 0 : ivec.to_real(ivec.X(range_anchor));
    f[FRAME.anchor_y] = range_anchor === null ? 0 : ivec.to_real(ivec.Y(range_anchor));
    f[FRAME.render_dist] = render_cfg.CHUNK_RENDER_DIST;
    f[FRAME.range_hysteresis] = RANGE_HYSTERESIS;
    f[FRAME.show_decor] = useSettingsStore.getState().show_decor ? 1 : 0;

    const t0 = import.meta.env.DEV ? performance.now() : 0;
    const draw = CORE_BUILD.tick();
    const built = CORE_BUILD.stat();
    lod_build_stats.scan_count = built.scans + built.direct;
    lod_build_stats.direct_count = built.direct;
    lod_build_stats.build_count = built.bakes;
    gpu_frame(state.viewport.dpr);
    if (draw) state.invalidate();
    if (import.meta.env.DEV) {
      const c = CORE_BUILD.census();
      visible_pass.current = { pending_total: c.pending, committed: c.committed, recheck_ms: 0, commit_ms: performance.now() - t0 };
    }
  }, "chunks");

  // The ledger, after every other phase has run, so `sched_ms` is the whole
  // tick and not this component's share of it.
  useTick("report", ({ delta, budget, phase_ms, frame_ms: display_ms, settled }) => {
    if (!import.meta.env.DEV) return;
    const pass = visible_pass.current;
    if (!pass) {
      // No chunks this frame — a search jump empties the stage for a moment.
      return;
    }
    const { pending_total, committed, recheck_ms, commit_ms } = pass;
    {
      const pending_lod_total_cnt = pending_total - committed;
      const frame_t0 = budget.t0;
      const frame_ms = performance.now() - frame_t0;

      // Report this frame to sched/ledger.ts BEFORE reading the peak below —
      // the ledger owns both views now (the ring and the window peak), so
      // this loop only has to hand over raw numbers.
      const perf_memory = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory;
      let poi_count = 0;
      for (const resident of poi_pages.handles) poi_count += resident.page.len;
      const gpu = gpu_renderer;
      // Reads and clears in one step, so the next frame starts from zero
      // without a separate reset pass to forget.
      const node_work = take_node_counts();
      ledger_record_frame({
        t: performance.now(),
        dt_ms: delta * 1000,
        frame_ms: display_ms,
        settled,
        sched_ms: frame_ms,
        budget_ms: budget.total_ms,
        phases: { ...phase_ms },
        cpu: { recheck_ms, commit_ms },
        // Filled by the microtask below, which runs once the rAF task's JS
        // stack empties — i.e. after every other step and the render.
        // Carries the previous frame's value here and
        // is corrected in place a moment later.
        task_ms: last_task_ms,
        gpu_ms: gpu?.stat.gpu_ms ?? 0,
        draw_calls: gpu?.stat.draws ?? 0,
        triangles: gpu?.stat.triangles ?? 0,
        buffers: gpu?.stat.buffers ?? 0,
        buffer_mb: gpu ? Math.round(gpu.stat.bytes / MB) : 0,
        textures_mb: {
          depth: gpu ? +(gpu.stat.texture_bytes.depth / MB).toFixed(1) : 0,
          msaa: gpu ? +(gpu.stat.texture_bytes.msaa / MB).toFixed(1) : 0,
          shadow: gpu && gpu.shadows ? +(gpu.stat.texture_bytes.shadow / MB).toFixed(1) : 0,
          layers: gpu ? +(gpu.stat.texture_bytes.layers / MB).toFixed(1) : 0,
        },
        census: gpu?.census ?? {},
        px: gpu ? gpu.canvas.width * gpu.canvas.height : 0,
        mounts: { added: node_work.added, removed: node_work.removed, rendered: node_work.rebuilt },
        lod: {
          chunks: chunk_nodes.size,
          pending: pending_lod_total_cnt,
          ...lod_census(),
        },
        heap_mb: perf_memory ? Math.round(perf_memory.usedJSHeapSize / 1_048_576) : 0,
        tile_pages: tile_pages.size,
        poi_count,
        pool: pool_sample(),
        work: work_sample(),
        q_terrain: fetch_pending_count("terrain"),
        q_poi: fetch_pending_count("poi"),
      });
      // Measure the whole rAF task once the stack empties: a microtask is
      // the vantage point that sees everything the frame ran.
      const sample = ledger_last_frame();
      queueMicrotask(() => {
        last_task_ms = performance.now() - frame_t0;
        if (sample) sample.task_ms = last_task_ms;
      });
    }
  }, "ledger");

  return <>{children}</>;
}

export type ChunkStageProps = Record<string, never>;

/**
 * Tells the module which pages have a terrain tile, and makes and drops the
 * nodes of the chunks it mounts from them. Everything else — which chunks,
 * when — is the module's, from the range anchor the visible pass writes
 * into the frame. Decor models are not passed in: decor/asset_store.ts owns
 * them and every ChunkNode subscribes to it directly.
 */
export const ChunkStage = memo((_: ChunkStageProps) => {
  useLayoutEffect(() => {
    CORE_BUILD.mounts = {
      on_mount: (id, page_idx, base_x, base_y) => mount_chunk(id, page_idx, base_x, base_y),
      on_unmount: () => {},
    };
    for (const page_idx of tile_pages.keys()) CORE_BUILD.page(page_idx, true);
    const off = world_wakers.subscribe_topic(WorldTopic.TERRAIN_PAGE, (key) => {
      if (key === null) {
        for (const page_idx of tile_pages.keys()) CORE_BUILD.page(page_idx, true);
        return;
      }
      const page_idx = BigInt.asUintN(64, key);
      CORE_BUILD.page(page_idx, tile_pages.has(page_idx));
    });
    // The stage leaving takes its pages out of the range, and the module
    // unmounts their chunks on the next tick. Nothing is disposed here: a
    // StrictMode replay puts every page back before that tick, and the
    // module then has nothing to unmount and nothing to mount again.
    return () => {
      off();
      for (const page_idx of tile_pages.keys()) CORE_BUILD.page(page_idx, false);
    };
  }, []);
  return null;
});
ChunkStage.displayName = "ChunkStage";
