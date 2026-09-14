// @pxl/web/module/world/decor/scatter.ts

/**
 * Deterministic decoration placement — the general half of the "scatter
 * some prop across eligible terrain cells" pipeline this module family
 * exists to establish (trees, boats, cars, and trains today; more kinds
 * later reuse this same function with their own DecorKindSeed +
 * eligibility predicate, never their own bespoke RNG). Rendering, terrain
 * access, and the "is a user object already standing here" check are all
 * decor_kinds.ts's concern instead — this file only ever sees plain
 * ground-channel arrays and a seed, so it can be tested head-on with
 * synthetic ground data, no tile store or Three.js involved.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { derive_seed_u64, make_rng, u32_to_unit } from "@xpute/core/kit/rng.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { type Ground, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { PAGE_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

/** True if this cell's raw ground-channel values are safe ground for this
 * decoration kind to stand on — mirrors reference/terrain_layer.ts's
 * own ground_material_at precedence (land is the base, water/structure/
 * transport each override it only when not VOID), since that's the same
 * "what does this cell actually look like" question a texture-pack-based
 * predicate has to answer. */
export type DecorEligibility = (land: u32, water: u32, structure: u32, transport: u32) => boolean;

/** What a channel's type must be for a kind to stand on the cell: anything,
 * nothing painted, or one of a list of types. */
export type DecorChannelRule = "any" | "void" | readonly u32[];

/** A kind's rule per ground channel, declared so the module can be taught
 * the same rule (crates/pxl-core/src/world/decor.rs) and `eligibility_of` derives
 * the predicate the TypeScript reference reads. */
export type DecorRules = {
  land: DecorChannelRule;
  water: DecorChannelRule;
  structure: DecorChannelRule;
  transport: DecorChannelRule;
};

export const DECOR_RULE_CHANNELS: readonly (keyof DecorRules)[] = ["land", "water", "structure", "transport"];

function rule_allows(rule: DecorChannelRule, t: u32): boolean {
  if (rule === "any") return true;
  if (rule === "void") return t === cube_types.common.VOID;
  return rule.includes(t);
}

export function eligibility_of(rules: DecorRules): DecorEligibility {
  return (land, water, structure, transport) => rule_allows(rules.land, land) && rule_allows(rules.water, water) && rule_allows(rules.structure, structure) && rule_allows(rules.transport, transport);
}

/**
 * How a placed instance picks its own heading:
 * - "random": uniform over a full turn — fine for anything that reads the
 *   same from any angle (a tree, a boat drifting on open water).
 *   "align_transport": faces along whichever axis (X or Y) this cell's own
 *   transport channel actually runs, read off its immediate neighbors —
 *   for anything that only looks right lined up with a road/rail (a car, a
 *   train), where a uniformly random spin would read as "crashed", not
 *   "parked/passing through".
 */
export type DecorOrientationMode = "random" | "align_transport";

export type DecorKindSeed = {
  /** Distinguishes this kind's own random stream from another kind
   * scattered over the very same cells (e.g. a future fence kind sharing
   * this same green terrain) — any value works as long as it stays fixed
   * for this kind across sessions; changing it reshuffles every placement. */
  tag: bigint;
  /** Per-eligible-cell probability [0, 1) of actually placing an instance
   * there — the only knob "how dense" tuning needs. */
  density: f64;
  /** Uniformly picks a variant index in [0, variant_count). */
  variant_count: u32;
  orientation: DecorOrientationMode;
};

export type DecorCandidate = {
  /** Page-local cell coordinates (same convention slice_ground_ch/
   * ground_material_at index by: cell_y * PAGE_CELL_SPAN + cell_x). */
  cell_x: u32;
  cell_y: u32;
  variant: u32;
  /** Radians, about the decoration's own vertical axis. */
  rotation: f64;
  /** Uniform scale multiplier, jittered around 1 for organic variety. */
  scale: f64;
};

// +/- this fraction around 1.0 — visible variety between instances of the
// same variant without any one of them reading as a visibly wrong size.
const SCALE_JITTER: f64 = 0.18;

// How far transport_aligned_rotation looks past the immediate neighbor on
// each side — a fixed, small radius (never a flood-fill/BFS: O(1) reads per
// candidate, independent of how long the actual road/rail run is), just
// enough to outvote a single-cell diagonal "stairstep" jog in an otherwise
// straight run. A true curve still won't resolve to the right heading at
// any radius — there's no cardinal-aligned rotation that's actually
// correct there — this only smooths over rasterization noise on a
// fundamentally straight segment, not real curvature.
const ALIGN_NEIGHBOR_RADIUS: u32 = 3;

