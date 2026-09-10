// @pxl/web/modules/world/chunk_stage.tsx

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
 * (chunk, decor kind) in the shared LodSchedulerRegistry below) — retired
 * once Chunk absorbed decor's own build into its own commit(), the fix for
 * the documented terrain/decor ready-state desync (decor could render a
 * frame before its own co-located terrain). See decor_layer.ts's own top
 * doc comment and discussions/20260905-tier1-geometry-baking-implemented.md
 * (and its own same-day follow-up) for the fuller account.
 */

import { memo, type ReactNode, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import { useFrame, useThree } from "@react-three/fiber";
import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { FROM_L3_IDX, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { useAnchorRegistry } from "@/modules/world/anchor/context.tsx";
import {
  ChunkNode,
  commit_step_stats,
  lod_build_stats,
  lod_scheduler,
  LOD_WORK_BUDGET_MS,
  mesh_mount_stats,
  mesh_unmount_stats,
  record_timing,
  take_node_counts,
  type TimingStats,
} from "@/modules/world/chunk_node.ts";
import { AnchorNode } from "@/modules/world/anchor/node.tsx";
import { decor_field } from "@/modules/world/decor/field.ts";
import { RENDER_RADIUS } from "@/modules/world/terrain/config.ts";
import { useTileStore } from "@/modules/world/terrain/store.ts";
import { CHEBYSHEV_DIST } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { usePoiStore } from "@/modules/world/poi/store.ts";
import { fetch_pending_count } from "@/modules/world/sched/fetch_scheduler.ts";
import { run_under_budget } from "@/modules/world/sched/frame_budget.ts";
import { gpu_timer_attach, gpu_timer_begin, gpu_timer_end, gpu_timer_last_ms } from "@/modules/world/sched/gpu_timer.ts";
import { ledger_frame_peak, ledger_last_frame, ledger_record_frame } from "@/modules/world/sched/ledger.ts";
import { in_range_chunks } from "@/render/chunk_range.ts";
import { compositor_cache_stats } from "@/render/layer/pipeline/compositor.ts";
import type { WorldLayerStack } from "@/render/layer/pipeline/contract.ts";
import { coarse_instance_stats, occlusion_stats } from "@/render/layer/pipeline/chunk_mesher.ts";
import { mesh_pool_stat } from "@/render/layer/pipeline/mesh_pool.ts";
import { update_frustum } from "@/render/chunk_lod.ts";

const LOD_RECHECK_INTERVAL: f64 = 0.15;
const LOD_MAX_CHUNKS_PER_FRAME: u32 = 8;
/**
 * Ceiling on either LOD pass, applied even when the pass has committed
 * nothing — one frame's worth, so a pass can never itself be the stutter.
 *
 * Measured before this existed: a zoom-out leaves ~2000 chunks pending, most
 * `commit()` calls scan (up to ~15ms) and then defer, and a deferral returns
 * false. Since LOD_WORK_BUDGET_MS was waived until something actually
 * committed, thousands of expensive deferrals armed nothing and produced a
 * single 4711ms frame — while no individual step exceeded 42ms. Draining
 * across frames is the correct behaviour; freezing for 4.7s is not.
 */
const LOD_HARD_BUDGET_MS: f64 = 16;

/* Session-peak scheduler load used to live here as its own object. It now
 * comes from sched/ledger.ts's ledger_frame_peak(): this loop reports each
 * frame's raw numbers and the ledger keeps both the ~4s ring and the session
 * peak, instead of two places tracking the same maxima. The reason the peak
 * exists at all is unchanged — a manual __PXL_RENDER_STATS__ read during a
 * live stutter lands a frame or two after the storm has passed. */

/**
 * The actual `gl.render(scene, camera)` call — three.js walking the scene
 * graph and issuing WebGL draw calls — happens in R3F's own frame loop,
 * *after* every useFrame callback above returns. Nothing in
 * scheduler_peak_stats/commit_step_stats covers it. Measured by wrapping
 * gl.render itself (see ChunkStage's effect below), not by tracing — every
 * DevTools Performance capture this session was dominated 80-90%+ by the
 * recording's own async-task-tracking overhead even in the single busiest
 * window, not real work, so a plain performance.now() wrapper is the only
 * trustworthy way left to see this number.
 */
const render_call_stats: TimingStats = { total_ms: 0, count: 0, max_ms: 0 };
// Last single gl.render() duration, for sched/ledger.ts's per-frame CPU
// breakdown — the cumulative render_call_stats can't give one frame's number.
let last_render_ms = 0;
/** Previous frame's whole-rAF-task duration; see FrameSample.task_ms. */
let last_task_ms = 0;

/* Draw counts are read with `gl.info.autoReset` turned off and reset once
 * per frame instead (see LodSchedulerProvider's effect). three.js otherwise
 * clears info at the start of every `render()`, and the post-processing
 * composer issues several per frame — the last being a fullscreen quad — so
 * reading `gl.info.render` from useFrame reported that quad (`draws 1,
 * tris 1`) while the scene was drawing hundreds. It read plausibly enough to
 * survive this whole session until it was checked against a screenshot.
 * Resetting per frame accumulates every pass, which is the number actually
 * wanted: what this frame costs in draw calls, post stack included. */

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

function pool_sample(): { live_mb: number; idle_mb: number; hit_pct: number } {
  const stat = mesh_pool_stat();
  const acquires = stat.hits + stat.misses;
  return {
    live_mb: Math.round(stat.live_bytes / MB),
    idle_mb: Math.round(stat.idle_bytes / MB),
    hit_pct: acquires > 0 ? Math.round((stat.hits / acquires) * 100) : 0,
  };
}

type PageChunksProps = { page_idx: IVec; stack: WorldLayerStack; range_anchor: IVec };

/**
 * One page's worth of chunks, created and destroyed imperatively.
 *
 * This is where React stops. A page is a handful of components; its ~256
 * chunks are ChunkNodes (chunk_node.ts), plain objects hung off one group.
 * The rendered tree used to reach all the way down — a component per chunk,
 * per LOD role, and per decor variant, thousands of them re-reconciled
 * whenever a layout changed — which bought nothing (a chunk is described by
 * no props and reads no other chunk's state) and cost the deferral that made
 * chunk memory unsafe to reason about.
 *
 * `offsets` changes only when the page or the range anchor does, so the diff
 * below is normally empty; when it does change it is a set difference by
 * chunk key, not a reconciliation.
 */
const PageChunks = memo(({ page_idx, stack, range_anchor }: PageChunksProps) => {
  const page_base = FROM_L3_IDX(page_idx);
  const camera = useThree((state) => state.camera);
  const anchor_registry = useAnchorRegistry();
  const group = useMemo(() => {
    const g = new THREE.Group();
    g.name = `chunk-stage-page/${ivec.repr(page_idx)}`;
    return g;
  }, [page_idx]);
  const nodes = useRef(new Map<string, ChunkNode>());
  const offsets = useMemo(() => in_range_chunks(page_base, range_anchor), [page_base, range_anchor]);

  useLayoutEffect(() => {
    const live = nodes.current;
    const wanted = new Map(offsets.map((offset) => [offset.key, offset]));
    for (const [key, node] of live) {
      if (wanted.has(key)) continue;
      node.dispose();
      live.delete(key);
    }
    for (const [key, offset] of wanted) {
      if (live.has(key)) continue;
      const node = new ChunkNode({ page_idx, offset, stack, camera, anchor_registry });
      group.add(node.group);
      live.set(key, node);
    }
  }, [offsets, page_idx, stack, camera, anchor_registry, group]);

  useEffect(() => {
    const live = nodes.current;
    return () => {
      for (const node of live.values()) node.dispose();
      live.clear();
    };
  }, []);

  return (
    <AnchorNode i={page_base}>
      <primitive object={group} />
    </AnchorNode>
  );
});
PageChunks.displayName = "PageChunks";

/**
 * Owns the one LodSchedulerRegistry chunk_stage.tsx's own Chunk registers
 * into as a ChunkLodControl — mounted once above ChunkStage (world.tsx).
 * Chunk is the only registrant now: decor used to register its own separate
 * ChunkDecor per (chunk, decor kind) here too, competing for this same
 * budget as an independent component with its own `ready` state; as of
 * 2026-09-05 Chunk builds decor alongside its own cube content in one
 * commit() instead (see decor_layer.ts's own top doc comment), so this
 * registry now only ever holds one control per chunk position, not up to
 * five. Everything here — the registry, the frustum, the recheck/commit
 * loop, the dev stats — used to live inside ChunkStage itself, back when it
 * was the only registrant; none of it is terrain-specific, so it moved out
 * rather than ChunkStage keeping ownership of something it no longer
 * exclusively uses.
 */
export function LodSchedulerProvider({ children }: { children: ReactNode }) {
  const scheduler = lod_scheduler;
  const lod_elapsed = useRef(LOD_RECHECK_INTERVAL);
  const frustum = useMemo(() => new THREE.Frustum(), []);
  const { gl } = useThree();
  const anchor_registry = useAnchorRegistry();

  // Decor hangs off the anchor root rather than off any one chunk or page:
  // it is one field for the whole world (decor/field.ts), and the root is the
  // node whose frame every anchor group is already expressed in.
  useLayoutEffect(() => {
    anchor_registry.root.add(decor_field.group);
    return () => {
      anchor_registry.root.remove(decor_field.group);
    };
  }, [anchor_registry]);

  useEffect(() => {
    if (!import.meta.env.DEV) return;
    const original_render = gl.render.bind(gl);
    gl.render = (scene, camera) => {
      const t0 = performance.now();
      original_render(scene, camera);
      last_render_ms = performance.now() - t0;
      record_timing(render_call_stats, last_render_ms);
    };
    return () => {
      gl.render = original_render;
    };
  }, [gl]);

  // GPU timing needs the live context; harmless when the extension is absent.
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    const ctx = gl.getContext();
    if (ctx instanceof WebGL2RenderingContext) gpu_timer_attach(ctx);
  }, [gl]);

  // Accumulate renderer.info across every pass of a frame rather than letting
  // each render() clear it — see the note above frame draw counts.
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    gl.info.autoReset = false;
    return () => {
      gl.info.autoReset = true;
    };
  }, [gl]);
  const projection = useMemo(() => new THREE.Matrix4(), []);

  useFrame((state, delta) => {
    const frame_t0 = import.meta.env.DEV ? performance.now() : 0;
    if (import.meta.env.DEV) gpu_timer_begin();
    const controls = Array.from(scheduler.controls);
    if (controls.length === 0) return;
    lod_elapsed.current += delta;
    const full_recheck = lod_elapsed.current >= LOD_RECHECK_INTERVAL;
    if (full_recheck) lod_elapsed.current = 0;
    update_frustum(frustum, state.camera, projection);

    const recheck_t0 = import.meta.env.DEV ? performance.now() : 0;
    // Same cooperative shape as the commit loop below: the first eligible
    // control this frame always runs (forward progress guaranteed even on
    // a slow frame), the rest are time-boxed. Profiled: during a fast zoom,
    // settling_lod/blend_active can both spike into the dozens at once, and
    // recheck()-ing every one of them every single frame (previously
    // unconditional) cost up to ~23ms by itself — a stale blend/settling
    // flag for one skipped frame is a much smaller visual cost than that.
    //
    // Sorted, not left in whatever order controls happened to register into
    // the Set — plain registration order has no relationship to real 3D
    // distance from the camera, so under real budget pressure (a big
    // pan/zoom mounting many chunks in one frame) it could let a handful of
    // far chunks resolve first purely because they mounted first, while much
    // closer chunks queued behind them for a frame or more — the opposite
    // of the "resolves outward from the camera" this whole scheduler exists
    // for. Same (in_frustum desc, distance2 asc) key the commit loop below
    // sorts pending by — one shared notion of priority, not two.
    //
    // distance2 is always a real, if sometimes approximate, camera-relative
    // value now (every control seeds it from estimate_initial_chunk_view at
    // construction — see that control's own distance2 field comment — never
    // a true Infinity sentinel), so this alone is enough: a brand new
    // control mounted this very frame competes fairly on real distance
    // against an older still-pending one instead of needing a separate
    // "urgent" tier. An earlier version of this sort DID give blend_active/
    // settling controls a whole tier ahead of distance — that seemed right
    // (their own first-ever recheck matters most) but backfires under
    // sustained camera movement: a continuously panning camera keeps
    // minting brand-new blend_active chunks at its leading edge, an
    // unbounded stream that could occupy the "urgent" tier forever and
    // starve an older, still-pending, genuinely-closer chunk that merely
    // isn't blend_active/settling anymore — exactly "far keeps cutting in
    // line over near" during continuous motion, not just a one-off frame.
    // Distance alone doesn't have that failure mode: a far new chunk sorts
    // behind a near old one no matter how continuously the camera moves.
    //
    // Filtered to this frame's actually-eligible controls BEFORE sorting,
    // not sorted-then-filtered — controls can number in the thousands
    // (every terrain chunk across LOAD_RADIUS's whole page window, times
    // every decor kind on top), and on all but a full_recheck frame only
    // the blend_active/settling subset (usually far smaller) needs
    // anything done at all. Sorting the full set every single frame
    // regardless of eligibility would size this copy+sort to the total
    // registrant count instead of the actual per-frame workload — real,
    // needless GC pressure and comparisons on the common (non-full_recheck)
    // frame, for a subset most of which was about to be skipped anyway.
    const eligible = controls.filter((control) => full_recheck || control.blend_active || control.settling);
    eligible.sort((a, b) => Number(b.in_frustum) - Number(a.in_frustum) || a.distance2 - b.distance2);
    run_under_budget(eligible, (control) => {
      control.recheck(state.camera, frustum);
      return true;
    }, { budget_ms: LOD_WORK_BUDGET_MS, hard_budget_ms: LOD_HARD_BUDGET_MS });

    // Every blend_active-true control is by construction included in
    // eligible above (it's one of the three needs_recheck conditions), so
    // this OR over just the eligible subset still sees every one of them —
    // equivalent to scanning all of controls for this specific flag, just
    // without paying to visit the ones that couldn't have it. Deliberately
    // a full scan, not folded into the budgeted pass above: a control whose
    // recheck got deferred still carries last frame's flag and still needs a
    // repaint scheduled for it, so this must see the ones the budget skipped.
    let blend_active = false;
    for (const control of eligible) blend_active ||= control.blend_active;
    const recheck_ms = import.meta.env.DEV ? performance.now() - recheck_t0 : 0;

    const pending_total = controls.filter((control) => control.pending);
    // Visible builds/refinements win. Pending work outside the frustum only
    // drops a formerly visible chunk's derived surface and GPU buffers.
    const pending = pending_total.sort((a, b) => Number(b.in_frustum) - Number(a.in_frustum) || a.distance2 - b.distance2);
    const commit_t0 = import.meta.env.DEV ? performance.now() : 0;
    // commit() returning false means "nothing to do after all" — it neither
    // counts toward LOD_MAX_CHUNKS_PER_FRAME nor arms the deadline, which is
    // what run_under_budget's own falsy-step rule preserves.
    const committed: u32 = run_under_budget(pending, (control) => control.commit(), {
      budget_ms: LOD_WORK_BUDGET_MS,
      hard_budget_ms: LOD_HARD_BUDGET_MS,
      max_steps: LOD_MAX_CHUNKS_PER_FRAME,
    }).ran;
    // After the commits, so a chunk's terrain and its decor reach the screen
    // in the same frame — the ready-state desync the 2026-09-05 unification
    // fixed, kept fixed now that decor is contributed rather than mounted.
    decor_field.update(anchor_registry.cur_map_origin);
    const commit_ms = import.meta.env.DEV ? performance.now() - commit_t0 : 0;
    if (import.meta.env.DEV) {
      const settling_lod = controls.filter((control) => control.settling).length;
      const blend_active_cnt = controls.filter((control) => control.blend_active).length;
      const in_frustum_cnt = controls.filter((control) => control.in_frustum).length;
      const pending_lod_total_cnt = pending_total.length - committed;
      const frame_ms = performance.now() - frame_t0;

      // Report this frame to sched/ledger.ts BEFORE reading the peak below —
      // the ledger owns both views now (a ~4s ring for the shape of a
      // stutter, a session peak for one you noticed too late to catch in the
      // ring), so this loop only has to hand over raw numbers.
      const perf_memory = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory;
      const poi_map = usePoiStore.getState().poi_map;
      let poi_count = 0;
      for (const pois of poi_map.values()) poi_count += pois.length;
      const fx = useSettingsStore.getState();
      const fx_passes = 1 + // color-grade + vignette, always on (fused)
        (fx.show_ao ? 1 : 0) + (fx.show_bloom ? 1 : 0) + (fx.show_tilt_shift ? 1 : 0) +
        (fx.show_chromatic_aberration ? 1 : 0) + (fx.show_god_rays ? 1 : 0) +
        (fx.aa_mode === "smaa" ? 1 : 0) + (fx.show_shadow_map ? 1 : 0);
      // Reads and clears in one step, so the next frame starts from zero
      // without a separate reset pass to forget.
      const node_work = take_node_counts();
      ledger_record_frame({
        t: performance.now(),
        dt_ms: delta * 1000,
        sched_ms: frame_ms,
        cpu: { recheck_ms, commit_ms, render_ms: last_render_ms },
        // Filled by the microtask below, which runs once the rAF task's JS
        // stack empties — i.e. after every other useFrame subscriber and after
        // the composer's render. Carries the previous frame's value here and
        // is corrected in place a moment later.
        task_ms: last_task_ms,
        gpu_ms: gpu_timer_last_ms(),
        draw_calls: state.gl.info.render.calls,
        triangles: state.gl.info.render.triangles,
        geometries: state.gl.info.memory.geometries,
        textures: state.gl.info.memory.textures,
        px: state.gl.domElement.width * state.gl.domElement.height,
        fx_passes,
        mounts: { added: node_work.added, removed: node_work.removed, rendered: node_work.rebuilt },
        lod: {
          chunks: controls.length,
          in_frustum: in_frustum_cnt,
          pending: pending_lod_total_cnt,
          settling: settling_lod,
          blend_active: blend_active_cnt,
        },
        heap_mb: perf_memory ? Math.round(perf_memory.usedJSHeapSize / 1_048_576) : 0,
        tile_pages: useTileStore.getState().tile_map.size,
        poi_count,
        pool: pool_sample(),
        work: work_sample(),
        q_terrain: fetch_pending_count("terrain"),
        q_poi: fetch_pending_count("poi"),
      });
      const peak = ledger_frame_peak();

      const debug_stats = {
        chunks: controls.length,
        in_frustum: in_frustum_cnt,
        pending_lod: pending.length - committed,
        pending_lod_total: pending_lod_total_cnt,
        blend_active: blend_active_cnt,
        settling_lod,
        draw_calls: state.gl.info.render.calls,
        triangles: state.gl.info.render.triangles,
        geometries: state.gl.info.memory.geometries,
        textures: state.gl.info.memory.textures,
        compositor: compositor_cache_stats(),
        lod_build: { ...lod_build_stats },
        coarse_instances: { ...coarse_instance_stats },
        occlusion: { ...occlusion_stats },
        // Derived from the ledger's session peak rather than a second set of
        // counters maintained here. Key names kept as they were — this blob
        // is read from the console and scraped off the canvas dataset.
        scheduler_peak: {
          recheck_ms_max: peak.recheck_ms,
          commit_ms_max: peak.commit_ms,
          frame_ms_max: peak.sched_ms,
          settling_lod_max: peak.settling,
          blend_active_max: peak.blend_active,
          pending_lod_total_max: peak.pending,
          in_frustum_max: peak.in_frustum,
        },
        mesh_mount: { ...mesh_mount_stats },
        mesh_unmount: { ...mesh_unmount_stats },
        commit_step: { ...commit_step_stats },
        render_call: { ...render_call_stats },
      };
      (globalThis as typeof globalThis & { __PXL_RENDER_STATS__?: unknown }).__PXL_RENDER_STATS__ = debug_stats;
      state.gl.domElement.dataset.pixeletRenderStats = JSON.stringify(debug_stats);
      // Start the next frame's draw-count accumulation (autoReset is off).
      state.gl.info.reset();

      // Close the GPU query and measure the whole rAF task once the stack
      // empties. A microtask is the only vantage point that sees the render:
      // the post-processing composer renders through its own useFrame, so the
      // gl.render wrapper above never fires and render_ms reads 0.
      const sample = ledger_last_frame();
      queueMicrotask(() => {
        gpu_timer_end();
        last_task_ms = performance.now() - frame_t0;
        if (sample) sample.task_ms = last_task_ms;
      });
    }
    if (committed > 0 || pending.length > committed || blend_active) state.invalidate();
  });

  return <>{children}</>;
}

