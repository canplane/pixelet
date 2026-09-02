// @pxl/web/modules/world/decor/decor_layer.tsx

/**
 * Renders one DecorKind (trees today) as scattered GLB instances across
 * every visible chunk — mirrors chunk_stage.tsx's own page/chunk
 * enumeration (visible_pages by RENDER_RADIUS, in_range_chunks for the
 * chunk offsets within each page) closely enough that decorations pop in
 * and out of view in lockstep with the terrain they sit on. The scheduling
 * lifecycle (frustum culling, LOD-tier recheck/commit, shared work budget)
 * IS shared with chunk_stage.tsx today — see the ChunkLodControl paragraph
 * below. What's still separate is the build/render pipeline itself:
 * chunk_stage.tsx bakes terrain by instancing one shared unit box, while
 * this file instances arbitrary per-variant GLB geometry — a scattered,
 * sparse decoration layer never needed that box-instancing machinery's own
 * complexity (dense per-cell scanning, multi-tier blending), so it keeps
 * its own much simpler build step rather than folding into chunk_stage.tsx's.
 *
 * No decoration is ever stored: scatter.ts's placement is a pure function
 * of terrain + a fixed seed, recomputed on demand. A cell that currently
 * holds a user-placed object simply has its candidate filtered out at
 * render time (get_objects_in_cell, exact-cell — not get_objects_near's
 * ±1-cell neighborhood); removing that object clears the very same filter
 * on the next objects-revision-triggered recompute, so the decoration
 * reappears with no bookkeeping of its own.
 *
 * Each ChunkDecor registers its own ChunkLodControl (frustum test, distance,
 * LOD-step recheck()/commit()) into the exact same LodSchedulerRegistry
 * chunk_stage.tsx's own Chunk components use — see LodSchedulerProvider in
 * that file, mounted once above both ChunkStage and every DecorLayer
 * (world.tsx), so terrain and decor compete for one shared per-frame work
 * budget and chunk count cap instead of each spending its own. Frustum
 * culling and LOD-tier selection are not two separate systems here: a
 * chunk's own recheck() does the frustum sphere test first and returns
 * immediately if it fails, before ever sampling elevation or asking
 * render/lod.ts for a tier — an off-screen chunk costs nothing beyond that
 * one sphere check, and only commit() (gated by the same shared budget)
 * ever actually rebuilds anything.
 *
 * Density DOES already fall off with distance, snapped to the exact same
 * LOD tiers terrain hands off at — not by reimplementing "distance to
 * camera" a second time (render/chunk_lod.ts's own doc comment explains
 * why that drifts out of sync with what terrain is actually showing), but
 * by calling the exact same camera -> distance arithmetic chunk_stage.tsx's
 * Chunk uses for its own real per-frame LOD. See DECOR_DENSITY_BY_LOD_STEP
 * below for the actual falloff curve and why it's steep rather than a
 * gentle taper.
 *
 * CURRENT LIMITATIONS, left as-is deliberately rather than gaps to close:
 * - No blend/secondary-tier dithering across an LOD threshold band, unlike
 *   chunk_stage.tsx's own Chunk (get_lod_pair_visibility's stochastic
 *   finer/coarser split in render/lod.ts). That machinery exists there to
 *   hide a *dense, screen-filling* surface snapping to a new tier all at
 *   once; a decor chunk's density merely stepping down (1 -> 0.5 -> 0.25 ->
 *   0) already reads as individual sparse objects thinning out, not a
 *   visible surface repainting itself, so the same fix would cost real
 *   complexity (a second built instance set per chunk, the same per-chunk
 *   hash/coarse_fraction machinery) for a pop that isn't actually
 *   objectionable here. Not planned unless that changes.
 * - A source-data change (tile arrives, an object is placed/removed) is
 *   picked up inside recheck() itself by comparing against what was last
 *   committed, not through an immediate-reaction path — unlike
 *   chunk_stage.tsx's own Chunk, which has a separate source_ref-keyed
 *   useEffect that reacts the same React commit a change happens, ahead of
 *   the next scheduler tick. A decor chunk instead waits for its next
 *   recheck (at most LOD_RECHECK_INTERVAL, 0.15s, since decor's
 *   blend_active is only ever true for its first frame) — an imperceptible
 *   delay, so the extra machinery wasn't worth duplicating.
 *
 * LONGER-TERM DIRECTION: Minecraft's own block model is the reference point
 * here, and it's a 3-tier split, not 2 — (1) static grid content (full
 * cubes AND non-cube decorative blocks like candles alike) shares one
 * per-cell storage array and gets baked into one shared per-chunk mesh, no
 * instancing at all; (2) static content that still needs a per-frame
 * visual update (an open chest, sign text) keeps that (1) baked mesh and
 * adds a thin renderer on top for just the dynamic part; (3) anything that
 * moves under its own logic every frame (mobs, minecarts) lives entirely
 * outside the grid as its own object. Tier 3 already has a match here —
 * characters/companion.tsx's own useFrame loop, untouched by any of this.
 * Tier 1 is the actual gap: chunk_stage.tsx bakes terrain by instancing one
 * shared unit box (TERRAIN_BOX/OBJECT_BOX), while this file instances
 * arbitrary GLB geometry per variant — two different primitives standing in
 * for what Minecraft treats as one baked buffer per chunk regardless of
 * block shape. Full parity means folding this bundle's geometry into
 * chunk_stage.tsx's own build step instead of a second InstancedMesh
 * pipeline — a materially bigger, separate effort from the scheduler
 * registration above (already landed — this file's ChunkDecor and that
 * file's Chunk are equal ChunkLodControl registrants today) since it needs
 * a new geometry-agnostic per-chunk baking scheme, not just wiring into an
 * existing one. User-placed decor (a candle a player places, as
 * opposed to a procedurally scattered tree) should still land in
 * objects/store.ts's existing per-cell tree as its own ObjectLayerEntry
 * kind alongside "solid" — same tier-1 storage Minecraft gives decorative
 * blocks — with its instances merged into this file's own
 * instances_by_variant next to scatter_decor_candidates' output, not a
 * separate store.
 */