/** "align_transport" heading: counts populated transport cells up to
 * ALIGN_NEIGHBOR_RADIUS out along each axis and picks whichever axis has
 * more (a plurality vote, not just the immediate ±1 neighbor) — PI/2 if
 * the horizontal count wins, 0 otherwise (vertical, a tie, or a fully
 * isolated cell with nothing on either axis; a tie or isolated cell has no
 * real answer, so 0 is only ever an arbitrary-but-deterministic fallback
 * for those, not a claim that the run is actually vertical). Bounds-
 * checked against the page edge itself, not just span, since `ground` here
 * is always a full page's own arrays (see this file's own doc comment).
 *
 * `flip` adds a random +PI on top of that axis — still lined up with the
 * road/rail either way, but facing one direction or the opposite one, same
 * as real two-way traffic never all pointing the same way. Without it every
 * instance on a given road/rail run shares the exact same heading, which
 * reads as a repeated prop, not independent vehicles. */
function transport_aligned_rotation(transport: Uint8Array, cell_x: u32, cell_y: u32, flip: boolean): f64 {
  const idx = cell_y * PAGE_CELL_SPAN + cell_x;
  let horizontal_votes = 0;
  let vertical_votes = 0;
  for (let d = 1; d <= ALIGN_NEIGHBOR_RADIUS; d++) {
    if (cell_x - d >= 0 && transport[idx - d] !== cube_types.common.VOID) horizontal_votes++;
    if (cell_x + d < PAGE_CELL_SPAN && transport[idx + d] !== cube_types.common.VOID) horizontal_votes++;
    if (cell_y - d >= 0 && transport[idx - d * PAGE_CELL_SPAN] !== cube_types.common.VOID) vertical_votes++;
    if (cell_y + d < PAGE_CELL_SPAN && transport[idx + d * PAGE_CELL_SPAN] !== cube_types.common.VOID) vertical_votes++;
  }
  const axis_rotation = horizontal_votes > vertical_votes ? Math.PI / 2 : 0;
  return flip ? axis_rotation + Math.PI : axis_rotation;
}

/**
 * Scans page-local cells [tx0, tx0+span) x [ty0, ty0+span) — a chunk's own
 * footprint within a page's full-resolution ground arrays — and returns
 * every eligible cell's placement, keyed off a seed derived from
 * (page_seed, kind.tag, cell_x, cell_y) alone. Pure and fully deterministic:
 * the same five inputs always produce the same candidates, regardless of
 * when, how often, or in what order this runs — decor_kinds.ts depends on
 * that determinism to let a placed user object's cell "win" over a
 * scattered decoration by simply filtering candidates afterward, rather
 * than persisting which cells got suppressed.
 */
export function scatter_decor_candidates(
  ground: Ground,
  tx0: u32,
  ty0: u32,
  span: u32,
  page_seed: bigint,
  kind: DecorKindSeed,
  is_eligible: DecorEligibility,
  /** Multiplies kind.density before the per-cell roll — the caller's own
   * distance-based falloff (e.g. decor_kinds.ts thinning candidates past a
   * given LOD tier), kept as a plain scalar here so this function stays
   * free of any renderer/LOD concept of its own. 1 (every candidate rolled
   * at the kind's own density) unless a caller passes otherwise. */
  density_scale: f64 = 1,
): DecorCandidate[] {
  const out: DecorCandidate[] = [];
  const land = ground[GroundChannelKey.LAND];
  const water = ground[GroundChannelKey.WATER];
  const structure = ground[GroundChannelKey.STRUCTURE];
  const transport = ground[GroundChannelKey.TRANSPORT];
  const kind_seed = derive_seed_u64(page_seed, kind.tag);

  for (let dy: u32 = 0; dy < span; dy++) {
    const cell_y = ty0 + dy;
    const row = cell_y * PAGE_CELL_SPAN;
    for (let dx: u32 = 0; dx < span; dx++) {
      const cell_x = tx0 + dx;
      const idx = row + cell_x;
      if (!is_eligible(land[idx], water[idx], structure[idx], transport[idx])) continue;

      // 16 bits per axis comfortably covers PAGE_CELL_SPAN (1024, 10 bits) —
      // room to spare if a future L3_SHAMT change ever grows it.
      const cell_tag = (BigInt(cell_x) << 16n) | BigInt(cell_y);
      const rng = make_rng(derive_seed_u64(kind_seed, cell_tag));

      if (u32_to_unit(rng()) >= kind.density * density_scale) continue;
      const variant = Math.min(Math.floor(u32_to_unit(rng()) * kind.variant_count), kind.variant_count - 1);
      const rotation = kind.orientation === "align_transport" ? transport_aligned_rotation(transport, cell_x, cell_y, u32_to_unit(rng()) < 0.5) : u32_to_unit(rng()) * Math.PI * 2;
      const scale = 1 + (u32_to_unit(rng()) * 2 - 1) * SCALE_JITTER;
      out.push({ cell_x, cell_y, variant, rotation, scale });
    }
  }

  return out;
}
