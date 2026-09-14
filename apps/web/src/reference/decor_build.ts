// @pxl/web/reference/decor_build.ts

/**
 * The TypeScript build of a chunk's decor at one tier, the reference the
 * module's (crates/pxl-core/src/world/decor.rs) is held to instance for
 * instance by core_decor.test.ts — the scatter's streams bit for bit — and
 * the account of why each rule is what it is. The kinds and the density
 * falloff are module/world/decor/decor_kinds.ts's.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { Tile } from "@pxl/shared/bake/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { derive_seed_u64 } from "@xpute/core/kit/rng.ts";
import { decor_id_from_num, object_value_is_decor } from "@pxl/shared/spec/cell/mod.ts";
import { useSettingsStore } from "@/app/setting/store.ts";
import { get_ele_at_local } from "@/module/world/terrain/elevation.ts";
import { cell_index, cell_is_occupied, each_column_entry, object_column_of, virtual_hidden_id } from "@/module/world/object/store.ts";
import { type LodStep } from "@/module/world/lod.ts";
import { scatter_decor_candidates } from "@/module/world/decor/scatter.ts";
import { type DecorInstances, HIDDEN_FLAG, instances_push, instances_reset, instances_take, NO_ENTRY } from "@/reference/decor_instance.ts";
import { decor_density, DECOR_MASTER_SEED, type DecorKindBundle, kind_extent, user_decor_pick } from "@/module/world/decor/decor_kinds.ts";

/** A chunk's instances at one step, one lane set per variant (null where a
 * variant has none). */
export type DecorInstancesByVariant = (DecorInstances | null)[];

// User-placed instances of this bundle's own decor kind (a tree a player
// planted via mode_bar.tsx's decor button) — merged in unconditionally,
// never gated by density_scale: that falloff exists to thin PROCEDURAL
// clutter at distance, not to hide deliberate user content, the same
// reasoning ObjectWorldLayer's own solid cubes never fade with LOD either.
// Real geometry (this instancing path), not a chunk_mesher.ts box — see
// object/store.ts's own "decor" channel and ObjectWorldLayer.paint's own
// decor skip. The "user" half of the same system/user split
// TerrainWorldLayer/ObjectWorldLayer keep as two classes (terrain_layer.ts,
// object_layer.ts) — kept as a plain function here instead of a
// matching class of its own: nothing polymorphically iterates "every decor
// source" the way WorldLayerStack iterates world layers, so there's no
// shared interface for a class split to actually buy anything.
function collect_user_decor_instances(
  variant_wanted: number,
  bundle: DecorKindBundle,
  page_idx: IVec,
  base_x: number,
  base_y: number,
): void {
  if (!bundle.decor_id) return;
  const page_off = ivec.gather(BigInt(base_x), BigInt(base_y), 0n);
  const hidden_id = virtual_hidden_id();
  each_column_entry(object_column_of(page_idx, page_off), (entry) => {
    if (!object_value_is_decor(entry.state) || decor_id_from_num(entry.state) !== bundle.decor_id) return;
    const { variant, rotation } = user_decor_pick(entry.corner().i, bundle.variants.length);
    if (variant !== variant_wanted) return;
    // +size/2 on X/Y: the entry is a cell's min corner, but a free-standing
    // model plants itself at the middle of its footprint — the same centre a
    // scattered tree gets below (`+ U_PER_CELL / 2`). Z stays on the slot: a
    // decor model's origin is at its base. entry.size is DECOR_SIZE (one
    // whole cell) in practice; written as size/2 so a sub-cell decor tier
    // still centres in its own footprint.
    const hcenter = entry.size / 2;
    // Only the hidden check needs the string id, and only while something is
    // hidden at all.
    const hidden = hidden_id !== null && entry.id() === hidden_id;
    // The cursor's corner is already chunk-local — the same frame chunk
    // content is built in (build_decor_instances' own doc comment).
    instances_push(entry.lx + hcenter, entry.ly + hcenter, entry.lz, rotation, 1, hidden ? HIDDEN_FLAG : 0, entry.band_of, entry.pos_of);
  });
}

// The "system/procedural" half — scatter_decor_candidates' own output,
// thinned by density_scale, filtered against real elevation and existing
// occupancy. Mirrors TerrainWorldLayer's own role (system-generated content,
// elevation-derived Z) the same way collect_user_decor_instances above
// mirrors ObjectWorldLayer's (user-stored content, stored Z) — see this
// file's own top doc comment and the 2026-09-06 industry-comparison
// discussion for the fuller account of that symmetry.
function collect_procedural_decor_instances(
  variant_wanted: number,
  bundle: DecorKindBundle,
  tile: Tile | undefined,
  lod_step: LodStep,
  page_idx: IVec,
  page_base: IVec,
  base_x: number,
  base_y: number,
): void {
  if (!useSettingsStore.getState().show_decor) return;
  const density_scale = decor_density(kind_extent(bundle), lod_step);
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
    if (candidate.variant !== variant_wanted) continue;
    const x: f64 = candidate.cell_x * U_PER_CELL + U_PER_CELL / 2;
    const y: f64 = candidate.cell_y * U_PER_CELL + U_PER_CELL / 2;
    const ele = get_ele_at_local(page_idx, x, y);
    if (ele === null) continue;

    const cell_ivec = ivec.add(page_base, ivec.gather(BigInt(candidate.cell_x), BigInt(candidate.cell_y), BigInt(Math.floor(ele))));
    const [ix, iy, iz] = cell_index({ i: cell_ivec, f: 0n });
    if (cell_is_occupied(ix, iy, iz)) continue;

    // NO_ENTRY — procedural placements are never stored (this file's own
    // top doc comment), so there's nothing for decor_hit.ts's raycast_decor
    // to resolve a hit back to; they stay transparent to hit-testing.
    // Chunk-local position (see build_decor_instances' own doc comment) —
    // x/y above stay page-relative for the elevation lookup right above
    // this, shifted only in the value actually stored.
    instances_push(x - base_x * U_PER_CELL, y - base_y * U_PER_CELL, ele, candidate.rotation, candidate.scale, 0, NO_ENTRY, 0);
  }
}

/** Pure "what would this chunk look like at this LOD tier, given this
 * source data" computation — the reference the module's build is held to,
 * with every input a chunk's node has in scope.
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
): DecorInstancesByVariant {
  // One variant at a time through the shared scratch: the scatter is
  // re-run per variant, which costs a candidate walk per variant instead of
  // a per-instance object, and a bundle has a handful of variants.
  const page_base = FROM_L3_IDX(page_idx);
  return bundle.variants.map((_v, variant) => {
    instances_reset();
    collect_user_decor_instances(variant, bundle, page_idx, base_x, base_y);
    collect_procedural_decor_instances(variant, bundle, tile, lod_step, page_idx, page_base, base_x, base_y);
    return instances_take();
  });
}
