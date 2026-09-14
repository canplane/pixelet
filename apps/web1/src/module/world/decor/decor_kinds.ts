// @pxl/web/module/world/decor/decor_kinds.ts

/**
 * The decor kinds: what real GLB models stand where, taught to the module
 * (crates/pxl/core/src/world/decor.rs) as data — each kind's extent, density,
 * the channels it scatters on, its variants and their palette rows — and
 * the falloff by tier the module and the reference build share. The build
 * itself is the module's, a block of lanes a chunk and tier; the
 * TypeScript build it is held to is reference/decor_build.ts, with the
 * account of why each rule is what it is.
 *
 * No decoration is ever stored: scatter.ts's placement is a pure function
 * of terrain + a fixed seed, recomputed on demand. A cell that currently
 * holds a user-placed object simply has its candidate filtered out at
 * build time; removing that object clears the very same filter on the next
 * rebuild, so the decoration reappears with no bookkeeping of its own.
 *
 * Density falls off with distance snapped to the exact same LOD tiers the
 * terrain hands off at — not by reimplementing "distance to camera" a
 * second time, but by taking the chunk's own resolved tier — see
 * decor_density below: the terrain tiers' own projected-size criterion
 * applied to the kind's extent. Stepping a tier down (1 -> 0.5 -> 0.25 -> 0)
 * used to pop synchronized across every chunk that crossed a threshold in
 * the same frame, a whole ring of forest thinning at once; the scheduler's
 * blend staggers it (sched.rs's decor_secondary), whole-set: an entire
 * density level, on or off, never an instance's own scale or opacity.
 *
 * The tier split is Minecraft's own block model, taken literally
 * (2026-09-05): (1) static grid content — cubes and decor alike — bakes into
 * the chunk's own blocks; (2) static content that still needs a per-frame
 * visual update (an open chest, sign text) would keep that and add a thin
 * pass for the dynamic part — no such content exists yet; (3) anything that
 * moves under its own logic every frame (the companion) lives outside the
 * grid as an actor. User-placed decor (a tree a player plants via
 * mode_bar.tsx's decor button) lands in object/store.ts's per-cell tree as
 * its own entry kind ("decor") alongside "solid" — the same tier-1 storage
 * Minecraft gives decorative blocks — and is merged into the build next to
 * the scattered instances, not kept in a separate store.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { type IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { derive_seed_u64, make_rng, u32_to_unit } from "@xpute/core/kit/rng.ts";
import { decor_num_id, type DecorId } from "@pxl/shared/spec/cell/mod.ts";

import { shipped_palette } from "@pxl/shared/record/mod.ts";

import { asset_url } from "@/lib/env/route.ts";
import { type LodStep, TARGET_CELL_PX } from "@/module/world/lod.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { CORE_BUILD, packet } from "@/wasm/core_build.ts";

import { DECOR_RULE_CHANNELS, type DecorEligibility, type DecorKindSeed, type DecorRules } from "./scatter.ts";

// Fixed, arbitrary — the one thing every DecorKind's own placement
// ultimately derives from (alongside its own DecorKindSeed.tag). Changing
// this reshuffles every decoration in the world; there is no reason to,
// since a page's own idx already makes every page's stream independent.
export const DECOR_MASTER_SEED = 0xd0c0d0c0_dec0_5eed_0001n;

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

export function kind_extent(bundle: DecorKindBundle): f64 {
  let extent = 0;
  for (const row of bundle.palette_rows ?? []) extent = Math.max(extent, shipped_palette().extent_max(row));
  return extent;
}

/**
 * Teaches the module every kind, in bundle order — the order of the groups
 * in the decor blocks it hands over (crates/pxl/core/src/world/decor.rs): each
 * kind's variants in variant order, one group a variant. Called once, from
 * decor_registry.ts, with the registry's own list.
 */
export function teach_decor_kinds(bundles: readonly DecorKindBundle[]): void {
  CORE_BUILD.send(
    0,
    Cmd.DECOR_KIND_UPDATE,
    packet((p) =>
      p.branch((kinds) => {
        for (const bundle of bundles) {
          kinds.branch((k) =>
            k.u64(BigInt.asUintN(64, bundle.seed.tag))
              .f64(bundle.seed.density)
              .u32(bundle.seed.variant_count)
              .bool(bundle.seed.orientation === "align_transport")
              .f64(kind_extent(bundle))
              .i32(bundle.decor_id ? decor_num_id(bundle.decor_id) : -1)
              .branch((rules) => {
                for (const channel of DECOR_RULE_CHANNELS) {
                  const rule = bundle.rules[channel];
                  const mode = rule === "any" ? 0 : rule === "void" ? 1 : 2;
                  rules.branch((r) => r.u32(mode).u8_array(Uint8Array.from(mode === 2 ? rule as readonly number[] : [])));
                }
              })
          );
        }
      })
    ),
  );
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