import { memo, useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";
import type { Tile } from "@pxl/shared/bake/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { FROM_L3_IDX, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { derive_seed_u64 } from "@xpute/core/kit/rng.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useTileStore } from "@/modules/world/terrain/store.ts";
import { RENDER_RADIUS } from "@/modules/world/terrain/config.ts";
import { CHEBYSHEV_DIST } from "@/modules/world/terrain/use_tile_fetcher.ts";
import { get_ele_at_local } from "@/modules/world/terrain/elevation.ts";
import { cell_index, cell_key, get_objects_in_cell, l2_column_key_of, useObjectL2ColumnRevision } from "@/modules/world/objects/store.ts";
import { in_range_chunks } from "@/render/chunk_range.ts";
import { get_lod_guide, type LodStep, step_lod_toward } from "@/render/lod.ts";
import { APPROXIMATE_CHUNK_RADIUS, chunk_distance_to_camera } from "@/render/chunk_lod.ts";
import { type ChunkLodControl, use_lod_scheduler } from "@/modules/world/chunk_stage.tsx";
import { AnchorNode } from "@/modules/world/anchor/node.tsx";

import { type DecorEligibility, type DecorKindSeed, scatter_decor_candidates } from "./scatter.ts";
import { compose_instance_matrix, type DecorVariantAsset, useInstancedModelVariants } from "./instanced_model.ts";

// Fixed, arbitrary — the one thing every DecorKind's own placement
// ultimately derives from (alongside its own DecorKindSeed.tag). Changing
// this reshuffles every decoration in the world; there is no reason to,
// since a page's own idx already makes every page's stream independent.
const DECOR_MASTER_SEED = 0xd0c0d0c0_dec0_5eed_0001n;

