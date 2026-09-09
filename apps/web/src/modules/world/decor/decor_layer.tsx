// @pxl/web/modules/world/decor/decor_layer.tsx

/**
 * Decor's own pure build logic and rendering primitive — bakes real GLB
 * geometry across every visible chunk, the same "each block contributes its
 * own real geometry into one merged per-chunk buffer" model chunk_mesher.ts
 * uses for cube content (instanced_model.ts's own bake_decor_instances,
 * TIER-1 UNIFICATION below).
 *
 * As of 2026-09-05's second pass (see the merged-scheduling paragraph
 * below), decor no longer owns its own scheduled component tree: this file
 * exports build_decor_instances (a pure function) and DecorVariantMesh (a
 * rendering primitive), and chunk_stage.tsx's own Chunk calls/renders them
 * directly, alongside its own cube build, as part of ONE build step. What
 * used to live here — ChunkDecor/PageDecor/DecorLayer, each with their own
 * ChunkLodControl and independently-computed distance/LOD-tier guess — is
 * gone; Chunk already has everything build_decor_instances needs (tile,
 * page_idx, base_x/base_y, objects_revision, and now its own resolved LOD
 * tier) in scope from building its own cube content, so folding the call in
 * removed a whole parallel copy of that bookkeeping rather than adding a new
 * one. See discussions/20260905-tier1-geometry-baking-implemented.md and its
 * own follow-up doc for the fuller account of why (the terrain/decor
 * ready-state desync this fixes) and what changed.
 *
 * No decoration is ever stored: scatter.ts's placement is a pure function
 * of terrain + a fixed seed, recomputed on demand. A cell that currently
 * holds a user-placed object simply has its candidate filtered out at
 * render time (get_objects_in_cell, exact-cell — not get_objects_near's
 * ±1-cell neighborhood); removing that object clears the very same filter
 * on the next objects-revision-triggered recompute, so the decoration
 * reappears with no bookkeeping of its own.
 *
 * Density DOES already fall off with distance, snapped to the exact same
 * LOD tiers terrain hands off at — not by reimplementing "distance to
 * camera" a second time (render/chunk_lod.ts's own doc comment explains
 * why that drifts out of sync with what terrain is actually showing), but
 * by using Chunk's own already-resolved `primary` tier directly (now the
 * literal same value, not just the same arithmetic run twice — a direct
 * structural consequence of sharing one build step). See
 * DECOR_DENSITY_BY_LOD_STEP below for the actual falloff curve and why it's
 * steep rather than a gentle taper.
 *
 * LOD BLEND — DONE (2026-09-06). Decor density stepping down a tier
 * (1 -> 0.5 -> 0.25 -> 0) used to pop synchronized across every chunk that
 * crossed a threshold in the same frame — a whole ring of forest visibly
 * thinning at once. Now mirrors chunk_stage.tsx's own cube-content blend
 * (get_lod_pair_visibility's stochastic finer/coarser split, render/lod.ts):
 * inside a threshold's own band, a chunk holds BOTH density levels' own
 * built instance sets (chunk_stage.tsx's DecorLayoutPair) and shows exactly
 * one, chosen by comparing the chunk's own stable hash against how far the
 * camera has moved through the band — staggering which chunks have already
 * switched across a range of distances instead of all of them switching on
 * the same frame. This does NOT make an individual tree fade or shrink —
 * decor still never blends an instance's own scale/opacity with distance
 * (see DECOR_DENSITY_BY_LOD_STEP below) — it only smears WHERE the
 * population-level thinning happens across space, the same thing cube
 * content's own blend does for its tier pop. Toggled via a plain
 * `THREE.Object3D.visible` flip (render/lod_visibility.ts), not a per-vertex
 * attribute the way cube content's own a_visible works: decor's blend is
 * always whole-mesh (an entire density level's own built set, on or off),
 * so there was never a need for cube's finer per-vertex granularity here.
 *
 * TIER-1 UNIFICATION — DONE (2026-09-05), Minecraft's own block model as the
 * literal reference, not just an inspiration. It's a 3-tier split, not 2 —
 * (1) static grid content (full cubes AND non-cube decorative content like
 * trees alike) bakes into one merged per-chunk mesh, no instancing at all;
 * (2) static content that still needs a per-frame visual update (an open
 * chest, sign text) would keep that (1) baked mesh and add a thin renderer
 * on top for just the dynamic part — no such content exists yet, so this
 * tier is unimplemented, not merely deferred; (3) anything that moves under
 * its own logic every frame (mobs, minecarts) lives entirely outside the
 * grid as its own object — already matched here by characters/companion.tsx's
 * own useFrame loop, untouched by any of this. Both cube content and decor
 * now bake real geometry (chunk_mesher.ts / this file's own
 * bake_decor_instances) — every block, cube or not, contributing its own
 * real geometry into a merged per-chunk buffer, not a shared primitive
 * restyled per instance. The SAME-DAY follow-up (also 2026-09-05) finished
 * the other half of "one build step, one ready flag" that TIER-1 UNIFICATION
 * deliberately left open: chunk_stage.tsx's own Chunk now builds decor
 * alongside cube content in the exact same commit(), instead of two
 * separately-scheduled components racing each other — see this file's own
 * top paragraph. User-placed decor (a tree a player plants via
 * mode_bar.tsx's decor button, as opposed to a procedurally scattered one)
 * lands in objects/store.ts's existing per-cell tree as its own
 * ObjectLayerEntry kind ("decor") alongside "solid" — same tier-1 storage
 * Minecraft gives decorative blocks — with its instances merged into
 * build_decor_instances' own output next to scatter_decor_candidates',
 * not a separate store.
 */

