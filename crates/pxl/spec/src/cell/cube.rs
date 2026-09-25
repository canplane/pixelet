// pxl-spec/cell/cube.rs
// [Layer: cube] The one numeric-id/color catalog for every placeable cell
// type — both the baked 2.5D terrain raster's own four channels
// (land/water/structure/transport, filled by the bake pipeline, one
// independent value per channel per cell) and the flat "block" colors a
// user paints directly. Every channel is a name-to-number table of the same
// shape, whether the bake pipeline treats it as real map data or a user's
// palette.
//
// `common` is a shared registry of sentinel values — VOID (nothing painted),
// AIR (an explicit object-layer erase) and UNKNOWN (missing or invalid data)
// — and the other channels do not spread its keys into their own. Only
// land/water/structure/transport can hold VOID or UNKNOWN, those being raw
// bytes a baked raster legitimately produces; AIR means something only
// within the object-placement encoding (object.rs), never as a terrain value,
// so a channel that can never produce it does not list it.
//
// object.rs is where a placed cube's own value is derived from these channels,
// banding the terrain channels into disjoint ranges — a real terrain cell
// can hold one value from every channel at once, where a placed cube is
// exactly one of them. Nothing here needs to know that; this file is cell
// definitions and nothing else.

// A cube's color is color.spec.rs's, by its channel and its number; a
// ground channel falls back to `common`'s, which is how a cell holding VOID
// shows nothing and one holding UNKNOWN a neutral grey.

pub use super::cube_type::{cube_types, ground_pack, GroundChannelKey};
