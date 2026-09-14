// @pxl/web/module/world/decor/decor_layer.ts

/**
 * Decor's own pure build logic — what real GLB models stand where, across
 * every visible chunk, at the tier that chunk is showing. Drawing them is
 * field.ts's (one instanced mesh per variant for the whole world); this file
 * decides only what exists.
 *
 * What is drawn is built by the module (crates/pxl-core/src/decor.rs, a
 * block of lanes a chunk and tier, handed to chunk_node.ts) from the kinds
 * this file teaches it (teach_decor_kinds); build_decor_instances below is
 * the TypeScript build the module's is held to (core_decor.test.ts), and
 * the account of why each rule is what it is.
 *
 * As of 2026-09-05's second pass (see the merged-scheduling paragraph
 * below), decor no longer owns its own scheduled component tree: this file
 * exports build_decor_instances (a pure function) and DecorVariantMesh (a
 * rendering primitive), and chunk_stage.tsx's own Chunk calls/renders them
 * directly, alongside its own cube build, as part of ONE build step. What
 * used to live here — ChunkDecor/PageDecor/DecorLayer, each with their own
 * scheduler slot and independently-computed distance/LOD-tier guess — is
 * gone; Chunk already has everything build_decor_instances needs (tile,
 * page_idx, base_x/base_y, objects_revision, and now its own resolved LOD
 * tier) in scope from building its own cube content, so folding the call in
 * removed a whole parallel copy of that bookkeeping rather than adding a new
 * one. See issue/20260905-tier1-geometry-baking-implemented.md and its
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
 * decor_density below for the falloff, which is the terrain tiers' own
 * projected-size criterion applied to the kind's extent.
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
 * (see decor_density below) — it only smears WHERE the
 * population-level thinning happens across space, the same thing cube
 * content's own blend does for its tier pop. Which of the two a chunk shows
 * is which one it contributes to field.ts, not a flag on a mesh holding the
 * other: decor's blend is whole-set (an entire density level, on or off), so
 * it never wanted cube content's per-vertex granularity, and once the mesh
 * stopped belonging to the chunk there was nothing left to flag.
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
 * grid as its own object — already matched here by character/companion.tsx's
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
 * lands in object/store.ts's existing per-cell tree as its own
 * ObjectLayerEntry kind ("decor") alongside "solid" — same tier-1 storage
 * Minecraft gives decorative blocks — with its instances merged into
 * build_decor_instances' own output next to scatter_decor_candidates',
 * not a separate store.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import type { Tile } from "@pxl/shared/bake/format.ts";
import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { derive_seed_u64, make_rng, u32_to_unit } from "@xpute/core/kit/rng.ts";
import { decor_id_from_num, decor_num_id, type DecorId, object_value_is_decor } from "@pxl/shared/spec/cell/mod.ts";

import { shipped_palette } from "@pxl/shared/record/mod.ts";

import { asset_url } from "@/lib/env/route.ts";
import { useSettingsStore } from "@/app/setting/store.ts";
import { get_ele_at_local } from "@/module/world/terrain/elevation.ts";
import { cell_index, cell_is_occupied, each_column_entry, object_column_of, virtual_hidden_id } from "@/module/world/object/store.ts";
import { type LodStep, TARGET_CELL_PX } from "@/render/lod.ts";
import { CORE_PXL } from "@/render/layer/pipeline/core_pool.ts";

import { DECOR_RULE_CHANNELS, type DecorEligibility, type DecorKindSeed, type DecorRules, scatter_decor_candidates } from "./scatter.ts";
import { type DecorInstances, HIDDEN_FLAG, instances_push, instances_reset, instances_take, NO_ENTRY } from "./instance.ts";

// Fixed, arbitrary — the one thing every DecorKind's own placement
// ultimately derives from (alongside its own DecorKindSeed.tag). Changing
// this reshuffles every decoration in the world; there is no reason to,
// since a page's own idx already makes every page's stream independent.
const DECOR_MASTER_SEED = 0xd0c0d0c0_dec0_5eed_0001n;

/**
 * How far a kind is drawn, from its size and nothing else.
 *
 * One criterion for every kind, the terrain tiers' own: what decides
 * whether a thing is drawn is how large it is to the eye
 * (issue/20260911-decor-identity-orientation-and-one-cell-format.md,
 * the 2026-09-12 follow-up). A kind brings its extent — the largest
 * dimension of its models, in world units — and never a threshold. At the
 * far edge of a tier's band, a cell projects to TARGET_CELL_PX (render/
 * lod.ts); an extent of E cells projects to E times that over the step. A
 * thing that would cover fewer pixels than two of the tier's cells is not
 * drawn: at that size its absence makes no visible difference, and the
 * tier's cubes are already coarser than it. The ratio is
 * viewport-independent, since the thresholds move with the projection and
 * the cell's pixel size does not. Twelve pixels at the reference viewport
 * — the middle of the range at which Google's map products stop drawing a
 * building in 3D (2026-09-13, decided by eye against the alternative of
 * one cell, which kept trees and cars a tier further out).
 *
 * No fade: a kind is drawn at full density up to its last tier and not
 * past it, the way those clients drop a building rather than thin a
 * district. What softens the edge is the blend across a band boundary
 * (below), which staggers the tier change across chunks. The models
 * shipped today (extents 2.6 to 3.9) are all drawn on the nearest tier
 * only; a ten-unit building would be drawn to the fourth.
 *
 * This replaced a hand-written ladder (1, 1/2, 1/4, 0 by step, and "nearest
 * tier only" for the kinds that cost more) which set thresholds per kind
 * by cost. Cost is the low-poly tier's problem, not the criterion's; a
 * tree and a car of the same size are drawn to the same range.
 */
