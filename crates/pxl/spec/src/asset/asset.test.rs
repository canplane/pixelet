// pxl-spec/asset/asset.test.rs

use super::texture::{shipped_texture_tile, CubeFace, SHIPPED_SHEET};
use crate::cell::cube::cube_types::{Block, Land, Water};
use crate::cell::PlaceableCube;

#[test]
fn shipped_textures_a_land_top_and_side_a_block_top_and_no_block_side() {
    let bare = PlaceableCube::Land(Land::bare);
    let white = PlaceableCube::Block(Block::white);
    assert!(shipped_texture_tile(bare, CubeFace::Top).is_some());
    assert!(shipped_texture_tile(bare, CubeFace::Side).is_some());
    assert!(shipped_texture_tile(white, CubeFace::Top).is_some());
    assert_eq!(shipped_texture_tile(white, CubeFace::Side), None);
    assert_eq!(shipped_texture_tile(PlaceableCube::Water(Water::river), CubeFace::Side), None);
    // Every tile at a cell-aligned origin.
    let tile = shipped_texture_tile(bare, CubeFace::Top).unwrap();
    assert_eq!(tile.x % SHIPPED_SHEET.cell, 0);
    assert_eq!(tile.y % SHIPPED_SHEET.cell, 0);
}