// Interim thinning step in place of real scheduler-driven LOD (this file's
// own doc comment above) — snapped to the exact same LOD tiers terrain
// itself hands off at (render/lod.ts's LOD_STEP_BY_LEVEL), rather than an
// independent distance curve, so decor density drops in step with terrain
// detail instead of on its own unrelated schedule.
//
// Halved at every tier starting from step1 itself, not a gentle taper:
// unlike terrain, a remaining decor instance never shrinks or blends with
// distance — it's the same full-size, full-detail model whether it's the
// one tree left after thinning or the only one that was ever there, so a
// moderate density deep into the falloff (e.g. 0.25) still reads as "a
// sparse forest", clearly present, not "basically gone". Cut to 0 at step8
// rather than continuing to halve — 0.125 was still a visible handful per
// once-dense cluster, not actually gone.
const DECOR_DENSITY_BY_LOD_STEP: Record<LodStep, f64> = {
  1: 1,
  2: 0.5,
  4: 0.25,
  8: 0,
  16: 0,
  32: 0,
  64: 0,
};

export type DecorKindBundle = {
  seed: DecorKindSeed;
  is_eligible: DecorEligibility;
  /** One entry per variant_count — see instanced_model.ts's own doc comment
   * on why each variant carries its own skin rather than the whole kind
   * sharing one: a kind's variants can (and for trees, do) span more than
   * one Kenney pack, each with its own colormap atlas. */
  variants: readonly DecorVariantAsset[];
};

type ChunkDecorProps = {
  page_idx: IVec;
  base_x: number;
  base_y: number;
  bundle: DecorKindBundle;
  geometries: readonly (THREE.BufferGeometry | null)[];
  materials: readonly (THREE.MeshBasicMaterial | null)[];
};

/** Pure "what would this chunk look like at this LOD tier, given this
 * source data" computation — the whole scan+placement pipeline, pulled out
 * of ChunkDecor itself so the ChunkLodControl closure below can call it from
 * commit() without capturing component state directly (mirrors
 * chunk_stage.tsx's own build_ref: control logic reads its inputs fresh
 * through refs, not through a React dependency array). */
function build_decor_instances(
  tile: Tile | undefined,
  lod_step: LodStep,
  page_idx: IVec,
  base_x: number,
  base_y: number,
  bundle: DecorKindBundle,
): ModelInstanceTransformList[] {
  const by_variant: ModelInstanceTransformList[] = bundle.variants.map(() => []);
  const density_scale = DECOR_DENSITY_BY_LOD_STEP[lod_step];
  // Skip the O(CHUNK_CELL_SPAN^2) eligibility+RNG scan entirely at zero
  // density instead of running it only to have every candidate rejected
  // by its own probability check below — density_scale=0 means "nothing
  // here", decided once, not discovered 4096 times.
  if (!tile || density_scale === 0) return by_variant;

  const page_seed = derive_seed_u64(DECOR_MASTER_SEED, page_idx);
  const candidates = scatter_decor_candidates(
    tile.terrain.ground,
    base_x,
    base_y,
    CHUNK_CELL_SPAN,
    page_seed,
    bundle.seed,
    bundle.is_eligible,
    density_scale,
  );

  const page_base = FROM_L3_IDX(page_idx);
  for (const candidate of candidates) {
    const x: f64 = candidate.cell_x * U_PER_CELL + U_PER_CELL / 2;
    const y: f64 = candidate.cell_y * U_PER_CELL + U_PER_CELL / 2;
    const ele = get_ele_at_local(page_idx, x, y);
    if (ele === null) continue;

    const cell_ivec = ivec.add(page_base, ivec.gather(BigInt(candidate.cell_x), BigInt(candidate.cell_y), BigInt(Math.floor(ele))));
    const [ix, iy, iz] = cell_index({ i: cell_ivec, f: 0n });
    if (get_objects_in_cell(cell_key(ix, iy, iz)).length > 0) continue;

    by_variant[candidate.variant].push({ position: [x, y, ele], rotation_z: candidate.rotation, scale: candidate.scale });
  }
  return by_variant;
}

