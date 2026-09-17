// pxl_shared/asset/asset.test.rs

use super::texture::{shipped_texture_key, shipped_texture_tile, CubeFace, SHIPPED_SHEET};

#[test]
fn shipped_textures_a_land_top_and_side_a_block_top_and_no_block_side() {
    assert!(shipped_texture_key("land", "BARE", CubeFace::Top).unwrap().starts_with("asset/"));
    assert!(shipped_texture_key("land", "BARE", CubeFace::Side).unwrap().starts_with("asset/"));
    assert!(shipped_texture_key("block", "white", CubeFace::Top).unwrap().starts_with("asset/"));
    assert_eq!(shipped_texture_key("block", "white", CubeFace::Side), None);
    assert_eq!(shipped_texture_key("water", "RIVER", CubeFace::Side), None);
    // The sheet holds every tile the keys name, each at a cell-aligned origin.
    assert!(SHIPPED_SHEET.key.starts_with("asset/"));
    let tile = shipped_texture_tile("land", "BARE", CubeFace::Top).unwrap();
    assert_eq!(tile.x % SHIPPED_SHEET.cell, 0);
    assert_eq!(tile.y % SHIPPED_SHEET.cell, 0);
    assert_eq!(shipped_texture_tile("block", "white", CubeFace::Side), None);
}