import { memo, useEffect, useLayoutEffect, useMemo } from "react";
import * as THREE from "three";

import type { f64 } from "@xpute/core/kit/word.ts";
import type { Tile } from "@pxl/shared/bake/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { fvec, type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { derive_seed_u64, make_rng, u32_to_unit } from "@xpute/core/kit/rng.ts";
import type { DecorId } from "@pxl/shared/spec/cell/mod.ts";

import { useSettingsStore } from "@/app/settings/settings_store.ts";
import { rim_cfg } from "@/modules/world/environment/config.ts";
import { set_rim_intensity } from "@/modules/world/render/lit_gltf_material.ts";
import { get_ele_at_local } from "@/modules/world/terrain/elevation.ts";
import { cell_index, cell_key, get_objects_in_cell, get_render_object_entries_in_l2_column, l2_column_key_of } from "@/modules/world/objects/store.ts";
import type { LodStep } from "@/render/lod.ts";
import type { ChunkLodRole } from "@/render/layer/pipeline/chunk_bucket.ts";
import { type ChunkMeshRegistry, type MountedLodMesh } from "@/render/lod_visibility.ts";

import { type DecorEligibility, type DecorKindSeed, scatter_decor_candidates } from "./scatter.ts";
import { bake_decor_instances, type DecorVariantAsset, type ModelInstanceTransform, useInstancedModelVariants } from "./instanced_model.ts";
import { register_decor_mesh } from "./decor_hit.ts";

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
  /** Which objects/store.ts "decor" channel value a user-placed instance of
   * this kind is tagged with (cell/decor.ts's decor_types) — links
   * mode_bar.tsx's own decor button back to this bundle's variant list, so a
   * placed tree renders through the exact same instancing path a scattered
   * one does. See build_decor_instances' own doc comment. Omitted entirely
   * for a kind with no user-placeable equivalent yet (boats/cars/trains
   * today — scattered-only, no mode_bar button of their own). */
  decor_id?: DecorId;
};

// Seeded purely by the placed object's own absolute position — unlike
// scatter_decor_candidates' own per-cell RNG (page_seed + cell_x/cell_y),
// there's no stable "page seed" concept to key a user placement off: the
// object itself, not a terrain cell, is the one stable identity a placement
// keeps across reloads (objects/store.ts persists it verbatim). Reuses
// DECOR_MASTER_SEED as the same root every procedural stream already shares,
// so a user-placed tree's own variant/heading pick lives in the same seed
// space, not a parallel one.
//
// Exported: markers/hover_cursor.tsx's own ghost preview calls this too, at
// the exact same `i` a placement/hover resolves to, so the ghost shows the
// real variant/heading a real placement would get (or, for an already-
// placed entry, the real one it actually got) instead of an arbitrary
// stand-in — this function being pure and keyed only on `i` is exactly what
// makes that prediction exact rather than a guess.
export function user_decor_pick(i: IVec, variant_count: number): { variant: number; rotation: number } {
  const rng = make_rng(derive_seed_u64(DECOR_MASTER_SEED, i));
  const variant = Math.min(Math.floor(u32_to_unit(rng()) * variant_count), variant_count - 1);
  const rotation = u32_to_unit(rng()) * Math.PI * 2;
  return { variant, rotation };
}

export type ModelInstanceTransformList = ModelInstanceTransform[];