const ChunkDecor = memo(({ page_idx, base_x, base_y, bundle, geometries, materials }: ChunkDecorProps) => {
  const tile = useTileStore((state) => state.get_tile(page_idx));
  const page_off = useMemo(() => ivec.gather(BigInt(base_x), BigInt(base_y), 0n), [base_x, base_y]);
  const object_key = useMemo(() => l2_column_key_of(page_idx, page_off), [page_idx, page_off]);
  const objects_revision = useObjectL2ColumnRevision(object_key);
  const scheduler = use_lod_scheduler();

  // recheck()/commit() (inside the control useMemo below) read the latest
  // tile/objects_revision through these refs rather than closing over them
  // directly — the control itself is deliberately NOT rebuilt (and
  // re-registered into the scheduler) on every tile fetch or object edit,
  // only on page_idx/base_x/base_y/bundle, which never change for an
  // already-mounted chunk.
  const tile_ref = useRef(tile);
  tile_ref.current = tile;
  const objects_revision_ref = useRef(objects_revision);
  objects_revision_ref.current = objects_revision;

  // Exists ONLY to give chunk_lod.ts a real mounted group to measure
  // through — see the return statement below for why it carries no meshes
  // of its own.
  const lod_group_ref = useRef<THREE.Group>(null);
  const [instances_by_variant, set_instances_by_variant] = useState<ModelInstanceTransformList[]>(
    () => bundle.variants.map(() => []),
  );

  const control = useMemo<ChunkLodControl>(() => {
    const half = CHUNK_CELL_SPAN * U_PER_CELL * 0.5;
    // Own scratch Vector3/Sphere, not shared across chunks — same shape as
    // chunk_stage.tsx's own Chunk control closure, so every chunk's
    // per-frame measurement costs no allocation without a registry-wide
    // scratch pool.
    const center = new THREE.Vector3();
    const sphere = new THREE.Sphere(center, APPROXIMATE_CHUNK_RADIUS);
    // committed_* is what's currently on screen (what instances_by_variant
    // was last built from); desired_lod_step is recheck()'s candidate for
    // the next commit(). Unlike chunk_stage.tsx's own Chunk, there is no
    // blend/secondary tier to track here — decor never blends between
    // tiers, it just thins or fills back in.
    let committed_lod_step: LodStep = 1;
    let committed_tile: Tile | undefined = undefined;
    let committed_objects_revision = -1;
    let desired_lod_step: LodStep = 1;
    // Same one-tier-at-a-time easing chunk_stage.tsx's own Chunk applies
    // (step_lod_toward) — without it this jumps straight to the target tier
    // in one commit while terrain eases in gradually, so the two visibly
    // disagree mid-transition even though both converge on the same tier
    // once the camera settles. has_measured mirrors that file's own
    // current.ready: the very first measurement has nothing on screen yet
    // to ease from, so it commits directly instead of stepping up from 1.
    let has_measured = false;

    return {
      // Sentinel only, matching chunk_stage.tsx's own Chunk convention —
      // decor never actually blends between tiers; this flag's only
      // remaining job is "recheck me on the very first frame regardless of
      // the periodic full_recheck cadence", cleared right after that first
      // recheck runs.
      blend_active: true,
      settling: false,
      in_frustum: true,
      distance2: Infinity,
      pending: false,
      recheck(camera, frustum) {
        const group = lod_group_ref.current;
        if (!group) return;

        // Frustum test first, same gate chunk_stage.tsx's own Chunk.recheck()
        // applies before its own (much heavier) scan/build step — a chunk
        // behind or beside the camera reads as "close" by distance alone, so
        // skipping this would keep scanning/building decor for chunks the
        // renderer was never going to draw in the first place. z=0 here is
        // an approximation (real elevation is sampled below, only once this
        // chunk is worth the cost), the same slack APPROXIMATE_CHUNK_RADIUS
        // already covers for chunk_stage.tsx's own equivalent fallback-
        // bounds case.
        center.set(half, half, 0).applyMatrix4(group.matrixWorld);
        this.in_frustum = frustum.intersectsSphere(sphere);
        this.blend_active = false;

        // Identity/value comparison against what instances_by_variant was
        // actually last built from — same "don't cache what a live store
        // already owns" shape as chunk_stage.tsx's own source_ref, just
        // folded into this same recheck() instead of a parallel useEffect:
        // decor's periodic full_recheck cadence (0.15s) is imperceptible,
        // so there's no need for chunk_stage.tsx's extra machinery to react
        // to a data change on the very same frame it happens.
        const source_changed = !Object.is(tile_ref.current, committed_tile) ||
          objects_revision_ref.current !== committed_objects_revision;

        if (!this.in_frustum) {
          this.distance2 = center.distanceToSquared(camera.position);
          desired_lod_step = has_measured ? step_lod_toward(committed_lod_step, 64) : 64;
          has_measured = true;
          this.pending = desired_lod_step !== committed_lod_step || source_changed;
          return;
        }

        // Real ground height at this chunk's own center, not an assumed 0 —
        // chunk_stage.tsx's own Chunk gets this for free once it has
        // actually built geometry to measure a real AABB from; decor never
        // builds anything chunk-shaped to measure, so it has to sample the
        // same elevation data its own candidates already use instead, or a
        // hillside chunk's distance would be measured as if it sat at sea
        // level while the trees on it are genuinely much closer to (or
        // farther from) the camera than that.
        const ele = get_ele_at_local(page_idx, base_x * U_PER_CELL + half, base_y * U_PER_CELL + half) ?? 0;
        const dist = chunk_distance_to_camera(group, half, half, ele, camera, center);
        this.distance2 = dist * dist;
        const guide_step = get_lod_guide(dist, committed_lod_step).lod_step;
        desired_lod_step = has_measured ? step_lod_toward(committed_lod_step, guide_step) : guide_step;
        has_measured = true;
        this.pending = desired_lod_step !== committed_lod_step || source_changed;
      },
      commit() {
        if (!this.pending) return false;
        committed_lod_step = desired_lod_step;
        committed_tile = tile_ref.current;
        committed_objects_revision = objects_revision_ref.current;
        this.pending = false;
        set_instances_by_variant(build_decor_instances(committed_tile, committed_lod_step, page_idx, base_x, base_y, bundle));
        return true;
      },
    };
    // deno-lint-ignore-next-line
  }, [page_idx, base_x, base_y, bundle]);

  useEffect(() => {
    scheduler.controls.add(control);
    return () => void scheduler.controls.delete(control);
  }, [scheduler, control]);

  // lod_group_ref below exists ONLY to give chunk_lod.ts a real mounted
  // group to measure through — it carries no meshes of its own. Actual
  // decor instances stay unpositioned by any wrapper group here, unlike
  // chunk_stage.tsx's own Chunk: scatter_decor_candidates already returns
  // page-absolute cell_x/cell_y (it scans starting from tx0 = base_x, not
  // from 0), and this component is mounted directly under PageDecor's
  // <AnchorNode i={page_base}> — so candidate positions are already correct
  // in that frame as-is. Wrapping the meshes themselves in lod_group_ref's
  // [base_x, base_y] offset would double-apply this chunk's own base,
  // landing every instance on a different cell than the one its own height
  // (get_ele_at_local, computed from the true x/y above) was sampled from.
  return (
    <>
      <group ref={lod_group_ref} position={[base_x * U_PER_CELL, base_y * U_PER_CELL, 0]} />
      {geometries.map((geometry, variant) => {
        const material = materials[variant];
        return geometry && material && instances_by_variant[variant].length > 0
          ? <DecorVariantMesh key={variant} geometry={geometry} material={material} instances={instances_by_variant[variant]} />
          : null;
      })}
    </>
  );
});
ChunkDecor.displayName = "ChunkDecor";

