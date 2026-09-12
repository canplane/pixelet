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
 * (chunk, decor kind) in the shared scheduler below) — retired
 * once Chunk absorbed decor's own build into its own commit(), the fix for
 * the documented terrain/decor ready-state desync (decor could render a
 * frame before its own co-located terrain). See decor_layer.ts's own top
 * doc comment and discussions/20260905-tier1-geometry-baking-implemented.md
 * (and its own same-day follow-up) for the fuller account.
 */

import { memo, type ReactNode, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import { useThree } from "@react-three/fiber";
import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import type { U32Array } from "@xpute/core/kit/array.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { fvec, type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { useMapStore } from "@/modules/map/store.ts";
import { useAnchorRegistry } from "@/modules/world/anchor/context.tsx";
import { Vector } from "@xpute/core/ds/arena.ts";

import {
  chunk_retained_bytes,
  ChunkNode,
  commit_step_stats,
  lod_build_stats,
  lod_scheduler,
  mesh_mount_stats,
  mesh_unmount_stats,
  RETAIN_BUDGET_BYTES,
  take_node_counts,
} from "@/modules/world/chunk_node.ts";
import { AnchorNode } from "@/modules/world/anchor/node.tsx";
import { decor_field } from "@/modules/world/decor/field.ts";
import { tile_pages, useTileStore } from "@/modules/world/terrain/store.ts";
import { poi_pages } from "@/modules/world/poi/store.ts";
import { fetch_pending_count } from "@/modules/world/sched/fetch_scheduler.ts";
import { draw_census_install, draw_census_take } from "@/modules/world/sched/draw_census.ts";
import { gpu_timer_attach, gpu_timer_begin, gpu_timer_end, gpu_timer_last_ms } from "@/modules/world/sched/gpu_timer.ts";
import { ledger_frame_peak, ledger_last_frame, ledger_record_frame } from "@/modules/world/sched/ledger.ts";
import { useTick } from "@/modules/world/sched/tick.ts";
import { lead_point, priority_order } from "@/modules/world/sched/priority.ts";
import { BLEND_ACTIVE, IN_FRUSTUM, PENDING, SETTLING, VISIBILITY_CHANGED } from "@/modules/world/sched/chunk_table.ts";
import { useFrameRefs } from "@/modules/world/frame/context.tsx";
import { useRangeAnchor } from "@/modules/world/frame/use_range_anchor.ts";
import { in_range_chunks, pages_in_radius, RANGE_HYSTERESIS } from "@/render/chunk_range.ts";
import { render_cfg } from "@/modules/world/environment/config.ts";
import { set_lod_projection } from "@/render/lod.ts";
import { compositor_cache_stats } from "@/render/layer/pipeline/compositor.ts";
import type { WorldLayerStack } from "@/render/layer/pipeline/contract.ts";
import { coarse_instance_stats, occlusion_stats } from "@/render/layer/pipeline/chunk_mesher.ts";
import { CORE_ALLOC, core_pool_stat } from "@/render/layer/pipeline/core_pool.ts";
import { update_frustum } from "@/render/chunk_lod.ts";

const LOD_RECHECK_INTERVAL: f64 = 0.15;
const LOD_MAX_CHUNKS_PER_FRAME: u32 = 8;
/* Releases are the lowest class and stay there (sched/priority.ts), but a
 * class that only ever runs after every visible build never runs at all while
 * a zoom-out keeps the queue full: 403 of them waited 10.4 s in the
 * 2026-09-12 capture while heap peaked at 522MB, and a pending release is
 * holding a mesh the pool could hand to someone else. So they get a small
 * share of their own rather than being aged up into the classes above. */
const RELEASE_MAX_PER_FRAME: u32 = 2;
/* How many off-screen chunks may give their built work back in one frame
 * once the memory budget is exceeded. Reclaiming is cheap (pool returns),
 * but the budget is a ceiling to drift back under, not a line to snap to. */
const RECLAIM_MAX_PER_FRAME: u32 = 16;
/* The per-pass budgets that used to sit here — a soft allowance per pass and
 * a 16ms hard ceiling each — are gone. Both passes now draw on the one
 * FrameBudget the tick hands every phase (sched/tick.ts), so the frame's
 * allowance is shared rather than granted twice, and the ceiling is slack
 * over that pool rather than a second constant. The 4711ms frame the hard
 * ceiling was measured against is recorded in frame_budget.ts. */

/* Session-peak scheduler load used to live here as its own object. It now
 * comes from sched/ledger.ts's ledger_frame_peak(): this loop reports each
 * frame's raw numbers and the ledger keeps both the ring and the window
 * peak, instead of two places tracking the same maxima. The reason the peak
 * exists at all is unchanged — a manual __PXL_RENDER_STATS__ read during a
 * live stutter lands a frame or two after the storm has passed. */

/* The render itself is not timed by wrapping gl.render: the post-processing
 * composer is always on and renders past the wrapper, which read 0. The
 * whole rAF task, measured from a microtask below, is what covers it. */
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

/** The core pool's gauges and its cumulative counters; a hit share is a
 * difference over a window, taken by summary.ts. */
function pool_sample(): { live_mb: number; idle_mb: number; hits: number; misses: number } {
  const stat = core_pool_stat();
  return { live_mb: Math.round(stat.live_bytes / MB), idle_mb: Math.round(stat.idle_bytes / MB), hits: stat.hits, misses: stat.misses };
}

/** Who is waiting, and for how long — the progress rule guarantees a step a
 * frame, not which one, so this is what shows whether something is being
 * passed over indefinitely. */
function starvation(): { offscreen_pending: u32; oldest_release_ms: f64; oldest_visible_ms: f64 } {
  const now = performance.now();
  const { flags, pending_since } = lod_scheduler;
  let offscreen_pending: u32 = 0;
  let oldest_release_ms: f64 = 0;
  let oldest_visible_ms: f64 = 0;
  for (let i = 0; i < lod_scheduler.len; i++) {
    if ((flags[i] & PENDING) === 0 || pending_since[i] < 0) continue;
    const waited = now - pending_since[i];
    if (flags[i] & IN_FRUSTUM) {
      if (waited > oldest_visible_ms) oldest_visible_ms = waited;
    } else {
      offscreen_pending++;
      if (waited > oldest_release_ms) oldest_release_ms = waited;
    }
  }
  return { offscreen_pending, oldest_release_ms, oldest_visible_ms };
}

/** Slot lists a frame sorts and walks: what to recheck, what to commit on
 * each side of the frustum, what to reclaim. Scratch, so a pass over
 * thousands of chunks allocates nothing. */
const SLOTS = { init_cap: 4096, alloc: CORE_ALLOC } as const;
const s_eligible = new Vector(Uint32Array, SLOTS);
const s_visible = new Vector(Uint32Array, SLOTS);
const s_offscreen = new Vector(Uint32Array, SLOTS);
/** The frustum as six planes of (nx, ny, nz, constant), the form
 * ChunkTable.refresh tests spheres against. */
const s_planes = new Float64Array(24);

function sorted(slots: Vector<U32Array>, order: (a: u32, b: u32) => number): U32Array {
  const view = slots.mem.subarray(0, slots.len);
  view.sort(order);
  return view;
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
  // Two sets, not one: what to build, and what to keep once built. A chunk
  // that merely drifted past the range is left alone until it is well past
  // it — see RANGE_HYSTERESIS.
  const offsets = useMemo(() => in_range_chunks(page_base, range_anchor), [page_base, range_anchor]);
  const keep = useMemo(
    () => new Set(in_range_chunks(page_base, range_anchor, render_cfg.CHUNK_RENDER_DIST + RANGE_HYSTERESIS).map((offset) => offset.key)),
    [page_base, range_anchor],
  );

  useLayoutEffect(() => {
    const live = nodes.current;
    const wanted = new Map(offsets.map((offset) => [offset.key, offset]));
    for (const [key, node] of live) {
      if (keep.has(key)) continue;
      node.dispose();
      live.delete(key);
    }
    for (const [key, offset] of wanted) {
      if (live.has(key)) continue;
      const node = new ChunkNode({ page_idx, offset, stack, camera, anchor_registry });
      group.add(node.group);
      live.set(key, node);
    }
  }, [offsets, keep, page_idx, stack, camera, anchor_registry, group]);

  // Destruction on unmount is deferred by one microtask, and a mount that
  // arrives in that window cancels it. StrictMode replays every mount as
  // mount, cleanup, mount, and a cleanup that destroyed the nodes made the
  // second mount rebuild the whole page from the scan up — up to 256 chunks a
  // page, on every page the view entered, in development only. Measured
  // 2026-09-12: with StrictMode on, a zoomed-out pan unmounted five pages and
  // disposed up to 510 nodes a frame; with it off, none. A real unmount is
  // never followed by a mount in the same microtask, so it still destroys
  // exactly as before. AnchorNode survives the same replay for the same
  // reason (its own doc comment).
  const teardown = useRef<Promise<void> | null>(null);
  useEffect(() => {
    teardown.current = null;
    const live = nodes.current;
    return () => {
      const pending = Promise.resolve().then(() => {
        if (teardown.current !== pending) return;
        for (const node of live.values()) node.dispose();
        live.clear();
      });
      teardown.current = pending;
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
  const scheduler = lod_scheduler;
  const lod_elapsed = useRef(LOD_RECHECK_INTERVAL);
  const frustum = useMemo(() => new THREE.Frustum(), []);
  const { gl } = useThree();
  const anchor_registry = useAnchorRegistry();
  const { local_world_frame_ref } = useFrameRefs();
  const lead = useMemo(() => new THREE.Vector3(), []);

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
    return draw_census_install(gl);
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
    if (import.meta.env.DEV) gpu_timer_begin();
    const table = scheduler;
    visible_pass.current = null;
    if (table.len === 0) return;
    lod_elapsed.current += delta;
    const full_recheck = lod_elapsed.current >= LOD_RECHECK_INTERVAL;
    if (full_recheck) lod_elapsed.current = 0;
    update_frustum(frustum, state.camera, projection);
    for (let p = 0; p < 6; p++) {
      const plane = frustum.planes[p];
      s_planes[p * 4] = plane.normal.x;
      s_planes[p * 4 + 1] = plane.normal.y;
      s_planes[p * 4 + 2] = plane.normal.z;
      s_planes[p * 4 + 3] = plane.constant;
    }
    // The tier criterion is a screen-space one (render/lod.ts), so it needs
    // the projection it measures against. Cheap, and a no-op while the
    // viewport and the field of view hold still.
    set_lod_projection(state.size.height, (state.camera as THREE.PerspectiveCamera).fov);

    // Where the camera is heading — see sched/priority.ts. Both passes below
    // order by it; the tier each chunk picks still comes from the camera as
    // drawn.
    const target = useMapStore.getState().map_center;
    const local_frame = local_world_frame_ref.current;
    const origin = anchor_registry.cur_map_origin;
    if (target && local_frame) {
      const local = ivec.sub(target.i, origin);
      lead_point(
        lead,
        state.camera.position,
        ivec.to_real(ivec.X(local)) + fvec.to_real(fvec.X(target.f)),
        ivec.to_real(ivec.Y(local)) + fvec.to_real(fvec.Y(target.f)),
        ivec.to_real(ivec.Z(local)) + fvec.to_real(fvec.Z(target.f)),
        local_frame.matrixWorld,
      );
    } else {
      lead.copy(state.camera.position);
    }

    // Every chunk, every frame, before anything is sorted: this is only
    // measurement, and it is what the orders below read. The decision pass
    // under it is rationed, so sorting on what that pass last wrote meant
    // sorting on numbers up to LOD_RECHECK_INTERVAL old — during a zoom-out,
    // the difference between "the middle of the screen matters" and "it did
    // not exist yet". One matrix for every chunk: the anchor root's, which
    // is the frame every chunk group is a pure translation inside.
    const cam = state.camera.position;
    table.refresh(
      anchor_registry.root.matrixWorld.elements,
      ivec.to_real(ivec.X(origin)),
      ivec.to_real(ivec.Y(origin)),
      ivec.to_real(ivec.Z(origin)),
      s_planes,
      cam.x,
      cam.y,
      cam.z,
      lead.x,
      lead.y,
      lead.z,
    );
    // One ordering for both passes of this frame, and one `now` for both:
    // ageing keys that moved mid-sort would make the comparator inconsistent.
    const order = priority_order(table, performance.now());

    // The lists this frame walks, in one pass over the flag lane: the scene
    // nodes whose visibility moved, the chunks to recheck, and the pending
    // work on each side of the frustum.
    const flags = table.flags;
    const handles = table.handles;
    s_eligible.truncate(0);
    s_visible.truncate(0);
    s_offscreen.truncate(0);
    let pending_total: u32 = 0;
    for (let i = 0; i < table.len; i++) {
      const f = flags[i];
      if (f & VISIBILITY_CHANGED) {
        handles[i].group.visible = (f & IN_FRUSTUM) !== 0;
        flags[i] = f & ~VISIBILITY_CHANGED;
      }
      if (full_recheck || (f & (BLEND_ACTIVE | SETTLING)) !== 0) s_eligible.push(i);
      if (f & PENDING) {
        pending_total++;
        if (f & IN_FRUSTUM) s_visible.push(i);
        else s_offscreen.push(i);
      }
    }

    const recheck_t0 = import.meta.env.DEV ? performance.now() : 0;
    // Same cooperative shape as the commit loop below: the first eligible
    // chunk this frame always runs (forward progress guaranteed even on a
    // slow frame), the rest are time-boxed. Profiled: during a fast zoom,
    // settling/blend_active can both spike into the dozens at once, and
    // rechecking every one of them every single frame cost up to ~23ms by
    // itself — a stale blend/settling flag for one skipped frame is a much
    // smaller visual cost than that.
    //
    // Sorted by the same priority_order the commit loop below sorts pending
    // by — one shared notion of priority, not two — and filtered to this
    // frame's eligible chunks before sorting: on all but a full_recheck
    // frame only the blend_active/settling subset needs anything done.
    // Distance alone orders it; an "urgent" tier for fresh chunks backfired
    // under sustained camera movement, where the leading edge mints new
    // chunks without end and an older, nearer, still-pending one starves.
    budget.run(sorted(s_eligible, order), (slot) => {
      handles[slot].recheck();
      return true;
    });

    // A chunk whose recheck the budget skipped still carries last frame's
    // flag and still needs a repaint scheduled for it, so this is read off
    // the lane, not off what ran.
    const blend_active = table.count(BLEND_ACTIVE) > 0;
    const recheck_ms = import.meta.env.DEV ? performance.now() - recheck_t0 : 0;

    const commit_t0 = import.meta.env.DEV ? performance.now() : 0;
    // Visible builds/refinements win; pending work outside the frustum only
    // drops a formerly visible chunk's derived surface and GPU buffers. The
    // slot holder goes first whatever its distance (chunk_table.ts's
    // HOLDS_SLOT). commit() returning false means "nothing to do after all"
    // — it neither counts toward LOD_MAX_CHUNKS_PER_FRAME nor arms the
    // deadline, which is what run_under_budget's own falsy-step rule
    // preserves. The commit pass gets what the recheck pass left, not a
    // budget of its own. Visible work takes the frame's steps; releases get
    // their own small guaranteed share out of what is left. run_under_budget
    // always lets the first item of a pass do work, so a release lands every
    // frame even on a frame the visible pass filled — which is the whole
    // point of a guaranteed share.
    const step = (slot: u32) => handles[slot].commit(budget);
    const committed: u32 = budget.run(sorted(s_visible, order), step, { max_steps: LOD_MAX_CHUNKS_PER_FRAME }).ran +
      budget.run(sorted(s_offscreen, order), step, { max_steps: RELEASE_MAX_PER_FRAME }).ran;
    // The memory budget: built work is kept off screen until what off-screen
    // chunks hold is over RETAIN_BUDGET_BYTES, then reclaimed farthest first —
    // `order` already sorts off-screen chunks that way. Bounded per frame
    // like everything else, and a no-op while under.
    if (chunk_retained_bytes().off > RETAIN_BUDGET_BYTES) {
      s_offscreen.truncate(0);
      const retained = table.retained;
      for (let i = 0; i < table.len; i++) {
        if ((flags[i] & IN_FRUSTUM) === 0 && retained[i] > 0) s_offscreen.push(i);
      }
      let freed = 0;
      for (const slot of sorted(s_offscreen, order)) {
        if (chunk_retained_bytes().off <= RETAIN_BUDGET_BYTES || freed >= RECLAIM_MAX_PER_FRAME) break;
        handles[slot].reclaim();
        freed++;
      }
    }
    // After the commits, so a chunk's terrain and its decor reach the screen
    // in the same frame — the ready-state desync the 2026-09-05 unification
    // fixed, kept fixed now that decor is contributed rather than mounted.
    decor_field.update(origin);
    const commit_ms = import.meta.env.DEV ? performance.now() - commit_t0 : 0;
    if (committed > 0 || pending_total > committed || blend_active) state.invalidate();
    if (import.meta.env.DEV) visible_pass.current = { pending_total, committed, recheck_ms, commit_ms };
  });

  // The ledger, after every other phase has run, so `sched_ms` is the whole
  // tick and not this component's share of it.
  useTick("report", ({ state, delta, budget, phase_ms, frame_ms: display_ms, settled }) => {
    if (!import.meta.env.DEV) return;
    const pass = visible_pass.current;
    if (!pass) {
      // No chunks this frame — a search jump empties the stage for a moment.
      // What the visible phase opened still has to be closed: skipping the
      // reset let draw counts pile up across every such frame and land on
      // the next one (a 152M-triangle "frame" in the ledger), and skipping
      // the query end left the GPU timer open into the next begin.
      state.gl.info.reset();
      draw_census_take();
      queueMicrotask(gpu_timer_end);
      return;
    }
    const { pending_total, committed, recheck_ms, commit_ms } = pass;
    {
      const settling_lod = scheduler.count(SETTLING);
      const blend_active_cnt = scheduler.count(BLEND_ACTIVE);
      const in_frustum_cnt = scheduler.count(IN_FRUSTUM);
      const pending_lod_total_cnt = pending_total - committed;
      const frame_t0 = budget.t0;
      const frame_ms = performance.now() - frame_t0;

      // Report this frame to sched/ledger.ts BEFORE reading the peak below —
      // the ledger owns both views now (the ring and the window peak), so
      // this loop only has to hand over raw numbers.
      const perf_memory = (performance as Performance & { memory?: { usedJSHeapSize: number } }).memory;
      let poi_count = 0;
      for (const pois of poi_pages.handles) poi_count += pois.length;
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
        frame_ms: display_ms,
        settled,
        sched_ms: frame_ms,
        budget_ms: budget.total_ms,
        phases: { ...phase_ms },
        cpu: { recheck_ms, commit_ms },
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
        census: draw_census_take(),
        px: state.gl.domElement.width * state.gl.domElement.height,
        fx_passes,
        mounts: { added: node_work.added, removed: node_work.removed, rendered: node_work.rebuilt },
        lod: {
          chunks: scheduler.len,
          in_frustum: in_frustum_cnt,
          pending: pending_lod_total_cnt,
          settling: settling_lod,
          blend_active: blend_active_cnt,
          ...starvation(),
        },
        heap_mb: perf_memory ? Math.round(perf_memory.usedJSHeapSize / 1_048_576) : 0,
        tile_pages: tile_pages.size,
        poi_count,
        pool: pool_sample(),
        work: work_sample(),
        q_terrain: fetch_pending_count("terrain"),
        q_poi: fetch_pending_count("poi"),
      });
      const peak = ledger_frame_peak();

      const debug_stats = {
        chunks: scheduler.len,
        in_frustum: in_frustum_cnt,
        pending_lod: pending_total - committed,
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
      };
      (globalThis as typeof globalThis & { __PXL_RENDER_STATS__?: unknown }).__PXL_RENDER_STATS__ = debug_stats;
      state.gl.domElement.dataset.pixeletRenderStats = JSON.stringify(debug_stats);
      // Start the next frame's draw-count accumulation (autoReset is off).
      state.gl.info.reset();

      // Close the GPU query and measure the whole rAF task once the stack
      // empties. A microtask is the only vantage point that sees the render:
      // the post-processing composer renders through its own useFrame.
      const sample = ledger_last_frame();
      queueMicrotask(() => {
        gpu_timer_end();
        last_task_ms = performance.now() - frame_t0;
        if (sample) sample.task_ms = last_task_ms;
      });
    }
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
  const tile_revision = useTileStore((state) => state.revision);

  // tile_revision: the keys live in tile_pages; the revision is what moves.
  const tiles = useMemo(() => Array.from(tile_pages.keys()), [tile_revision]);
  const range_anchor = useRangeAnchor();
  // Every page the render range reaches, nearest first, that has a tile to
  // draw from. A page whose tile has not arrived contributes nothing and is
  // simply not mounted; which chunks inside a mounted page become scene
  // objects is in_range_chunks' own decision.
  const visible_pages = useMemo(() => {
    if (range_anchor === null) return tiles;
    const have = new Set(tiles);
    // The same hysteresis one level up: a page is kept mounted while any
    // chunk of it could still be kept, so crossing the range by metres never
    // destroys a page's worth of built chunks.
    return pages_in_radius(range_anchor, render_cfg.CHUNK_RENDER_DIST + RANGE_HYSTERESIS)
      .filter(({ page_idx }) => have.has(page_idx))
      .map(({ page_idx }) => page_idx);
  }, [tiles, range_anchor]);

  if (range_anchor === null) return null;
  return (
    <group name="chunk-stage">
      {visible_pages.map((page_idx) => <PageChunks key={page_idx.toString()} page_idx={page_idx} stack={stack} range_anchor={range_anchor} />)}
    </group>
  );
});
ChunkStage.displayName = "ChunkStage";