// User-placed instances of this bundle's own decor kind (a tree a player
// planted via mode_bar.tsx's decor button) — merged in unconditionally,
// never gated by density_scale: that falloff exists to thin PROCEDURAL
// clutter at distance, not to hide deliberate user content, the same
// reasoning ObjectWorldLayer's own solid cubes never fade with LOD either.
// Real geometry (this instancing path), not a chunk_mesher.ts box — see
// objects/store.ts's own "decor" channel and ObjectWorldLayer.paint's own
// decor skip. The "user" half of the same system/user split
// TerrainWorldLayer/ObjectWorldLayer keep as two classes (render/layer/
// terrain, objects/adapter.ts) — kept as a plain function here instead of a
// matching class of its own: nothing polymorphically iterates "every decor
// source" the way WorldLayerStack iterates world layers, so there's no
// shared interface for a class split to actually buy anything.
function collect_user_decor_instances(
  by_variant: ModelInstanceTransformList[],
  bundle: DecorKindBundle,
  page_idx: IVec,
  page_base: IVec,
  base_x: number,
  base_y: number,
): void {
  if (!bundle.decor_id) return;
  const page_off = ivec.gather(BigInt(base_x), BigInt(base_y), 0n);
  const l2_key = l2_column_key_of(page_idx, page_off);
  for (const entry of get_render_object_entries_in_l2_column(l2_key)) {
    if (entry.kind !== "decor" || entry.decor_id !== bundle.decor_id) continue;
    const local = ivec.sub(entry.i, page_base);
    const x: f64 = ivec.to_real(ivec.X(local)) + fvec.to_real(fvec.X(entry.f));
    const y: f64 = ivec.to_real(ivec.Y(local)) + fvec.to_real(fvec.Y(entry.f));
    const z: f64 = ivec.to_real(ivec.Z(local)) + fvec.to_real(fvec.Z(entry.f));
    const { variant, rotation } = user_decor_pick(entry.i, bundle.variants.length);
    // +size/2 on X/Y: the entry stores a cell's min corner (objects/store.ts),
    // but a free-standing model plants itself at the middle of that footprint
    // — the same center collect_procedural_decor_instances below places a
    // scattered tree at (`+ U_PER_CELL / 2`), and the X/Y half of what
    // objects/geometry.ts's anchor_center does for the pointer tap (Z stays
    // on the slot here — a decor model's origin is at its base, not its
    // middle). entry.size is DECOR_SIZE (one whole cell) in practice, so this
    // is U_PER_CELL / 2; written as size/2 so a future sub-cell decor tier
    // still centers in its own footprint rather than a whole cell's.
    const hcenter = entry.size / 2;
    // Chunk-local: see build_decor_instances' own doc comment on why the
    // page-relative x/y computed above gets shifted here, at the last
    // possible moment, rather than earlier (everything upstream — the
    // entry's own absolute i/f, page_base — stays in its own natural frame;
    // only the final stored position changes convention).
    by_variant[variant].push({ position: [x - base_x * U_PER_CELL + hcenter, y - base_y * U_PER_CELL + hcenter, z], rotation_z: rotation, scale: 1, id: entry.id });
  }
}

// The "system/procedural" half — scatter_decor_candidates' own output,
// thinned by density_scale, filtered against real elevation and existing
// occupancy. Mirrors TerrainWorldLayer's own role (system-generated content,
// elevation-derived Z) the same way collect_user_decor_instances above
// mirrors ObjectWorldLayer's (user-stored content, stored Z) — see this
// file's own top doc comment and the 2026-09-06 industry-comparison
// discussion for the fuller account of that symmetry.
function collect_procedural_decor_instances(
  by_variant: ModelInstanceTransformList[],
  bundle: DecorKindBundle,
  tile: Tile | undefined,
  lod_step: LodStep,
  page_idx: IVec,
  page_base: IVec,
  base_x: number,
  base_y: number,
): void {
  const density_scale = DECOR_DENSITY_BY_LOD_STEP[lod_step];
  // Skip the O(CHUNK_CELL_SPAN^2) eligibility+RNG scan entirely at zero
  // density instead of running it only to have every candidate rejected by
  // its own probability check below — density_scale=0 means "nothing here",
  // decided once, not discovered 4096 times.
  if (!tile || density_scale === 0) return;

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

  for (const candidate of candidates) {
    const x: f64 = candidate.cell_x * U_PER_CELL + U_PER_CELL / 2;
    const y: f64 = candidate.cell_y * U_PER_CELL + U_PER_CELL / 2;
    const ele = get_ele_at_local(page_idx, x, y);
    if (ele === null) continue;

    const cell_ivec = ivec.add(page_base, ivec.gather(BigInt(candidate.cell_x), BigInt(candidate.cell_y), BigInt(Math.floor(ele))));
    const [ix, iy, iz] = cell_index({ i: cell_ivec, f: 0n });
    if (get_objects_in_cell(cell_key(ix, iy, iz)).length > 0) continue;

    // id: null — procedural placements are never stored (this file's own
    // top doc comment), so there's nothing for decor_hit.ts's raycast_decor
    // to resolve a hit back to; they stay transparent to hit-testing.
    // Chunk-local position (see build_decor_instances' own doc comment) —
    // x/y above stay page-relative for the elevation lookup right above
    // this, shifted only in the value actually stored.
    by_variant[candidate.variant].push({
      position: [x - base_x * U_PER_CELL, y - base_y * U_PER_CELL, ele],
      rotation_z: candidate.rotation,
      scale: candidate.scale,
      id: null,
    });
  }
}

