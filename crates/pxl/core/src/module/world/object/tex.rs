// pxl_core/module/world/object/tex.rs

use pxl_shared::asset::texture::{shipped_texture_key, CubeFace};
use pxl_shared::spec::cell::{placeable_cube_id, PlaceableCube};

/// The shipped image a cube of this kind renders with on this face. A cube's
/// id is `"<channel>.<KEY>"`, which is how the shipped texture table is
/// keyed, so the id is the lookup.
///
/// Side falls back to top when a kind has no side of its own — the same rule
/// the layer arrays the chunks sample are built by. Only the land channel has
/// sides today; object blocks never do.
pub fn texture_key(cube: PlaceableCube, face: CubeFace) -> Option<&'static str> {
    let id = placeable_cube_id(cube);
    let (channel, r#type) = id.split_once('.').expect("a cube id names its channel");
    shipped_texture_key(channel, r#type, face).or_else(|| shipped_texture_key(channel, r#type, CubeFace::Top))
}