const DECOR_GONE_PX: f64 = TARGET_CELL_PX * 2;

/** The density a kind is scattered at on a tier: 1, or 0 when it is not
 * drawn there. */
export function decor_density(extent_u: f64, lod_step: LodStep): f64 {
  const projected_px = (extent_u / U_PER_CELL) * TARGET_CELL_PX / lod_step;
  return projected_px < DECOR_GONE_PX ? 0 : 1;
}

/** One variant of a kind: its model and the skin it is drawn with. */
export type DecorVariantAsset = {
  model_url: string;
  /** Not necessarily shared across a kind's other variants: a kind's
   * variants may come from different packs with different colormaps. */
  skin_url: string;
};

export type DecorKindBundle = {
  /** The kind's name — the bank key's prefix, and so the draw census's tag. */
  kind: string;
  seed: DecorKindSeed;
  /** Where the kind stands, as the module is taught it; `is_eligible` is
   * the same rule as the predicate this reference reads. */
  rules: DecorRules;
  is_eligible: DecorEligibility;
  /** One entry per variant_count, filled by `bind_palette` from the
   * palette's rows for this kind — each variant carries its own skin rather
   * than the whole kind
   * sharing one: a kind's variants can (and for trees, do) span more than
   * one Kenney pack, each with its own colormap atlas. */
  variants: readonly DecorVariantAsset[];
  /** Which object/store.ts "decor" channel value a user-placed instance of
   * this kind is tagged with (cell/decor.ts's decor_types) — links
   * mode_bar.tsx's own decor button back to this bundle's variant list, so a
   * placed tree renders through the exact same instancing path a scattered
   * one does. See build_decor_instances' own doc comment. Omitted entirely
   * for a kind with no user-placeable equivalent yet (boats/cars/trains
   * today — scattered-only, no mode_bar button of their own). */
  decor_id?: DecorId;
  /** The palette rows of this kind's variants, in variant order — the
   * shipped palette's `group` of the kind's name. Filled by `bind_palette`
   * alongside `variants`. */
  palette_rows?: readonly u32[];
};

