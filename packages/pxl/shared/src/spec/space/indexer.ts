// @pxl/shared/spec/space/indexer.ts
// Multi-level spatial indexing and hierarchy helpers.

/**
 * Multi-level spatial topology and packed hierarchy helpers.
 *
 * This module defines the packed level hierarchy over IVec and provides:
 * - aligned level boundary helpers (BASE / CENTER / LIMIT)
 * - parent-relative local indices (IDX_IN)
 * - packed logical index conversion (IDX / FROM_*_IDX)
 *
 * Hierarchy (dimension-agnostic — a space-axis scale tier, independent of
 * what shape of data any given layer stores at that tier):
 * - L5 (Shard)         : global sharding domain
 * - L4 (Anchor)        : floating-origin anchor domain
 * - L3 (Page)          : network transaction / paging unit
 * - L2 (Chunk)         : render / LOD batching unit
 * - L1 (Block)         : fundamental canonical spatial unit (`u`)
 * - L0 (Sub-block/SVO) : fractional / sub-block layer handled outside this module
 *
 * Packed lane layout (inherited from PVF3 IVec):
 * [ 63 ................. 40 | 39 ................. 16 | 15 ................. 0 ]
 * [      Lane X (24 bits)   |      Lane Y (24 bits)   |      Lane Z (16 bits)   ]
 *
 * Physical coordinate interpretation:
 * - X: signed lane with cyclic seam semantics
 * - Y: signed lane for north-south extent
 * - Z: signed lane for altitude
 *
 * Address / index domain:
 * - `addr` is a packed L1-granularity address in IVec space.
 * - IVec storage is a packed bitfield. Signedness is a lane interpretation rule,
 *   not a storage rule.
 * - IDX / IDX_IN are per-axis coordinates at the named level, computed via
 *   ivec.sh's per-lane shift — never a raw packed-word shift, which leaks
 *   bits across lane boundaries and corrupts every lane but the outermost
 *   one, positive or negative alike. Safe to reuse for spatial arithmetic
 *   (add/sub) directly.
 *
 * Offset rule:
 * - "offset" is reserved for addr diffs only, i.e. differences between two L1 addrs.
 * - Higher-level hierarchy relations use IDX / IDX_IN, not "offset".
 *
 * Axis participation rule:
 * - BASE / LIMIT are computed mechanically from packed masks and ranges.
 * - If a level range reaches or exceeds a lane width, that lane collapses at that
 *   level.
 * - In a collapsed lane, BASE and LIMIT become identical on that axis, so that
 *   axis no longer contributes a meaningful partition range above that level.
 *
 * Current consequence:
 * - X / Y remain hierarchical through L5.
 * - Z collapses at L5 (Z_W = 16, L5_SHAMT = 16), so L5 is effectively horizontal.
 *
 * Naming:
 * - RANGE   = absolute coordinate range at that level, measured in canonical `u` units
 * - SPAN_IN = number of child slots / units contained in the parent level
 *
 * Range convention:
 * - BASE is inclusive
 * - LIMIT is exclusive
 */

import { BIT64, FIELD_MASK64, type u32, type u64 } from "@xpute/core/kit/word.ts";
import { type IVec, ivec, type PVF3, pvf3 } from "../space/pvf3/mod.ts";

const { add, fill } = ivec;

// ---- Hierarchy Configuration ----

/**
 * SHAMT is the cumulative packed bit distance from the canonical base unit (`u` = L1).
 */
export const L2_SHAMT: u32 = 6 as const;
export const L3_SHAMT: u32 = 10 as const;
export const L4_SHAMT: u32 = 12 as const;
// export const L5_SHAMT: u32 = 16 as const;

const _L2_SHAMT = Number(L2_SHAMT);
const _L3_SHAMT = Number(L3_SHAMT);
const _L4_SHAMT = Number(L4_SHAMT);
// const _L5_SHAMT = Number(L5_SHAMT);

// ---- Absolute Ranges (in Canonical `u` Units) ----

// canonical base unit: 1u = 1 L1 block
export const L2_RANGE: u64 = BIT64(L2_SHAMT); // 1 << 6  = 64u
export const L3_RANGE: u64 = BIT64(L3_SHAMT); // 1 << 10 = 1024u
export const L4_RANGE: u64 = BIT64(L4_SHAMT); // 1 << 12 = 4096u
// export const L5_RANGE: u64 = BIT64(L5_SHAMT); // 1 << 16 = 65536u

// ---- Relative Spans inside Parent Levels ----

export const L2_SPAN_IN_L3: u64 = BIT64(L3_SHAMT - L2_SHAMT);
export const L2_SPAN_IN_L4: u64 = BIT64(L4_SHAMT - L2_SHAMT);
export const L3_SPAN_IN_L4: u64 = BIT64(L4_SHAMT - L3_SHAMT);
// export const L3_SPAN_IN_L5: u64 = BIT64(L5_SHAMT - L3_SHAMT);
// export const L4_SPAN_IN_L5: u64 = BIT64(L5_SHAMT - L4_SHAMT);

// ---- Hierarchy Masks ----

/**
 * These are component-wise packed masks used to isolate lower-level bits.
 */
export const L2_MASK: IVec = fill(FIELD_MASK64(L2_SHAMT));
export const L3_MASK: IVec = fill(FIELD_MASK64(L3_SHAMT));
export const L4_MASK: IVec = fill(FIELD_MASK64(L4_SHAMT));
// export const L5_MASK: IVec = fill(FIELD_MASK64(L5_SHAMT)); // Z masks out completely at this level (Z_W = 16)

