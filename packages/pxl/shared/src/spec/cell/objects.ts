// @pxl/shared/spec/cell/objects.ts
// [Layer: objects] Placed-object type/color catalog — object layer's counterpart to terrain.ts

import { common_colors, common_types } from "./common.ts";
import { build_colors_from_json } from "./type.ts";
import colors_json from "@pxl/shared/assets/cell/palette/objects.json" with { type: "json" };

// ============================================================================
// Type Definitions
// ============================================================================
//
// Object leaf values own the high bit. Zero is sparse-tree absence, not an
// object type — the compositor falls through to terrain. Every real value
// has bit 7 set: AIR is 1, real materials start at 2, UNKNOWN is 0xff.
export const OBJECT_PRESENT_MASK = 0x80;
export const OBJECT_TYPE_MASK = 0x7f;
/** Sparse-tree default: the object layer has no opinion at this position. */
export const OBJECT_UNDEF = 0x00;

export const object_types = {
  block: {
    AIR: OBJECT_PRESENT_MASK | 1,
    UNKNOWN: 0xff,
    // Neutrals (light -> dark), then one sweep around the hue wheel
    // starting at red, each shade/tint immediately trailing its own base
    // hue (brown after orange, dark_green after green) — not off in a
    // separate "earth tones" bucket by itself. pink closes the sweep right
    // before it wraps back around to red, since it reads as a light
    // red/magenta rather than a distinct stop of its own.
    white: OBJECT_PRESENT_MASK | 2,
    grey: OBJECT_PRESENT_MASK | 3,
    dark_grey: OBJECT_PRESENT_MASK | 4,
    black: OBJECT_PRESENT_MASK | 5,
    red: OBJECT_PRESENT_MASK | 6,
    orange: OBJECT_PRESENT_MASK | 7,
    brown: OBJECT_PRESENT_MASK | 8,
    yellow: OBJECT_PRESENT_MASK | 9,
    green: OBJECT_PRESENT_MASK | 10,
    dark_green: OBJECT_PRESENT_MASK | 11,
    cyan: OBJECT_PRESENT_MASK | 12,
    blue: OBJECT_PRESENT_MASK | 13,
    violet: OBJECT_PRESENT_MASK | 14,
    magenta: OBJECT_PRESENT_MASK | 15,
    pink: OBJECT_PRESENT_MASK | 16,
  },
} as const satisfies Record<string, Record<string, number>>;

export type ObjectChannelKey = keyof typeof object_types;

// ============================================================================
// Color Definitions — loaded from JSON (see ../../assets/cell/palette/objects.json)
// ============================================================================

export const object_colors = build_colors_from_json(
  object_types,
  colors_json,
  { [object_types.block.AIR]: null, [object_types.block.UNKNOWN]: common_colors[common_types.UNKNOWN] },
);

// ============================================================================
// App-facing convenience — flat name-keyed accessors for the "block" channel
// ============================================================================

// Excludes AIR/UNKNOWN — structural values, not pickable colors.
export type ObjectColorId = Exclude<keyof typeof object_types.block, keyof typeof common_types>;

/** Zero means sparse absence/fallback, not explicit air. */
export function object_value_is_present(value: number): boolean {
  return (value & OBJECT_PRESENT_MASK) !== 0;
}

/** Explicit object-layer air: retain this leaf and erase lower layers. */
export function object_value_is_air(value: number): boolean {
  return value === object_types.block.AIR;
}

export function object_color_num_id(id: ObjectColorId): number {
  return object_types.block[id];
}

export function object_color_hex(id: ObjectColorId): string {
  // Only object AIR/UNKNOWN can be null; ObjectColorId excludes both.
  return object_colors.block[object_types.block[id]] as string;
}

const NUM_ID_TO_COLOR: readonly ObjectColorId[] = (() => {
  const names = (Object.keys(object_types.block) as (keyof typeof object_types.block)[])
    .filter((name): name is ObjectColorId => !(name in common_types));
  const out: ObjectColorId[] = [];
  for (const name of names) out[object_types.block[name]] = name;
  return out;
})();

/** Numeric id -> color name — the reverse of object_color_num_id, used to decode a packed color channel back into a PlacedObject's own ObjectColorId. */
export function object_color_id_from_num(num_id: number): ObjectColorId {
  const id = NUM_ID_TO_COLOR[num_id];
  if (id === undefined) throw new Error(`no color registered for numeric id ${num_id}`);
  return id;
}

/** Every real color id in encoded numeric-id order; `.filter()` compacts the sparse reverse table. */
export const object_color_ids: readonly ObjectColorId[] = NUM_ID_TO_COLOR.filter(() => true);

// ============================================================================
// Placeable scale — which of the 4 grid-aligned sizes each type may use
// ============================================================================

// Exponents, not fractions — apps/web's OBJECT_SIZES (objects/size.ts) is
// indexed the same way, tier index i == exponent -i.
export const OBJECT_SCALE_EXPONENTS = [0, -1, -2, -3] as const;
export type ObjectScaleExponent = typeof OBJECT_SCALE_EXPONENTS[number];

// Finest tier (-3) hidden from the flyout by default — too small to be a
// useful pick. Override per-type in ACTIVE_SCALES once needed.
const DEFAULT_ACTIVE_SCALES: readonly ObjectScaleExponent[] = [0, -1, -2, -3];

const ACTIVE_SCALES: Partial<Record<ObjectColorId, readonly ObjectScaleExponent[]>> = {};

/** Which scale exponents `id` may be placed at — mode_bar.tsx's size flyout filters to just these, per swatch. Falls back to DEFAULT_ACTIVE_SCALES for any type without its own override in ACTIVE_SCALES. */
export function object_scale_exponents(id: ObjectColorId): readonly ObjectScaleExponent[] {
  return ACTIVE_SCALES[id] ?? DEFAULT_ACTIVE_SCALES;
}