/**
 * Binds a bundle to the palette: its variants — the palette's model rows
 * for this kind, in row order, each a model and a skin fetched by content
 * key — and the extent the range criterion reads, the largest dimension
 * over every variant, since the criterion asks how large the kind is to
 * the eye and a kind is drawn as any of its variants. The seed's
 * variant_count is checked against the palette at import so the scatter's
 * pick and the palette's list cannot drift apart silently.
 */
export function bind_palette(bundle: Omit<DecorKindBundle, "variants" | "palette_rows">): DecorKindBundle {
  const palette = shipped_palette();
  const rows = palette.rows_of_group(bundle.kind).filter((row) => palette.model_key_of(row) !== "");
  if (rows.length !== bundle.seed.variant_count) {
    throw new Error(`decor: palette lists ${rows.length} models for ${bundle.kind}, ${bundle.kind}_kind.ts's variant_count is ${bundle.seed.variant_count}`);
  }
  const variants = rows.map((row) => ({ model_url: asset_url(palette.model_key_of(row)), skin_url: asset_url(palette.skin_key_of(row)) }));
  return { ...bundle, variants, palette_rows: rows };
}

function kind_extent(bundle: DecorKindBundle): f64 {
  let extent = 0;
  for (const row of bundle.palette_rows ?? []) extent = Math.max(extent, shipped_palette().extent_max(row));
  return extent;
}

/**
 * Teaches the module every kind, in bundle order — the order of the groups
 * in the decor blocks it hands over (crates/pxl-core/src/decor.rs): each
 * kind's variants in variant order, one group a variant. Called once, from
 * decor_registry.ts, with the registry's own list.
 */
export function teach_decor_kinds(bundles: readonly DecorKindBundle[]): void {
  const pxl = CORE_PXL;
  if (pxl === null) return;
  pxl.pxl_decor_forget();
  for (const bundle of bundles) {
    const tag = BigInt.asUintN(64, bundle.seed.tag);
    const kind = pxl.pxl_decor_kind(
      Number(tag & 0xffffffffn),
      Number(tag >> 32n),
      bundle.seed.density,
      bundle.seed.variant_count,
      bundle.seed.orientation === "align_transport" ? 1 : 0,
      kind_extent(bundle),
      bundle.decor_id ? decor_num_id(bundle.decor_id) : -1,
    );
    DECOR_RULE_CHANNELS.forEach((channel, ch) => {
      const rule = bundle.rules[channel];
      if (rule === "any") return;
      pxl.pxl_decor_rule(kind, ch, rule === "void" ? 1 : 2);
      if (rule !== "void") { for (const t of rule) pxl.pxl_decor_allow(kind, ch, t); }
    });
  }
}

// Seeded purely by the placed object's own absolute position — unlike
// scatter_decor_candidates' own per-cell RNG (page_seed + cell_x/cell_y),
// there's no stable "page seed" concept to key a user placement off: the
// object itself, not a terrain cell, is the one stable identity a placement
// keeps across reloads (object/store.ts persists it verbatim). Reuses
// DECOR_MASTER_SEED as the same root every procedural stream already shares,
// so a user-placed tree's own variant/heading pick lives in the same seed
// space, not a parallel one.
//
// Exported: marker/hover_cursor.tsx's own ghost preview calls this too, at
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
// TerrainWorldLayer/ObjectWorldLayer keep as two classes (render/layer/
// terrain, object/adapter.ts) — kept as a plain function here instead of a
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

/* DecorVariantMesh lived here — one React component per (chunk, bundle,
 * variant, role), thousands live, whose whole body was a useMemo owning a
 * THREE.Mesh and a <primitive> rendering it. It moved into chunk_node.ts for
 * the same reason the cube meshes did (a component that only owns a scene
 * object buys nothing and costs a deferred mount), and then stopped being a
 * mesh at all: field.ts owns one instanced mesh per variant for the whole
 * world, and a chunk hands it the instances it should draw. What is left here
 * is what was always the real content of this file — the pure "what would
 * this chunk look like at this tier" build. */