/** Pure "what would this chunk look like at this LOD tier, given this
 * source data" computation — called directly from chunk_stage.tsx's own
 * Chunk (build_ref.current(), alongside its own cube build), which already
 * has every one of these inputs in scope from building its own content.
 *
 * Returned positions are chunk-local (relative to this chunk's own
 * [base_x, base_y] corner), matching chunk_mesher.ts's own baked cube
 * positions exactly — both render inside the same offset `<group>` in
 * Chunk's own JSX (chunk_stage.tsx), one coordinate convention for
 * everything a chunk renders, not two. Elevation sampling
 * (get_ele_at_local) and occupancy checks (get_objects_in_cell) still work
 * in absolute/page-relative terms internally, exactly as before — only the
 * final position actually pushed into by_variant is shifted to chunk-local,
 * right at the point of return, so nothing upstream of that needs to change
 * meaning. */
export function build_decor_instances(
  tile: Tile | undefined,
  lod_step: LodStep,
  page_idx: IVec,
  base_x: number,
  base_y: number,
  bundle: DecorKindBundle,
): ModelInstanceTransformList[] {
  const by_variant: ModelInstanceTransformList[] = bundle.variants.map(() => []);
  const page_base = FROM_L3_IDX(page_idx);
  collect_user_decor_instances(by_variant, bundle, page_idx, page_base, base_x, base_y);
  collect_procedural_decor_instances(by_variant, bundle, tile, lod_step, page_idx, page_base, base_x, base_y);
  return by_variant;
}

/* DecorVariantMesh lived here — one React component per (chunk, bundle,
 * variant, role), thousands live, whose whole body was a useMemo owning a
 * THREE.Mesh and a <primitive> rendering it. chunk_node.ts attaches those
 * meshes directly now (attach_decor), for the same reason the cube meshes
 * moved: a component that only owns a scene object buys nothing and costs a
 * deferred mount. bake_decor_instances and register_decor_mesh, the two
 * things it actually called, are unchanged and now called from there. */

export type DecorBundleAssets = {
  bundle: DecorKindBundle;
  geometries: readonly (THREE.BufferGeometry | null)[];
  materials: readonly (THREE.MeshStandardMaterial | null)[];
};

/**
 * Loads every decor kind's own GLB geometry/skin in exactly one
 * useInstancedModelVariants call, regardless of how many bundles there are.
 * chunk_stage.tsx's own ChunkStage can't call that hook once per bundle in a
 * .map() — React's rules of hooks forbid a variable number of hook calls
 * per component — so this flattens every bundle's own variants into one
 * combined list first, then re-slices the single call's own results back
 * into per-bundle groups using each bundle's own variants.length as the
 * boundary: the same load, just grouped back the way callers actually need
 * it. `bundles` should be a stable (module-level constant) reference — see
 * world.tsx's own DECOR_BUNDLES — so this never needlessly re-slices.
 */
export function useAllDecorBundleAssets(bundles: readonly DecorKindBundle[]): readonly DecorBundleAssets[] {
  const all_variants = useMemo(() => bundles.flatMap((bundle) => bundle.variants), [bundles]);
  const { geometries, materials } = useInstancedModelVariants(all_variants);

  // instanced_model.ts's own material deliberately doesn't know the live
  // show_rim_light setting (importing app/settings/settings_store.ts there
  // would drag deno test into a module graph it can't load — see that
  // file's own top doc comment) — this is the one real (non-test) consumer
  // that needs it live, so it owns the subscription instead, updating every
  // currently-loaded material's own compiled shader uniform directly.
  // Cheaper than each of the (potentially many, one per chunk)
  // DecorVariantMesh consumers subscribing individually, and correct
  // regardless of how many chunks currently render a given variant: the
  // material object itself is the same one shared across all of them
  // (bundle_assets is computed once here, at ChunkStage level).
  const show_rim_light = useSettingsStore((state) => state.show_rim_light);
  useEffect(() => {
    for (const material of materials) {
      if (material) set_rim_intensity(material, show_rim_light ? rim_cfg.INTENSITY : 0);
    }
  }, [materials, show_rim_light]);

  return useMemo(() => {
    const out: DecorBundleAssets[] = [];
    let offset = 0;
    for (const bundle of bundles) {
      const count = bundle.variants.length;
      out.push({ bundle, geometries: geometries.slice(offset, offset + count), materials: materials.slice(offset, offset + count) });
      offset += count;
    }
    return out;
  }, [bundles, geometries, materials]);
}
