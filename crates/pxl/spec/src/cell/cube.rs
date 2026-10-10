// pxl-spec/cell/cube.rs
// The numeric-id catalog for every cell type: the four terrain channels and
// the flat block colors. `common` holds the sentinels VOID, AIR (an
// object-layer erase only) and UNKNOWN; only the terrain channels hold VOID
// or UNKNOWN.

pub use super::cube_type::{cube_types, ground_pack, GroundChannelKey};
