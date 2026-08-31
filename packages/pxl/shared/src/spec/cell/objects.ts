// @pxl/shared/spec/cell/objects.ts
// [Layer: objects] Placed-object type/color catalog — object layer's counterpart to terrain.ts

import { common_colors, common_types } from "./common.ts";
import default_colors_json from "@pxl/shared/assets/cell/palette/objects/default.json" with { type: "json" };

// ============================================================================
// Type Definitions
// ============================================================================
//
// Object leaf values own the high bit. Zero is deliberately not an object
// type: it is sparse-tree absence, meaning this layer has no opinion and the
// compositor must fall through to terrain. Every real object-layer value has
// bit 7 set. Low id 1 is explicit AIR (a cave/destruction tombstone), real
// materials begin at 2, and 0xff remains UNKNOWN.
//
// "block" — what this channel actually is — not "color": color (red,
// orange, ...) is one axis *within* a block's identity, not the channel
// itself; a channel should name the type of thing being cataloged the
// way terrain_types' own land/water/structure/transport channels each
// name a domain, not a channel's own visual property. Not "wool" either
// — wool is just the texture pack this catalog happens to render its
// blocks with today (../../assets/cell/object_images.ts), an implementation detail
// exactly as disconnected from this type namespace as terrain_types'
// channels are from "simplepixel" (terrain's own texture pack, never
// appears in terrain_types). More channels land here once real object
// categories exist beyond flat-colored blocks (wood/stone/... — a real
// material axis, not "which resource pack drew it"), same role
// terrain_types' multiple channels play. The id (not the name) is what
// actually gets packed into a PlacedObject's stored form (apps/web's
// objects/store.ts), the same way a terrain cell's `ground` channel
// stores a numeric type id rather than a name.
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
// Color Definitions — loaded from JSON (see ../../assets/cell/palette/objects/)
// ============================================================================
//
// Same reasoning and same build_colors_from_json shape as terrain.ts's own
// color loading: JSON keys by the type's own NAME string ("white", "red",
// ...) since a JSON object literal can't reference `[object_types.block.
// white]` the way a TS computed property could, so this does that name ->
// type_id resolution once, here, against object_types, and throws
// immediately on a typo'd name rather than silently dropping a color —
// not a separate helper shared with terrain.ts's own copy, since it's
// small and tightly coupled to each file's own *_types shape, same reason
// terrain.ts's own version isn't factored out either.
type ColorJsonEntry = { hex: string };
type ColorJsonFile = { _meta?: { description?: string }; [ch_key: string]: { description?: string } | Record<string, ColorJsonEntry> | undefined };

function build_object_colors_from_json(json: ColorJsonFile): Record<ObjectChannelKey, Record<number, string | null>> {
  const out = {} as Record<ObjectChannelKey, Record<number, string | null>>;

  (Object.keys(json) as (ObjectChannelKey | "_meta")[]).forEach((ch_key) => {
    if (ch_key === "_meta") return;

    const types_for_ch = object_types[ch_key];
    if (!types_for_ch) {
      throw new Error(`object color JSON references unknown channel "${ch_key}" — check object_types`);
    }

    const ch_out: Record<number, string | null> = {
      [object_types.block.AIR]: null,
      [object_types.block.UNKNOWN]: common_colors[common_types.UNKNOWN],
    };
    const ch_entries = json[ch_key] as Record<string, ColorJsonEntry>;
    Object.entries(ch_entries).forEach(([type_name, entry]) => {
      const type_id = (types_for_ch as Record<string, number>)[type_name];
      if (type_id === undefined) {
        throw new Error(`object color JSON references unknown type "${ch_key}.${type_name}" — check object_types`);
      }
      ch_out[type_id] = entry.hex;
    });

    out[ch_key] = ch_out;
  });

  return out;
}

/** Active palette — every object-color consumer reads this one directly (no apps/web-local adapter, same as terrain_colors — shader_meta.ts imports terrain_colors/terrain_types straight from terrain.ts, nothing wraps it). Only one theme so far ("default"), unlike terrain's brick/soft/carto — room for more later. */
export const object_colors: Record<ObjectChannelKey, Record<number, string | null>> = build_object_colors_from_json(
  default_colors_json,
);

// ============================================================================
// App-facing convenience — flat name-keyed accessors for the "block" channel
// ============================================================================
//
// object_types/object_colors are id-first (name -> id -> hex), matching
// terrain's own storage-oriented shape — but apps/web's UI/store code
// (palette swatches, PlacedObject.color_id, THREE material color) wants a
// plain color NAME as its working unit, the same way it always has. Since
// "block" is the only channel today, this is a real, non-speculative flat
// view of it — not a premature multi-channel abstraction — and it lives
// here (not a separate apps/web adapter file/module) so there's exactly
// one place objects' color vocabulary is defined, matching terrain having
// none either. Excludes the object-owned AIR/UNKNOWN states — those are
// structural values, not real pickable colors.
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
//
// Exponents, not fractions (0 / -1 / -2 / -3, not 1 / 1/2 / 1/4 / 1/8): a
// size tier is "one terrain cell scaled by 2^exponent", and apps/web's
// OBJECT_SIZES (objects/size.ts) is indexed the same way — tier index i
// there is exactly exponent -i. Keeping the correspondence as an integer
// (not a fraction string or a float) is what makes that index math exact
// instead of float-comparison-fragile.
export const OBJECT_SCALE_EXPONENTS = [0, -1, -2, -3] as const;
export type ObjectScaleExponent = typeof OBJECT_SCALE_EXPONENTS[number];

// Finest tier (-3, 1/8 cell) is off by default for every type — too small
// to be a useful pick in the flyout, same reasoning size.ts's own doc
// comment gives for why OBJECT_SIZES still carries it anyway (nothing
// downstream of placement/collision/render should lose the ability to
// represent that size, only the picker hides it). Override per-type below
// once a real type actually needs a narrower (or the full 4-tier) range —
// speculative entries for types that don't exist yet don't belong here.
const DEFAULT_ACTIVE_SCALES: readonly ObjectScaleExponent[] = [0, -1, -2, -3];

const OBJECT_ACTIVE_SCALES: Partial<Record<ObjectColorId, readonly ObjectScaleExponent[]>> = {};

/** Which scale exponents `id` may be placed at — mode_bar.tsx's size flyout filters to just these, per swatch. Falls back to DEFAULT_ACTIVE_SCALES for any type without its own override in OBJECT_ACTIVE_SCALES. */
export function object_scale_exponents(id: ObjectColorId): readonly ObjectScaleExponent[] {
  return OBJECT_ACTIVE_SCALES[id] ?? DEFAULT_ACTIVE_SCALES;
}