type ModelInstanceTransformList = { position: readonly [f64, f64, f64]; rotation_z: f64; scale: f64 }[];

type DecorVariantMeshProps = {
  geometry: THREE.BufferGeometry;
  material: THREE.MeshBasicMaterial;
  instances: ModelInstanceTransformList;
};

/** One InstancedMesh per (chunk, variant) pair with at least one instance —
 * the instanceMatrix buffer is replaced wholesale on every change (never
 * setMatrixAt-updated in place), same reasoning chunk_stage.tsx's own
 * ChunkInstanceMesh replaces its InstancedBufferAttribute outright on
 * rebuild: an InstancedMesh's buffer is fixed-size at construction, and
 * this chunk's own instance count for a variant can grow or shrink between
 * renders (a placed/removed object flipping candidates back in or out). */
const DecorVariantMesh = memo(({ geometry, material, instances }: DecorVariantMeshProps) => {
  const mesh = useMemo(() => {
    const m = new THREE.InstancedMesh(geometry, material, instances.length);
    const scratch = new THREE.Matrix4();
    for (let i = 0; i < instances.length; i++) {
      compose_instance_matrix(scratch, instances[i]);
      m.setMatrixAt(i, scratch);
    }
    m.instanceMatrix.needsUpdate = true;
    m.computeBoundingSphere();
    return m;
  }, [geometry, material, instances]);
  return <primitive object={mesh} />;
});
DecorVariantMesh.displayName = "DecorVariantMesh";

