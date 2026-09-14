// pxl_core/module/world/object/tex.rs

use pxl_shared::record::texture::{shipped_texture_key, CubeFace};
use pxl_shared::spec::cell::is_terrain_style_cube;

/// The shipped image a cube of this kind renders with on this face, flat
/// colour or terrain-styled alike. A terrain-styled pick is
/// `"<channel>.<KEY>"` and a flat colour is a `block` type, which is how the
/// shipped texture table is keyed, so the two catalogs differ only in the
/// channel.
///
/// Side falls back to top when a kind has no side of its own — the same rule
/// the layer arrays the chunks sample are built by. Only the land channel has
/// sides today; object blocks never do.
pub fn texture_key(id: &str, face: CubeFace) -> Option<&'static str> {
    let (channel, r#type) = if is_terrain_style_cube(id) { id.split_once('.').unwrap_or(("block", id)) } else { ("block", id) };
    shipped_texture_key(channel, r#type, face).or_else(|| shipped_texture_key(channel, r#type, CubeFace::Top))
}