// ---- Level Navigation ----

/**
 * Resolves aligned level boundaries from an absolute packed L1 address.
 */
export const L2_BASE = (addr: IVec): IVec => addr & ~L2_MASK;
export const L3_BASE = (addr: IVec): IVec => addr & ~L3_MASK;
export const L4_BASE = (addr: IVec): IVec => addr & ~L4_MASK;
// export const L5_BASE = (addr: IVec): IVec => addr & ~L5_MASK;

export const L2_CENTER = (addr: IVec): IVec => L2_BASE(addr) | (fill(L2_RANGE) >> 1n);
export const L3_CENTER = (addr: IVec): IVec => L3_BASE(addr) | (fill(L3_RANGE) >> 1n);
export const L4_CENTER = (addr: IVec): IVec => L4_BASE(addr) | (fill(L4_RANGE) >> 1n);
// export const L5_CENTER = (addr: IVec): IVec => L5_BASE(addr) | (fill(L5_RANGE) >> 1n);

/**
 * The centre of the cell that contains `addr`, for a span of `1 << shamt`
 * canonical units — the parameterised form of the three CENTER helpers
 * above, for a span resolved at runtime (a render tier, say) rather than
 * named at compile time.
 *
 * PVF3 rather than IVec, because the half is sub-integer at the base level:
 * a 1u cell's centre is 0.5u and lives in the fractional lane. L2/L3/L4 can
 * OR their half into the integer lane only because their spans are 64u and
 * up; this is the same expression written where it also holds at shamt 0.
 *
 * All three axes, symmetric. A cell is a cube and its centre is its centre —
 * X/Y quantised while Z is left raw (or the reverse) is two coordinate
 * systems in one point, and everything that marks a cell here already avoids
 * it: object/geometry.ts's anchor_center adds the half on every axis, and
 * hover, placed cubes and the location puck all read a cell the same way.
 */
export function SPAN_CENTER(out: PVF3, addr: IVec, shamt: u32): PVF3 {
  out.i = shamt === 0 ? addr : addr & ~fill(FIELD_MASK64(shamt));
  out.f = 0n;
  const half = pvf3.from_real(Number(BIT64(shamt)) / 2);
  return pvf3.gather(out, pvf3.X(out) + half, pvf3.Y(out) + half, pvf3.Z(out) + half);
}

export const L2_LIMIT = (addr: IVec): IVec => add(L2_BASE(addr), fill(L2_RANGE));
export const L3_LIMIT = (addr: IVec): IVec => add(L3_BASE(addr), fill(L3_RANGE));
export const L4_LIMIT = (addr: IVec): IVec => add(L4_BASE(addr), fill(L4_RANGE));
// export const L5_LIMIT = (addr: IVec): IVec => add(L5_BASE(addr), fill(L5_RANGE));

// ---- Parent-relative Local Indexing ----

/**
 * Returns the raw packed local index of a child level within its parent level.
 *
 * Semantics:
 * - IDX_IN is a logical local key, derived by isolating the bits that
 *   participate inside the parent level.
 */
export const L1_IDX_IN_L2 = (addr: IVec): IVec => addr & L2_MASK;
export const L1_IDX_IN_L3 = (addr: IVec): IVec => addr & L3_MASK;
export const L2_IDX_IN_L3 = (addr: IVec): IVec => addr & (L3_MASK & ~L2_MASK);
export const L1_IDX_IN_L4 = (addr: IVec): IVec => addr & L4_MASK;
export const L2_IDX_IN_L4 = (addr: IVec): IVec => addr & (L4_MASK & ~L2_MASK);
export const L3_IDX_IN_L4 = (addr: IVec): IVec => addr & (L4_MASK & ~L3_MASK);
// export const L3_IDX_IN_L5 = (addr: IVec): IVec => addr & (L5_MASK & ~L3_MASK);
// export const L4_IDX_IN_L5 = (addr: IVec): IVec => addr & (L5_MASK & ~L4_MASK);

// ---- Global Index Conversion ----

/**
 * Converts between aligned packed level bases and logical indices.
 *
 * Semantics:
 * - BASE(addr) first clears lower-level bits; ivec.sh then shifts each lane
 *   independently, never the whole packed word, so no lane leaks bits into
 *   its neighbor.
 * - FROM_*_IDX restores packed base form from that logical index field.
 */
export const L2_IDX = (addr: IVec): IVec => ivec.sh(L2_BASE(addr), -_L2_SHAMT);
export const L3_IDX = (addr: IVec): IVec => ivec.sh(L3_BASE(addr), -_L3_SHAMT);
export const L4_IDX = (addr: IVec): IVec => ivec.sh(L4_BASE(addr), -_L4_SHAMT);
// export const L5_IDX = (addr: IVec): IVec => ivec.sh(L5_BASE(addr), -_L5_SHAMT); // Z has collapsed at L5

export const FROM_L2_IDX = (idx: IVec): IVec => ivec.sh(idx, _L2_SHAMT);
export const FROM_L3_IDX = (idx: IVec): IVec => ivec.sh(idx, _L3_SHAMT);
export const FROM_L4_IDX = (idx: IVec): IVec => ivec.sh(idx, _L4_SHAMT);
// export const FROM_L5_IDX = (idx: IVec): IVec => ivec.sh(idx, _L5_SHAMT);
