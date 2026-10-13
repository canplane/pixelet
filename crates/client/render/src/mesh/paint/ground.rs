// pxl-render/mesh/paint/ground.rs

//! What a block that fills is drawn as: its swatch and texture layer, a
//! ground look's or a placed cube's (object.rs).

use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::block::color;
use pxl_spec::block::cube::{sentinel, Ground};
use pxl_spec::block::object::Cube;
use pxl_spec::block::Block;
use std::sync::LazyLock;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::mesh::paint::object::PAINTER;
use crate::mesh::paint::palette::{rgb255, LAYER_IDX};
use crate::mesh::word::NO_LAYER;

/// Each byte's swatch and layer; a byte that is no look is drawn as unknown.
static LOOKS: LazyLock<[([u8; 3], u8); 256]> = LazyLock::new(|| {
    let unknown = (color::sentinel(sentinel::unknown).map_or([0; 3], |s| rgb255(s.rgb)), NO_LAYER as u8);
    let mut looks = [unknown; 256];
    for g in Ground::ALL {
        looks[g as usize] = (color::ground(g as u8).map_or([0; 3], |s| rgb255(s.rgb)), LAYER_IDX(Cube::Ground(g)) as u8);
    }
    looks
});

/// A ground look's swatch and layer.
pub fn ground(g: GroundId) -> ([u8; 3], u8) {
    LOOKS[g as usize]
}

/// What a block that fills is drawn as: its swatch and layer.
pub fn look(block: Block) -> ([u8; 3], u8) {
    match block {
        Block::Ground(g, _) => ground(g),
        Block::Placed(state) => PAINTER.look_of(state).or_bug(Errno::ENOTRECOVERABLE),
    }
}