export type ChunkStageProps = { stack: WorldLayerStack };

/** Renders terrain/objects AND every decor kind together — see this file's
 * own top doc comment and decor_layer.ts's for why decor no longer has its
 * own top-level component (DecorLayer, retired 2026-09-05). Decor models are
 * not passed in either: decor/asset_store.ts owns them and every ChunkNode
 * subscribes to it directly. */
export const ChunkStage = memo(({ stack }: ChunkStageProps) => {
  const map_center = useMapStore((state) => state.map_center);
  const tile_map = useTileStore((state) => state.tile_map);

  const center_page_idx = map_center === null ? null : L3_IDX(map_center.i);
  const center_page_key = center_page_idx?.toString() ?? null;
  const tiles = useMemo(() => Array.from(tile_map.keys()), [tile_map]);
  const visible_pages = useMemo(() => {
    if (center_page_idx === null) return tiles;
    return tiles
      .map((page_idx) => ({ page_idx, dist: CHEBYSHEV_DIST(ivec.sub(page_idx, center_page_idx)) }))
      .filter(({ dist }) => dist <= RENDER_RADIUS)
      .sort((a, b) => a.dist - b.dist)
      .map(({ page_idx }) => page_idx);
  }, [tiles, center_page_key]);

  const anchor_x = map_center === null ? null : ivec.X(map_center.i) & ~63n;
  const anchor_y = map_center === null ? null : ivec.Y(map_center.i) & ~63n;
  const range_anchor_key = anchor_x === null ? null : `${anchor_x},${anchor_y}`;
  const range_anchor = useMemo(
    () => anchor_x === null || anchor_y === null ? null : ivec.gather(anchor_x, anchor_y, 0n),
    [range_anchor_key],
  );

  if (range_anchor === null) return null;
  return (
    <group name="chunk-stage">
      {visible_pages.map((page_idx) => <PageChunks key={page_idx.toString()} page_idx={page_idx} stack={stack} range_anchor={range_anchor} />)}
    </group>
  );
});
ChunkStage.displayName = "ChunkStage";
