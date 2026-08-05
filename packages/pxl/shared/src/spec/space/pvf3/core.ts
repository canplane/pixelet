// @pxl/shared/spec/space/pvf3/core.ts

/**
 * PVF3 (Packed 3-Vector w/ Fixed-Point Arithmetic)
 * Central Metadata Configuration
 *
 * This module defines the raw bit-width specifications and fundamental bitwise
 * utilities for the PVF3 system. It serves as the single source of truth for
 * canonical bit-layout specifications across all generalized vector kernels.
 *
 * Design Philosophy:
 * - Domain Agnostic: This system is a pure numerical framework. No assumptions
 *   are made about the coordinate domain (e.g., Geographic or Local).
 * - Width-Parametric Specification: All width constants are adjustable to fit
 *   specific spatial requirements and word-size constraints.
 * - Functional Encapsulation: Only raw widths are defined here. Shift amounts
 *   (SHAMT) and masks are derived within individual kernels to maintain
 *   internal operational autonomy and configuration flexibility.
 * - Deterministic Arithmetic: All utilities operate in the BigInt domain to
 *   ensure bit-exact behavior and avoid floating-point side effects.
 *
 * @module core
 */

import { BIT64, type bits, FIELD_MASK64, type u32, type u64 } from "@xpute/core/kit/word.ts";

// ---- IVec: Integer Part Specification ----

/** Total nominal bit-width for the packed integer word. */
export const IVEC_WORD_W: bits = 64 as const;
/** Mask for the entire packed word (e.g., 64-bit full mask) */
export const IVEC_WORD_MASK: u64 = FIELD_MASK64(IVEC_WORD_W);
/** Bit-depth resolution assigned to Lane X. */
export const IVEC_X_W: u32 = 24 as const;
/** Bit-depth resolution assigned to Lane Y. */
export const IVEC_Y_W: u32 = 24 as const;
/** Bit-depth resolution assigned to Lane Z. */
export const IVEC_Z_W: u32 = 16 as const;

// ---- FVec: Fractional Part Specification ----

/** Total nominal bit-width for the packed fractional word. */
export const FVEC_WORD_W: bits = 32 as const;
/** Mask for the entire packed word (e.g., 0xFFFFFFFF) */
export const FVEC_WORD_MASK: u64 = FIELD_MASK64(FVEC_WORD_W);
/** Symmetric bit-depth resolution assigned per fractional lane. */
export const FRAC_W: u32 = 10 as const;
/** Cardinality of one fractional lane (2^FRAC_W). */
export const ONE: u64 = BIT64(FRAC_W);
/** Low fractional-lane bit mask. */
export const ONE_MASK: u64 = ONE - 1n;

// ---- Atomic Bitwise Utilities ----

const FALSE: bigint = 0n;
const TRUE: bigint = ~FALSE;
export const ABS = (e: bigint): bigint => (e < 0n ? -e : e);
export const EQ = (a: bigint, b: bigint): bigint => (a === b ? TRUE : FALSE);
export const LT = (a: bigint, b: bigint): bigint => (a < b ? TRUE : FALSE);
export const GT = (a: bigint, b: bigint): bigint => (a > b ? TRUE : FALSE);
export const MAX = (a: bigint, b: bigint): bigint => (a > b ? a : b);
export const MIN = (a: bigint, b: bigint): bigint => (a < b ? a : b);