type PageDecorProps = {
  page_idx: IVec;
  range_anchor: IVec;
  bundle: DecorKindBundle;
  geometries: readonly (THREE.BufferGeometry | null)[];
  materials: readonly (THREE.MeshBasicMaterial | null)[];
};

const PageDecor = memo(({ page_idx, range_anchor, bundle, geometries, materials }: PageDecorProps) => {
  const page_base = FROM_L3_IDX(page_idx);
  const offsets = useMemo(() => in_range_chunks(page_base, range_anchor), [page_base, range_anchor]);
  return (
    <AnchorNode i={page_base}>
      <group name={`decor-page/${ivec.repr(page_idx)}`}>
        {offsets.map((offset) => <ChunkDecor key={offset.key} page_idx={page_idx} base_x={offset.base_x} base_y={offset.base_y} bundle={bundle} geometries={geometries} materials={materials} />)}
      </group>
    </AnchorNode>
  );
});
PageDecor.displayName = "PageDecor";

export type DecorLayerProps = { bundle: DecorKindBundle };

/** Mount alongside ChunkStage (world.tsx) — same AnchorSpace, so this
 * shares the same floating-origin frame every other world layer does. */
export const DecorLayer = memo(({ bundle }: DecorLayerProps) => {
  const map_center = useMapStore((state) => state.map_center);
  const tile_map = useTileStore((state) => state.tile_map);
  const { geometries, materials } = useInstancedModelVariants(bundle.variants);

  const center_page_idx = map_center === null ? null : L3_IDX(map_center.i);
  const center_page_key = center_page_idx?.toString() ?? null;
  const tiles = useMemo(() => Array.from(tile_map.keys()), [tile_map]);
  // Mirrors ChunkStage's own visible_pages selection (chunk_stage.tsx) —
  // see this file's own doc comment for why that's duplicated rather than
  // shared.
  const visible_pages = useMemo(() => {
    if (center_page_idx === null) return tiles;
    return tiles
      .map((page_idx) => ({ page_idx, dist: CHEBYSHEV_DIST(ivec.sub(page_idx, center_page_idx)) }))
      .filter(({ dist }) => dist <= RENDER_RADIUS)
      .map(({ page_idx }) => page_idx);
    // deno-lint-ignore-next-line
  }, [tiles, center_page_key]);

  const anchor_x = map_center === null ? null : ivec.X(map_center.i) & ~63n;
  const anchor_y = map_center === null ? null : ivec.Y(map_center.i) & ~63n;
  const range_anchor_key = anchor_x === null ? null : `${anchor_x},${anchor_y}`;
  const range_anchor = useMemo(
    () => anchor_x === null || anchor_y === null ? null : ivec.gather(anchor_x, anchor_y, 0n),
    // deno-lint-ignore-next-line
    [range_anchor_key],
  );

  if (range_anchor === null) return null;
  return (
    <group name="decor-layer">
      {visible_pages.map((page_idx) => <PageDecor key={page_idx.toString()} page_idx={page_idx} range_anchor={range_anchor} bundle={bundle} geometries={geometries} materials={materials} />)}
    </group>
  );
});
DecorLayer.displayName = "DecorLayer";
